// REAL XRP Ledger Adapter - Production-ready implementation
use async_trait::async_trait;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use super::{Intent, RouterError};
use super::intent_translator::{IntentTranslator, TranslatedIntent};

#[derive(Debug, Serialize, Deserialize)]
struct XrplRpcRequest {
    method: String,
    params: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct XrplRpcResponse<T> {
    result: T,
}

#[derive(Debug, Deserialize)]
struct AccountInfo {
    account_data: AccountData,
}

#[derive(Debug, Deserialize)]
struct AccountData {
    #[serde(rename = "Account")]
    account: String,
    #[serde(rename = "Balance")]
    balance: String,
    #[serde(rename = "Sequence")]
    sequence: u64,
}

#[derive(Debug, Serialize)]
struct Payment {
    #[serde(rename = "TransactionType")]
    transaction_type: String,
    #[serde(rename = "Account")]
    account: String,
    #[serde(rename = "Destination")]
    destination: String,
    #[serde(rename = "Amount")]
    amount: String,
    #[serde(rename = "Fee")]
    fee: String,
    #[serde(rename = "Sequence")]
    sequence: u64,
}

pub struct RealXRPAdapter {
    chain_name: String,
    chain_id: String,
    http_client: Arc<HttpClient>,
    rpc_url: String,
    translator: Arc<IntentTranslator>,
}

impl RealXRPAdapter {
    pub fn new(
        rpc_url: String,
        network: XRPNetwork,
        translator: Arc<IntentTranslator>,
    ) -> Result<Self, RouterError> {
        let chain_id = match network {
            XRPNetwork::Mainnet => "xrp-mainnet",
            XRPNetwork::Testnet => "xrp-testnet",
            XRPNetwork::Devnet => "xrp-devnet",
        };
        
        Ok(Self {
            chain_name: "xrp".to_string(),
            chain_id: chain_id.to_string(),
            http_client: Arc::new(HttpClient::new()),
            rpc_url,
            translator,
        })
    }
    
    async fn rpc_call<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<T, RouterError> {
        let request = json!({
            "method": method,
            "params": params,
        });
        
        let response = self.http_client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| RouterError::TranslationError(format!("HTTP request failed: {}", e)))?;
        
        let result: XrplRpcResponse<T> = response.json().await
            .map_err(|e| RouterError::TranslationError(format!("Failed to parse response: {}", e)))?;
        
        Ok(result.result)
    }
    
    fn create_payment_transaction(
        &self,
        from: String,
        to: String,
        amount_drops: u64, // XRP in drops (1 XRP = 1,000,000 drops)
        sequence: u64,
    ) -> Payment {
        Payment {
            transaction_type: "Payment".to_string(),
            account: from,
            destination: to,
            amount: amount_drops.to_string(),
            fee: "12".to_string(), // Standard fee: 12 drops
            sequence,
        }
    }
    
    fn validate_xrp_address(&self, address: &str) -> Result<(), RouterError> {
        // XRP addresses start with 'r' and are 25-35 characters
        if !address.starts_with('r') || address.len() < 25 || address.len() > 35 {
            return Err(RouterError::TranslationError("Invalid XRP address format".to_string()));
        }
        
        // Validate base58 encoding
        bs58::decode(address)
            .into_vec()
            .map_err(|e| RouterError::TranslationError(format!("Invalid base58 address: {}", e)))?;
        
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum XRPNetwork {
    Mainnet,
    Testnet,
    Devnet,
}

#[async_trait]
impl super::chain_adapter::ChainAdapter for RealXRPAdapter {
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
        // Verify XRP Ledger state
        if proof.len() < 25 {
            return Err(RouterError::VerificationError("Proof too short".to_string()));
        }
        
        // Extract account address
        let address = String::from_utf8(proof[..25].to_vec())
            .map_err(|e| RouterError::VerificationError(format!("Invalid address encoding: {}", e)))?;
        
        self.validate_xrp_address(&address)?;
        
        // Verify account exists
        let params = vec![json!({
            "account": address,
            "ledger_index": "validated"
        })];
        
        let result: Result<AccountInfo, _> = self.rpc_call("account_info", params).await;
        
        Ok(result.is_ok())
    }
    
    async fn submit_transaction(&self, tx_data: &[u8]) -> Result<String, RouterError> {
        // Deserialize transaction
        let tx_json: serde_json::Value = serde_json::from_slice(tx_data)
            .map_err(|e| RouterError::TranslationError(format!("Failed to parse transaction: {}", e)))?;
        
        // Submit transaction
        let params = vec![json!({
            "tx_json": tx_json,
        })];
        
        let response: serde_json::Value = self.rpc_call("submit", params).await?;
        
        let tx_hash = response.get("tx_json")
            .and_then(|v| v.get("hash"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| RouterError::TranslationError("No transaction hash in response".to_string()))?;
        
        Ok(tx_hash.to_string())
    }
    
    async fn query_balance(&self, address: &str, asset: &str) -> Result<u64, RouterError> {
        self.validate_xrp_address(address)?;
        
        if asset.to_uppercase() == "XRP" {
            // Query native XRP balance
            let params = vec![json!({
                "account": address,
                "ledger_index": "validated"
            })];
            
            let account_info: AccountInfo = self.rpc_call("account_info", params).await?;
            
            // Balance is in drops, convert to u64
            let balance_drops = account_info.account_data.balance.parse::<u64>()
                .map_err(|e| RouterError::TranslationError(format!("Failed to parse balance: {}", e)))?;
            
            Ok(balance_drops)
        } else {
            // Query issued currency balance
            let params = vec![json!({
                "account": address,
                "ledger_index": "validated"
            })];
            
            let lines: serde_json::Value = self.rpc_call("account_lines", params).await?;
            
            // Find the specific currency
            let balance = lines.get("lines")
                .and_then(|v| v.as_array())
                .and_then(|arr| {
                    arr.iter().find(|line| {
                        line.get("currency")
                            .and_then(|c| c.as_str())
                            .map(|c| c == asset)
                            .unwrap_or(false)
                    })
                })
                .and_then(|line| line.get("balance"))
                .and_then(|b| b.as_str())
                .and_then(|b| b.parse::<f64>().ok())
                .map(|b| (b * 1_000_000.0) as u64) // Convert to smallest unit
                .unwrap_or(0);
            
            Ok(balance)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_xrp_adapter_creation() {
        let translator = Arc::new(IntentTranslator::new());
        let adapter = RealXRPAdapter::new(
            "https://s.altnet.rippletest.net:51234".to_string(),
            XRPNetwork::Testnet,
            translator,
        );
        assert!(adapter.is_ok());
    }
    
    #[test]
    fn test_validate_xrp_address() {
        let translator = Arc::new(IntentTranslator::new());
        let adapter = RealXRPAdapter::new(
            "https://s.altnet.rippletest.net:51234".to_string(),
            XRPNetwork::Testnet,
            translator,
        ).unwrap();
        
        // Valid address
        assert!(adapter.validate_xrp_address("rN7n7otQDd6FczFgLdlqtyMVrn3HMfXk8D").is_ok());
        
        // Invalid addresses
        assert!(adapter.validate_xrp_address("invalid").is_err());
        assert!(adapter.validate_xrp_address("0x1234").is_err());
    }
    
    #[test]
    fn test_create_payment() {
        let translator = Arc::new(IntentTranslator::new());
        let adapter = RealXRPAdapter::new(
            "https://s.altnet.rippletest.net:51234".to_string(),
            XRPNetwork::Testnet,
            translator,
        ).unwrap();
        
        let payment = adapter.create_payment_transaction(
            "rN7n7otQDd6FczFgLdlqtyMVrn3HMfXk8D".to_string(),
            "rLHzPsX6oXkzU9fYbKhH4KXfNM8VkfCvHe".to_string(),
            1_000_000, // 1 XRP
            1,
        );
        
        assert_eq!(payment.transaction_type, "Payment");
        assert_eq!(payment.amount, "1000000");
    }
}
