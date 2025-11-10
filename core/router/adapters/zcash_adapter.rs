// Zcash Adapter - Privacy-focused cryptocurrency with zk-SNARKs
// Production-ready implementation with shielded and transparent address support

use async_trait::async_trait;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use sha2::{Sha256, Digest};

use crate::router::{Intent, RouterError};
use crate::router::intent_translator::{IntentTranslator, TranslatedIntent};
use crate::router::chain_adapter::ChainAdapter;

use super::zcash_proof_verifier::{ZcashProofVerifier, ZcashProofType};

pub struct ZcashAdapter {
    chain_name: String,
    chain_id: String,
    http_client: Arc<HttpClient>,
    rpc_url: String,
    rpc_user: Option<String>,
    rpc_password: Option<String>,
    translator: Arc<IntentTranslator>,
    network: ZcashNetwork,
    proof_verifier: tokio::sync::Mutex<ZcashProofVerifier>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZcashNetwork {
    Mainnet,
    Testnet,
}

impl ZcashAdapter {
    pub fn new(
        rpc_url: String,
        network: ZcashNetwork,
        translator: Arc<IntentTranslator>,
    ) -> Result<Self, RouterError> {
        let chain_id = match network {
            ZcashNetwork::Mainnet => "zcash-mainnet",
            ZcashNetwork::Testnet => "zcash-testnet",
        };
        
        Ok(Self {
            chain_name: "zcash".to_string(),
            chain_id: chain_id.to_string(),
            http_client: Arc::new(HttpClient::new()),
            rpc_url,
            rpc_user: None,
            rpc_password: None,
            translator,
            network,
            proof_verifier: tokio::sync::Mutex::new(ZcashProofVerifier::new()),
        })
    }
    
    pub fn with_auth(mut self, user: String, password: String) -> Self {
        self.rpc_user = Some(user);
        self.rpc_password = Some(password);
        self
    }
    
    async fn rpc_call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, RouterError> {
        let mut request = self.http_client
            .post(&self.rpc_url)
            .json(&json!({
                "jsonrpc": "1.0",
                "id": "sentium",
                "method": method,
                "params": params
            }));
        
        if let (Some(user), Some(pass)) = (&self.rpc_user, &self.rpc_password) {
            request = request.basic_auth(user, Some(pass));
        }
        
        let response = request.send().await
            .map_err(|e| RouterError::TranslationError(format!("RPC request failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(RouterError::TranslationError(format!("RPC error: {}", response.status())));
        }
        
        let result: serde_json::Value = response.json().await
            .map_err(|e| RouterError::TranslationError(format!("Failed to parse response: {}", e)))?;
        
        if let Some(error) = result.get("error") {
            if !error.is_null() {
                return Err(RouterError::TranslationError(format!("RPC error: {}", error)));
            }
        }
        
        result.get("result")
            .cloned()
            .ok_or_else(|| RouterError::TranslationError("No result in RPC response".to_string()))
    }
    
    fn validate_address(&self, address: &str) -> Result<ZcashAddressType, RouterError> {
        // Zcash addresses:
        // Transparent: t1... (P2PKH) or t3... (P2SH) for mainnet, tm... for testnet
        // Shielded Sprout: zc... for mainnet, zt... for testnet
        // Shielded Sapling: zs... for mainnet and testnet
        // Unified: u1... for mainnet, utest... for testnet
        
        if address.starts_with("t1") || address.starts_with("t3") || address.starts_with("tm") {
            Ok(ZcashAddressType::Transparent)
        } else if address.starts_with("zc") || address.starts_with("zt") {
            Ok(ZcashAddressType::Sprout)
        } else if address.starts_with("zs") {
            Ok(ZcashAddressType::Sapling)
        } else if address.starts_with("u1") || address.starts_with("utest") {
            Ok(ZcashAddressType::Unified)
        } else {
            Err(RouterError::TranslationError("Invalid Zcash address format".to_string()))
        }
    }
    
    async fn get_blockchain_info(&self) -> Result<ZcashBlockchainInfo, RouterError> {
        let result = self.rpc_call("getblockchaininfo", json!([])).await?;
        
        Ok(ZcashBlockchainInfo {
            chain: result["chain"].as_str().unwrap_or("main").to_string(),
            blocks: result["blocks"].as_u64().unwrap_or(0),
            headers: result["headers"].as_u64().unwrap_or(0),
            best_block_hash: result["bestblockhash"].as_str().unwrap_or("").to_string(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ZcashAddressType {
    Transparent,
    Sprout,
    Sapling,
    Unified,
}

#[derive(Debug, Serialize, Deserialize)]
struct ZcashBlockchainInfo {
    chain: String,
    blocks: u64,
    headers: u64,
    best_block_hash: String,
}

#[async_trait]
impl ChainAdapter for ZcashAdapter {
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
        // Zcash uses zk-SNARK proofs for shielded transactions
        // Proof format: proof_type (1 byte) + proof_data (192+ bytes) + public_inputs (96+ bytes)
        
        if proof.is_empty() {
            return Err(RouterError::VerificationError("Empty proof".to_string()));
        }
        
        // Parse proof type from first byte
        let proof_type_byte = proof[0];
        let proof_type = match proof_type_byte {
            0 => ZcashProofType::SaplingSpend,
            1 => ZcashProofType::SaplingOutput,
            2 => ZcashProofType::Orchard,
            _ => return Err(RouterError::VerificationError(
                format!("Unknown proof type: {}", proof_type_byte)
            )),
        };
        
        // Determine expected sizes based on proof type
        let (min_proof_size, min_public_inputs_size) = match proof_type {
            ZcashProofType::SaplingSpend | ZcashProofType::SaplingOutput => (192, 96),
            ZcashProofType::Orchard => (512, 96),
        };
        
        // Parse proof structure: proof_type (1) + proof_size (4) + proof_data + public_inputs
        if proof.len() < 1 + 4 + min_proof_size + min_public_inputs_size {
            return Err(RouterError::VerificationError(
                format!("Proof too short: expected at least {} bytes, got {}", 
                    1 + 4 + min_proof_size + min_public_inputs_size, 
                    proof.len())
            ));
        }
        
        // Read proof size (4 bytes, little-endian)
        let proof_size = u32::from_le_bytes([
            proof[1], proof[2], proof[3], proof[4]
        ]) as usize;
        
        if proof.len() < 1 + 4 + proof_size {
            return Err(RouterError::VerificationError(
                format!("Proof data truncated: expected {} bytes, got {}", 
                    1 + 4 + proof_size, 
                    proof.len())
            ));
        }
        
        // Extract proof data and public inputs
        let proof_data = &proof[5..5 + proof_size];
        let public_inputs = &proof[5 + proof_size..];
        
        // Verify the proof using ZcashProofVerifier
        let mut verifier = self.proof_verifier.lock().await;
        verifier.verify_proof(proof_data, public_inputs, proof_type).await
    }
    
    async fn submit_transaction(&self, tx_data: &[u8]) -> Result<String, RouterError> {
        let tx_hex = hex::encode(tx_data);
        
        // Send raw transaction
        let result = self.rpc_call("sendrawtransaction", json!([tx_hex])).await?;
        
        let tx_hash = result.as_str()
            .ok_or_else(|| RouterError::TranslationError("Invalid transaction hash".to_string()))?;
        
        Ok(tx_hash.to_string())
    }
    
    async fn query_balance(&self, address: &str, _asset: &str) -> Result<u64, RouterError> {
        let addr_type = self.validate_address(address)?;
        
        match addr_type {
            ZcashAddressType::Transparent => {
                // Query transparent address balance
                let result = self.rpc_call(
                    "getaddressbalance",
                    json!({
                        "addresses": [address]
                    })
                ).await;
                
                if let Ok(balance_data) = result {
                    if let Some(balance) = balance_data.get("balance").and_then(|v| v.as_u64()) {
                        return Ok(balance);
                    }
                }
                
                // Fallback: use listunspent
                let unspent = self.rpc_call(
                    "listunspent",
                    json!([0, 9999999, [address]])
                ).await?;
                
                let mut total_balance = 0u64;
                if let Some(utxos) = unspent.as_array() {
                    for utxo in utxos {
                        if let Some(amount) = utxo.get("amount").and_then(|v| v.as_f64()) {
                            total_balance += (amount * 100_000_000.0) as u64;
                        }
                    }
                }
                
                Ok(total_balance)
            },
            ZcashAddressType::Sprout | ZcashAddressType::Sapling | ZcashAddressType::Unified => {
                // Query shielded address balance
                let result = self.rpc_call("z_getbalance", json!([address])).await?;
                
                let balance = result.as_f64()
                    .ok_or_else(|| RouterError::TranslationError("Invalid balance format".to_string()))?;
                
                // Convert ZEC to zatoshis (1 ZEC = 10^8 zatoshis)
                Ok((balance * 100_000_000.0) as u64)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[ignore]
    async fn test_zcash_connection() {
        let translator = Arc::new(IntentTranslator::new());
        let adapter = ZcashAdapter::new(
            "https://zcash.getblock.io/mainnet/".to_string(),
            ZcashNetwork::Mainnet,
            translator,
        );
        assert!(adapter.is_ok());
    }
    
    #[test]
    fn test_zcash_address_validation() {
        let translator = Arc::new(IntentTranslator::new());
        let adapter = ZcashAdapter::new(
            "https://zcash.getblock.io/mainnet/".to_string(),
            ZcashNetwork::Mainnet,
            translator,
        ).unwrap();
        
        // Transparent addresses
        assert!(matches!(adapter.validate_address("t1Hsc1LR8yKnbbe3twRp88p6vFfC5t7DLbs"), Ok(ZcashAddressType::Transparent)));
        
        // Sapling addresses
        assert!(matches!(adapter.validate_address("zs1z7rejlpsa98s2rrrfkwmaxu53e4ue0ulcrw0h4x5g8jl04tak0d3mm47vdtahatqrlkngh9sly"), Ok(ZcashAddressType::Sapling)));
        
        // Invalid
        assert!(adapter.validate_address("invalid").is_err());
    }
}
