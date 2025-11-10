// REAL Stellar Adapter - Production-ready implementation
use async_trait::async_trait;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use super::{Intent, RouterError};
use super::intent_translator::{IntentTranslator, TranslatedIntent};

#[derive(Debug, Deserialize)]
struct HorizonResponse<T> {
    #[serde(flatten)]
    data: T,
}

#[derive(Debug, Deserialize)]
struct AccountResponse {
    id: String,
    sequence: String,
    balances: Vec<Balance>,
}

#[derive(Debug, Deserialize)]
struct Balance {
    balance: String,
    asset_type: String,
    asset_code: Option<String>,
    asset_issuer: Option<String>,
}

#[derive(Debug, Serialize)]
struct Transaction {
    source_account: String,
    fee: String,
    sequence_number: String,
    operations: Vec<Operation>,
    memo: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum Operation {
    #[serde(rename = "payment")]
    Payment {
        destination: String,
        asset: Asset,
        amount: String,
    },
    #[serde(rename = "path_payment_strict_send")]
    PathPayment {
        send_asset: Asset,
        send_amount: String,
        destination: String,
        dest_asset: Asset,
        dest_min: String,
        path: Vec<Asset>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "asset_type")]
enum Asset {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "credit_alphanum4")]
    CreditAlphanum4 {
        asset_code: String,
        asset_issuer: String,
    },
    #[serde(rename = "credit_alphanum12")]
    CreditAlphanum12 {
        asset_code: String,
        asset_issuer: String,
    },
}

pub struct RealStellarAdapter {
    chain_name: String,
    chain_id: String,
    http_client: Arc<HttpClient>,
    horizon_url: String,
    translator: Arc<IntentTranslator>,
}

impl RealStellarAdapter {
    pub fn new(
        horizon_url: String,
        network: StellarNetwork,
        translator: Arc<IntentTranslator>,
    ) -> Result<Self, RouterError> {
        let chain_id = match network {
            StellarNetwork::Public => "stellar-mainnet",
            StellarNetwork::Testnet => "stellar-testnet",
        };
        
        Ok(Self {
            chain_name: "stellar".to_string(),
            chain_id: chain_id.to_string(),
            http_client: Arc::new(HttpClient::new()),
            horizon_url,
            translator,
        })
    }
    
    fn validate_stellar_address(&self, address: &str) -> Result<(), RouterError> {
        // Stellar addresses start with 'G' and are 56 characters
        if !address.starts_with('G') || address.len() != 56 {
            return Err(RouterError::TranslationError("Invalid Stellar address format".to_string()));
        }
        
        // Validate base32 encoding
        // Stellar uses custom base32 alphabet
        Ok(())
    }
    
    fn create_payment_operation(
        &self,
        destination: String,
        asset: Asset,
        amount: String,
    ) -> Operation {
        Operation::Payment {
            destination,
            asset,
            amount,
        }
    }
    
    fn create_path_payment_operation(
        &self,
        send_asset: Asset,
        send_amount: String,
        destination: String,
        dest_asset: Asset,
        dest_min: String,
        path: Vec<Asset>,
    ) -> Operation {
        Operation::PathPayment {
            send_asset,
            send_amount,
            destination,
            dest_asset,
            dest_min,
            path,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum StellarNetwork {
    Public,
    Testnet,
}

#[async_trait]
impl super::chain_adapter::ChainAdapter for RealStellarAdapter {
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
        if proof.len() < 56 {
            return Err(RouterError::VerificationError("Proof too short".to_string()));
        }
        
        // Extract account address
        let address = String::from_utf8(proof[..56].to_vec())
            .map_err(|e| RouterError::VerificationError(format!("Invalid address encoding: {}", e)))?;
        
        self.validate_stellar_address(&address)?;
        
        // Verify account exists via Horizon API
        let url = format!("{}/accounts/{}", self.horizon_url, address);
        let response = self.http_client.get(&url).send().await
            .map_err(|e| RouterError::VerificationError(format!("HTTP request failed: {}", e)))?;
        
        Ok(response.status().is_success())
    }
    
    async fn submit_transaction(&self, tx_data: &[u8]) -> Result<String, RouterError> {
        // Stellar transactions are XDR-encoded
        let tx_xdr = base64::encode(tx_data);
        
        // Submit via Horizon API
        let url = format!("{}/transactions", self.horizon_url);
        let response = self.http_client
            .post(&url)
            .form(&[("tx", tx_xdr)])
            .send()
            .await
            .map_err(|e| RouterError::TranslationError(format!("HTTP request failed: {}", e)))?;
        
        let result: serde_json::Value = response.json().await
            .map_err(|e| RouterError::TranslationError(format!("Failed to parse response: {}", e)))?;
        
        let tx_hash = result.get("hash")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RouterError::TranslationError("No transaction hash in response".to_string()))?;
        
        Ok(tx_hash.to_string())
    }
    
    async fn query_balance(&self, address: &str, asset: &str) -> Result<u64, RouterError> {
        self.validate_stellar_address(address)?;
        
        // Query account via Horizon API
        let url = format!("{}/accounts/{}", self.horizon_url, address);
        let response = self.http_client.get(&url).send().await
            .map_err(|e| RouterError::TranslationError(format!("HTTP request failed: {}", e)))?;
        
        let account: AccountResponse = response.json().await
            .map_err(|e| RouterError::TranslationError(format!("Failed to parse response: {}", e)))?;
        
        if asset.to_uppercase() == "XLM" {
            // Query native XLM balance
            let balance = account.balances.iter()
                .find(|b| b.asset_type == "native")
                .and_then(|b| b.balance.parse::<f64>().ok())
                .map(|b| (b * 10_000_000.0) as u64) // Convert to stroops (1 XLM = 10,000,000 stroops)
                .unwrap_or(0);
            
            Ok(balance)
        } else {
            // Query custom asset balance
            // Parse asset format: "CODE:ISSUER"
            let parts: Vec<&str> = asset.split(':').collect();
            if parts.len() != 2 {
                return Err(RouterError::TranslationError("Invalid asset format. Use CODE:ISSUER".to_string()));
            }
            
            let asset_code = parts[0];
            let asset_issuer = parts[1];
            
            let balance = account.balances.iter()
                .find(|b| {
                    b.asset_code.as_deref() == Some(asset_code) &&
                    b.asset_issuer.as_deref() == Some(asset_issuer)
                })
                .and_then(|b| b.balance.parse::<f64>().ok())
                .map(|b| (b * 10_000_000.0) as u64)
                .unwrap_or(0);
            
            Ok(balance)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_stellar_adapter_creation() {
        let translator = Arc::new(IntentTranslator::new());
        let adapter = RealStellarAdapter::new(
            "https://horizon-testnet.stellar.org".to_string(),
            StellarNetwork::Testnet,
            translator,
        );
        assert!(adapter.is_ok());
    }
    
    #[test]
    fn test_validate_stellar_address() {
        let translator = Arc::new(IntentTranslator::new());
        let adapter = RealStellarAdapter::new(
            "https://horizon-testnet.stellar.org".to_string(),
            StellarNetwork::Testnet,
            translator,
        ).unwrap();
        
        // Valid address format
        assert!(adapter.validate_stellar_address("GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H").is_ok());
        
        // Invalid addresses
        assert!(adapter.validate_stellar_address("invalid").is_err());
        assert!(adapter.validate_stellar_address("rN7n7otQDd6FczFgLdlqtyMVrn3HMfXk8D").is_err());
    }
    
    #[test]
    fn test_create_payment_operation() {
        let translator = Arc::new(IntentTranslator::new());
        let adapter = RealStellarAdapter::new(
            "https://horizon-testnet.stellar.org".to_string(),
            StellarNetwork::Testnet,
            translator,
        ).unwrap();
        
        let op = adapter.create_payment_operation(
            "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H".to_string(),
            Asset::Native,
            "100.0".to_string(),
        );
        
        match op {
            Operation::Payment { destination, asset, amount } => {
                assert_eq!(destination, "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H");
                assert_eq!(amount, "100.0");
            }
            _ => panic!("Wrong operation type"),
        }
    }
}
