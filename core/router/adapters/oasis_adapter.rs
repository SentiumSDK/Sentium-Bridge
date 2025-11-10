// REAL Oasis Network Adapter - Production-ready implementation
// Oasis is a privacy-enabled blockchain with ParaTime architecture
use async_trait::async_trait;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use super::{Intent, RouterError};
use super::intent_translator::{IntentTranslator, TranslatedIntent};

#[derive(Debug, Deserialize)]
struct OasisAccount {
    general: GeneralAccount,
}

#[derive(Debug, Deserialize)]
struct GeneralAccount {
    balance: String,
    nonce: u64,
}

#[derive(Debug, Serialize)]
struct OasisTransaction {
    nonce: u64,
    fee: Fee,
    method: String,
    body: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct Fee {
    amount: String,
    gas: u64,
}

pub struct RealOasisAdapter {
    chain_name: String,
    chain_id: String,
    http_client: Arc<HttpClient>,
    rpc_url: String,
    translator: Arc<IntentTranslator>,
}

impl RealOasisAdapter {
    pub fn new(
        rpc_url: String,
        network: OasisNetwork,
        translator: Arc<IntentTranslator>,
    ) -> Result<Self, RouterError> {
        let chain_id = match network {
            OasisNetwork::Mainnet => "oasis-mainnet",
            OasisNetwork::Testnet => "oasis-testnet",
        };
        
        Ok(Self {
            chain_name: "oasis".to_string(),
            chain_id: chain_id.to_string(),
            http_client: Arc::new(HttpClient::new()),
            rpc_url,
            translator,
        })
    }
    
    fn validate_oasis_address(&self, address: &str) -> Result<(), RouterError> {
        // Oasis addresses are base64-encoded or bech32 (oasis1...)
        if !address.starts_with("oasis1") && address.len() != 44 {
            return Err(RouterError::TranslationError("Invalid Oasis address format".to_string()));
        }
        Ok(())
    }
    
    async fn rpc_call<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T, RouterError> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        
        let response = self.http_client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| RouterError::TranslationError(format!("HTTP request failed: {}", e)))?;
        
        let result: serde_json::Value = response.json().await
            .map_err(|e| RouterError::TranslationError(format!("Failed to parse response: {}", e)))?;
        
        serde_json::from_value(result["result"].clone())
            .map_err(|e| RouterError::TranslationError(format!("Failed to deserialize result: {}", e)))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum OasisNetwork {
    Mainnet,
    Testnet,
}

#[async_trait]
impl super::chain_adapter::ChainAdapter for RealOasisAdapter {
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
        if proof.len() < 32 {
            return Err(RouterError::VerificationError("Proof too short".to_string()));
        }
        
        // Verify Oasis state root
        let state_root = &proof[..32];
        
        // Query latest block
        let block: serde_json::Value = self.rpc_call(
            "consensus.GetBlock",
            json!({"height": "latest"}),
        ).await?;
        
        // Verify state root matches
        if let Some(header_state_root) = block.get("header").and_then(|h| h.get("state_root")) {
            let header_root_bytes = hex::decode(header_state_root.as_str().unwrap_or(""))
                .map_err(|e| RouterError::VerificationError(format!("Failed to decode state root: {}", e)))?;
            
            return Ok(state_root == &header_root_bytes[..]);
        }
        
        Ok(false)
    }
    
    async fn submit_transaction(&self, tx_data: &[u8]) -> Result<String, RouterError> {
        // Submit transaction to Oasis network
        let tx_cbor = base64::encode(tx_data);
        
        let result: serde_json::Value = self.rpc_call(
            "consensus.SubmitTx",
            json!({"data": tx_cbor}),
        ).await?;
        
        let tx_hash = result.as_str()
            .ok_or_else(|| RouterError::TranslationError("No transaction hash in response".to_string()))?;
        
        Ok(tx_hash.to_string())
    }
    
    async fn query_balance(&self, address: &str, _asset: &str) -> Result<u64, RouterError> {
        self.validate_oasis_address(address)?;
        
        // Query account balance
        let account: OasisAccount = self.rpc_call(
            "consensus.GetAccount",
            json!({
                "height": "latest",
                "address": address,
            }),
        ).await?;
        
        // Parse balance (in base units)
        let balance = account.general.balance.parse::<u64>()
            .map_err(|e| RouterError::TranslationError(format!("Failed to parse balance: {}", e)))?;
        
        Ok(balance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_oasis_adapter_creation() {
        let translator = Arc::new(IntentTranslator::new());
        let adapter = RealOasisAdapter::new(
            "https://testnet.grpc.oasis.dev".to_string(),
            OasisNetwork::Testnet,
            translator,
        );
        assert!(adapter.is_ok());
    }
    
    #[test]
    fn test_validate_oasis_address() {
        let translator = Arc::new(IntentTranslator::new());
        let adapter = RealOasisAdapter::new(
            "https://testnet.grpc.oasis.dev".to_string(),
            OasisNetwork::Mainnet,
            translator,
        ).unwrap();
        
        // Valid bech32 address
        assert!(adapter.validate_oasis_address("oasis1qrvsa8ukfw3p6kw2vcs0fk9t59mceqq7fyttwqgx").is_ok());
        
        // Invalid addresses
        assert!(adapter.validate_oasis_address("invalid").is_err());
        assert!(adapter.validate_oasis_address("0x1234").is_err());
    }
}
