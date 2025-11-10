// TON (The Open Network) Adapter - High-performance blockchain with infinite sharding
// Production-ready implementation with full TON API support

use async_trait::async_trait;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use base64::{Engine as _, engine::general_purpose};

use crate::router::{Intent, RouterError};
use crate::router::intent_translator::{IntentTranslator, TranslatedIntent};
use crate::router::chain_adapter::ChainAdapter;

pub struct TonAdapter {
    chain_name: String,
    chain_id: String,
    http_client: Arc<HttpClient>,
    api_url: String,
    translator: Arc<IntentTranslator>,
    network: TonNetwork,
}

#[derive(Debug, Clone, Copy)]
pub enum TonNetwork {
    Mainnet,
    Testnet,
}

impl TonAdapter {
    pub fn new(
        api_url: String,
        network: TonNetwork,
        translator: Arc<IntentTranslator>,
    ) -> Result<Self, RouterError> {
        let chain_id = match network {
            TonNetwork::Mainnet => "ton-mainnet",
            TonNetwork::Testnet => "ton-testnet",
        };
        
        Ok(Self {
            chain_name: "ton".to_string(),
            chain_id: chain_id.to_string(),
            http_client: Arc::new(HttpClient::new()),
            api_url,
            translator,
            network,
        })
    }
    
    async fn api_call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, RouterError> {
        let url = format!("{}/{}", self.api_url, method);
        
        let response = if params.is_null() {
            self.http_client.get(&url).send().await
        } else {
            self.http_client.post(&url).json(&params).send().await
        };
        
        let response = response
            .map_err(|e| RouterError::TranslationError(format!("API request failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(RouterError::TranslationError(format!("API error: {}", response.status())));
        }
        
        let result: serde_json::Value = response.json().await
            .map_err(|e| RouterError::TranslationError(format!("Failed to parse response: {}", e)))?;
        
        if let Some(ok) = result.get("ok") {
            if !ok.as_bool().unwrap_or(false) {
                let error = result.get("error").and_then(|e| e.as_str()).unwrap_or("Unknown error");
                return Err(RouterError::TranslationError(format!("API error: {}", error)));
            }
        }
        
        result.get("result")
            .cloned()
            .ok_or_else(|| RouterError::TranslationError("No result in response".to_string()))
    }
    
    fn validate_ton_address(&self, address: &str) -> Result<(), RouterError> {
        // TON addresses can be in two formats:
        // 1. Raw format: workchain:hex (e.g., 0:83dfd552e63729b472fcbcc8c45ebcc6691702558b68ec7527e1ba403a0f31a8)
        // 2. User-friendly format: base64 encoded (e.g., EQCBn2VvoGrLRLjnw5y-KD1TKh0-irF1xVJVOXXO8xMEscxT)
        
        if address.contains(':') {
            // Raw format
            let parts: Vec<&str> = address.split(':').collect();
            if parts.len() != 2 {
                return Err(RouterError::TranslationError("Invalid TON raw address format".to_string()));
            }
            
            // Verify workchain is valid number
            parts[0].parse::<i32>()
                .map_err(|_| RouterError::TranslationError("Invalid workchain".to_string()))?;
            
            // Verify hex part
            hex::decode(parts[1])
                .map_err(|_| RouterError::TranslationError("Invalid hex in address".to_string()))?;
        } else {
            // User-friendly format - should be base64
            general_purpose::STANDARD.decode(address)
                .map_err(|_| RouterError::TranslationError("Invalid TON user-friendly address".to_string()))?;
        }
        
        Ok(())
    }
    
    async fn get_address_info(&self, address: &str) -> Result<TonAddressInfo, RouterError> {
        let result = self.api_call(
            "getAddressInformation",
            json!({ "address": address })
        ).await?;
        
        Ok(TonAddressInfo {
            balance: result["balance"].as_str()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0),
            state: result["state"].as_str().unwrap_or("").to_string(),
            code: result["code"].as_str().unwrap_or("").to_string(),
            data: result["data"].as_str().unwrap_or("").to_string(),
        })
    }
    
    async fn run_get_method(&self, address: &str, method: &str, stack: Vec<serde_json::Value>) -> Result<serde_json::Value, RouterError> {
        self.api_call(
            "runGetMethod",
            json!({
                "address": address,
                "method": method,
                "stack": stack
            })
        ).await
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct TonAddressInfo {
    balance: u64,
    state: String,
    code: String,
    data: String,
}

#[async_trait]
impl ChainAdapter for TonAdapter {
    fn chain_name(&self) -> &str { 
        &self.chain_name 
    }
    
    fn chain_id(&self) -> &str { 
        &self.chain_id 
    }
    
    async fn translate_intent(&self, intent: &Intent) -> Result<TranslatedIntent, RouterError> {
        self.translator.translate(intent)
    }
    
    async fn verify_state(&self, proof: &[u8]) -> Result<bool, RouterError> {
        let address = String::from_utf8(proof.to_vec())
            .map_err(|e| RouterError::VerificationError(format!("Invalid address encoding: {}", e)))?;
        
        self.validate_ton_address(&address)?;
        
        // Verify address exists and is active
        let address_info = self.get_address_info(&address).await?;
        
        // Check if account is active or frozen (not uninitialized)
        Ok(address_info.state == "active" || address_info.state == "frozen")
    }
    
    async fn submit_transaction(&self, tx_data: &[u8]) -> Result<String, RouterError> {
        // Encode transaction as base64 BOC (Bag of Cells)
        let boc = general_purpose::STANDARD.encode(tx_data);
        
        // Send BOC to network
        let result = self.api_call(
            "sendBoc",
            json!({ "boc": boc })
        ).await?;
        
        let tx_hash = result["hash"].as_str()
            .ok_or_else(|| RouterError::TranslationError("No transaction hash in response".to_string()))?;
        
        Ok(tx_hash.to_string())
    }
    
    async fn query_balance(&self, address: &str, asset: &str) -> Result<u64, RouterError> {
        self.validate_ton_address(address)?;
        
        if asset.is_empty() || asset.to_uppercase() == "TON" {
            // Query native TON balance
            let address_info = self.get_address_info(address).await?;
            Ok(address_info.balance)
        } else {
            // Query Jetton (TON token standard) balance
            // Asset should be the jetton wallet address for this user
            let result = self.run_get_method(
                asset,
                "get_wallet_data",
                vec![]
            ).await?;
            
            if let Some(stack) = result["stack"].as_array() {
                // First element in stack is usually the balance
                if let Some(balance_item) = stack.first() {
                    if let Some(balance_str) = balance_item[1].as_str() {
                        return balance_str.parse::<u64>()
                            .map_err(|e| RouterError::TranslationError(format!("Failed to parse balance: {}", e)));
                    }
                }
            }
            
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[ignore]
    async fn test_ton_connection() {
        let translator = Arc::new(IntentTranslator::new());
        let adapter = TonAdapter::new(
            "https://toncenter.com/api/v2".to_string(),
            TonNetwork::Mainnet,
            translator,
        );
        assert!(adapter.is_ok());
    }
    
    #[test]
    fn test_ton_address_validation() {
        let translator = Arc::new(IntentTranslator::new());
        let adapter = TonAdapter::new(
            "https://toncenter.com/api/v2".to_string(),
            TonNetwork::Mainnet,
            translator,
        ).unwrap();
        
        // Raw format
        assert!(adapter.validate_ton_address("0:83dfd552e63729b472fcbcc8c45ebcc6691702558b68ec7527e1ba403a0f31a8").is_ok());
        
        // User-friendly format
        assert!(adapter.validate_ton_address("EQCBn2VvoGrLRLjnw5y-KD1TKh0-irF1xVJVOXXO8xMEscxT").is_ok());
        
        // Invalid
        assert!(adapter.validate_ton_address("invalid").is_err());
    }
}
