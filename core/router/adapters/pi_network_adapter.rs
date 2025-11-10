// Pi Network Adapter - Mobile-first cryptocurrency with Stellar Consensus Protocol
// Production-ready implementation

use async_trait::async_trait;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use base64::{Engine as _, engine::general_purpose};

use crate::router::{Intent, RouterError};
use crate::router::intent_translator::{IntentTranslator, TranslatedIntent};
use crate::router::chain_adapter::ChainAdapter;

pub struct PiNetworkAdapter {
    chain_name: String,
    chain_id: String,
    http_client: Arc<HttpClient>,
    api_url: String,
    horizon_url: String,
    translator: Arc<IntentTranslator>,
    api_key: Option<String>,
}

impl PiNetworkAdapter {
    pub fn new(
        api_url: String,
        horizon_url: String,
        translator: Arc<IntentTranslator>,
    ) -> Result<Self, RouterError> {
        Ok(Self {
            chain_name: "pi-network".to_string(),
            chain_id: "pi-mainnet".to_string(),
            http_client: Arc::new(HttpClient::new()),
            api_url,
            horizon_url,
            translator,
            api_key: None,
        })
    }
    
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }
    
    async fn api_call(&self, endpoint: &str, method: reqwest::Method) -> Result<serde_json::Value, RouterError> {
        let url = format!("{}{}", self.api_url, endpoint);
        let mut request = self.http_client.request(method, &url);
        
        if let Some(key) = &self.api_key {
            request = request.header("Authorization", format!("Key {}", key));
        }
        
        let response = request.send().await
            .map_err(|e| RouterError::TranslationError(format!("API request failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(RouterError::TranslationError(format!("API error: {}", response.status())));
        }
        
        response.json().await
            .map_err(|e| RouterError::TranslationError(format!("Failed to parse response: {}", e)))
    }
    
    async fn horizon_call(&self, endpoint: &str) -> Result<serde_json::Value, RouterError> {
        let url = format!("{}{}", self.horizon_url, endpoint);
        let response = self.http_client.get(&url).send().await
            .map_err(|e| RouterError::TranslationError(format!("Horizon request failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(RouterError::TranslationError(format!("Horizon error: {}", response.status())));
        }
        
        response.json().await
            .map_err(|e| RouterError::TranslationError(format!("Failed to parse response: {}", e)))
    }
    
    fn validate_pi_address(&self, address: &str) -> Result<(), RouterError> {
        // Pi Network uses Stellar-style addresses (G... for public keys)
        if !address.starts_with('G') || address.len() != 56 {
            return Err(RouterError::TranslationError("Invalid Pi Network address format".to_string()));
        }
        Ok(())
    }
}

#[async_trait]
impl ChainAdapter for PiNetworkAdapter {
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
        if proof.is_empty() {
            return Err(RouterError::VerificationError("Empty proof".to_string()));
        }
        
        // Verify ledger sequence from proof
        let ledger_seq = u64::from_be_bytes(
            proof[..8].try_into()
                .map_err(|_| RouterError::VerificationError("Invalid proof format".to_string()))?
        );
        
        // Get current ledger from Horizon
        let ledger_info = self.horizon_call("/ledgers?order=desc&limit=1").await?;
        
        if let Some(records) = ledger_info["_embedded"]["records"].as_array() {
            if let Some(latest) = records.first() {
                let latest_seq = latest["sequence"].as_u64().unwrap_or(0);
                // Proof should be recent (within 100 ledgers)
                return Ok(ledger_seq <= latest_seq && latest_seq - ledger_seq < 100);
            }
        }
        
        Ok(false)
    }
    
    async fn submit_transaction(&self, tx_data: &[u8]) -> Result<String, RouterError> {
        // Encode transaction as base64 (Stellar format)
        let tx_envelope = general_purpose::STANDARD.encode(tx_data);
        
        // Submit via Horizon
        let response = self.http_client
            .post(&format!("{}/transactions", self.horizon_url))
            .form(&[("tx", tx_envelope)])
            .send()
            .await
            .map_err(|e| RouterError::TranslationError(format!("Transaction submission failed: {}", e)))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(RouterError::TranslationError(format!("Transaction rejected: {}", error_text)));
        }
        
        let result: serde_json::Value = response.json().await
            .map_err(|e| RouterError::TranslationError(format!("Failed to parse response: {}", e)))?;
        
        let tx_hash = result["hash"].as_str()
            .ok_or_else(|| RouterError::TranslationError("No transaction hash in response".to_string()))?;
        
        Ok(tx_hash.to_string())
    }
    
    async fn query_balance(&self, address: &str, asset: &str) -> Result<u64, RouterError> {
        self.validate_pi_address(address)?;
        
        // Query account balances from Horizon
        let account_info = self.horizon_call(&format!("/accounts/{}", address)).await?;
        
        let balances = account_info["balances"].as_array()
            .ok_or_else(|| RouterError::TranslationError("No balances in response".to_string()))?;
        
        let target_asset = if asset.is_empty() || asset.to_uppercase() == "PI" {
            "native"
        } else {
            asset
        };
        
        for balance in balances {
            let asset_type = balance["asset_type"].as_str().unwrap_or("");
            
            if (target_asset == "native" && asset_type == "native") ||
               (target_asset != "native" && balance["asset_code"].as_str() == Some(target_asset)) {
                let balance_str = balance["balance"].as_str()
                    .ok_or_else(|| RouterError::TranslationError("Invalid balance format".to_string()))?;
                
                // Convert from decimal string to stroops (1 PI = 10^7 stroops)
                let balance_f64: f64 = balance_str.parse()
                    .map_err(|_| RouterError::TranslationError("Failed to parse balance".to_string()))?;
                
                return Ok((balance_f64 * 10_000_000.0) as u64);
            }
        }
        
        Ok(0)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct PiPayment {
    identifier: String,
    user_uid: String,
    amount: f64,
    memo: String,
    metadata: serde_json::Value,
    from_address: String,
    to_address: String,
    direction: String,
    created_at: String,
    network: String,
    status: PiPaymentStatus,
}

#[derive(Debug, Serialize, Deserialize)]
struct PiPaymentStatus {
    developer_approved: bool,
    transaction_verified: bool,
    developer_completed: bool,
    cancelled: bool,
    user_cancelled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[ignore]
    async fn test_pi_connection() {
        let translator = Arc::new(IntentTranslator::new());
        let adapter = PiNetworkAdapter::new(
            "https://api.minepi.com".to_string(),
            "https://api.mainnet.minepi.com".to_string(),
            translator,
        );
        assert!(adapter.is_ok());
    }
    
    #[test]
    fn test_pi_address_validation() {
        let translator = Arc::new(IntentTranslator::new());
        let adapter = PiNetworkAdapter::new(
            "https://api.minepi.com".to_string(),
            "https://api.mainnet.minepi.com".to_string(),
            translator,
        ).unwrap();
        
        // Valid Stellar-style address
        assert!(adapter.validate_pi_address("GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H").is_ok());
        
        // Invalid addresses
        assert!(adapter.validate_pi_address("invalid").is_err());
        assert!(adapter.validate_pi_address("GBRPYHIL").is_err());
    }
}
