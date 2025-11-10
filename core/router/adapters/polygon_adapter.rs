// Polygon (MATIC) Adapter - EVM-compatible sidechain with high throughput
// Production-ready implementation with full EIP-1559 support

use async_trait::async_trait;
use ethers::prelude::*;
use ethers::providers::{Provider, Ws, Http, Middleware};
use ethers::types::{Address, U256, TransactionRequest, Bytes, BlockNumber, Transaction, H256};
use ethers::signers::{LocalWallet, Signer};
use std::sync::Arc;
use std::str::FromStr;
use tokio::sync::RwLock;

use crate::router::{Intent, RouterError};
use crate::router::intent_translator::{IntentTranslator, TranslatedIntent};
use crate::router::chain_adapter::ChainAdapter;

pub struct PolygonAdapter {
    chain_name: String,
    chain_id: u64,
    provider: Arc<Provider<Ws>>,
    http_provider: Arc<Provider<Http>>,
    translator: Arc<IntentTranslator>,
    wallet: Option<Arc<LocalWallet>>,
    nonce_manager: Arc<RwLock<u64>>,
}

impl PolygonAdapter {
    pub async fn new(
        ws_url: String,
        http_url: String,
        chain_id: u64, // 137 for mainnet, 80001 for Mumbai testnet, 80002 for Amoy testnet
        translator: Arc<IntentTranslator>,
    ) -> Result<Self, RouterError> {
        let ws = Ws::connect(&ws_url).await
            .map_err(|e| RouterError::TranslationError(format!("WebSocket connection failed: {}", e)))?;
        let provider = Provider::new(ws);
        
        let http_provider = Provider::<Http>::try_from(&http_url)
            .map_err(|e| RouterError::TranslationError(format!("HTTP connection failed: {}", e)))?;
        
        Ok(Self {
            chain_name: "polygon".to_string(),
            chain_id,
            provider: Arc::new(provider),
            http_provider: Arc::new(http_provider),
            translator,
            wallet: None,
            nonce_manager: Arc::new(RwLock::new(0)),
        })
    }
    
    pub fn with_wallet(mut self, private_key: &str) -> Result<Self, RouterError> {
        let wallet = private_key.parse::<LocalWallet>()
            .map_err(|e| RouterError::TranslationError(format!("Invalid private key: {}", e)))?
            .with_chain_id(self.chain_id);
        self.wallet = Some(Arc::new(wallet));
        Ok(self)
    }
    
    async fn get_next_nonce(&self, address: Address) -> Result<U256, RouterError> {
        let nonce = self.http_provider.get_transaction_count(address, None).await
            .map_err(|e| RouterError::TranslationError(format!("Failed to get nonce: {}", e)))?;
        
        let mut nonce_lock = self.nonce_manager.write().await;
        let current = nonce.as_u64();
        if current > *nonce_lock {
            *nonce_lock = current;
        }
        let next = *nonce_lock;
        *nonce_lock += 1;
        
        Ok(U256::from(next))
    }
    
    async fn estimate_polygon_gas(&self, tx: &TransactionRequest) -> Result<(U256, U256), RouterError> {
        // Polygon uses EIP-1559 with maxPriorityFeePerGas and maxFeePerGas
        let gas_limit = self.provider.estimate_gas(&tx.clone().into(), None).await
            .map_err(|e| RouterError::TranslationError(format!("Gas estimation failed: {}", e)))?;
        
        // Get current base fee from latest block
        let block = self.provider.get_block(BlockNumber::Latest).await
            .map_err(|e| RouterError::TranslationError(format!("Failed to get block: {}", e)))?
            .ok_or_else(|| RouterError::TranslationError("Block not found".to_string()))?;
        
        let base_fee = block.base_fee_per_gas.unwrap_or(U256::from(30_000_000_000u64)); // 30 gwei default
        
        // Priority fee for Polygon (typically 30-50 gwei)
        let priority_fee = U256::from(35_000_000_000u64); // 35 gwei
        
        // Max fee = base fee * 2 + priority fee (for volatility)
        let max_fee = base_fee * 2 + priority_fee;
        
        Ok((gas_limit, max_fee))
    }
}

#[async_trait]
impl ChainAdapter for PolygonAdapter {
    fn chain_name(&self) -> &str { 
        &self.chain_name 
    }
    
    fn chain_id(&self) -> &str {
        match self.chain_id {
            137 => "polygon-mainnet",
            80001 => "polygon-mumbai",
            80002 => "polygon-amoy",
            _ => "polygon-unknown",
        }
    }
    
    async fn translate_intent(&self, intent: &Intent) -> Result<TranslatedIntent, RouterError> {
        self.translator.translate(intent)
    }
    
    async fn verify_state(&self, proof: &[u8]) -> Result<bool, RouterError> {
        if proof.len() < 32 {
            return Err(RouterError::VerificationError("Proof too short".to_string()));
        }
        
        let state_root = &proof[..32];
        let block = self.provider.get_block(BlockNumber::Latest).await
            .map_err(|e| RouterError::VerificationError(format!("Failed to get block: {}", e)))?
            .ok_or_else(|| RouterError::VerificationError("Block not found".to_string()))?;
        
        Ok(state_root == block.state_root.as_bytes())
    }
    
    async fn submit_transaction(&self, tx_data: &[u8]) -> Result<String, RouterError> {
        if tx_data.len() < 20 {
            return Err(RouterError::TranslationError("Invalid transaction data".to_string()));
        }
        
        let wallet = self.wallet.as_ref()
            .ok_or_else(|| RouterError::TranslationError("Wallet not configured".to_string()))?;
        
        let to_bytes: [u8; 20] = tx_data[..20].try_into()
            .map_err(|_| RouterError::TranslationError("Invalid address".to_string()))?;
        let to = Address::from(to_bytes);
        let data = Bytes::from(tx_data[20..].to_vec());
        
        let from = wallet.address();
        let nonce = self.get_next_nonce(from).await?;
        
        let mut tx = TransactionRequest::new()
            .to(to)
            .from(from)
            .data(data)
            .nonce(nonce)
            .chain_id(self.chain_id);
        
        let (gas_limit, max_fee) = self.estimate_polygon_gas(&tx).await?;
        let priority_fee = U256::from(35_000_000_000u64);
        
        tx = tx.gas(gas_limit)
            .max_fee_per_gas(max_fee)
            .max_priority_fee_per_gas(priority_fee);
        
        // Sign transaction
        let signature = wallet.sign_transaction(&tx.clone().into()).await
            .map_err(|e| RouterError::TranslationError(format!("Failed to sign transaction: {}", e)))?;
        
        // Send raw transaction
        let signed_tx = tx.rlp_signed(&signature);
        let pending_tx = self.provider.send_raw_transaction(signed_tx).await
            .map_err(|e| RouterError::TranslationError(format!("Failed to send transaction: {}", e)))?;
        
        Ok(format!("{:?}", pending_tx.tx_hash()))
    }
    
    async fn query_balance(&self, address: &str, asset: &str) -> Result<u64, RouterError> {
        let addr = Address::from_str(address)
            .map_err(|e| RouterError::TranslationError(format!("Invalid address: {}", e)))?;
        
        if asset.is_empty() || asset.to_uppercase() == "MATIC" || asset.to_uppercase() == "POL" {
            let balance = self.http_provider.get_balance(addr, None).await
                .map_err(|e| RouterError::TranslationError(format!("Balance query failed: {}", e)))?;
            Ok(balance.as_u64())
        } else {
            // Query ERC20 token balance
            let token_addr = Address::from_str(asset)
                .map_err(|e| RouterError::TranslationError(format!("Invalid token address: {}", e)))?;
            
            let function = ethabi::Function {
                name: "balanceOf".to_string(),
                inputs: vec![
                    ethabi::Param {
                        name: "account".to_string(),
                        kind: ethabi::ParamType::Address,
                        internal_type: None,
                    },
                ],
                outputs: vec![
                    ethabi::Param {
                        name: "balance".to_string(),
                        kind: ethabi::ParamType::Uint(256),
                        internal_type: None,
                    },
                ],
                constant: Some(true),
                state_mutability: ethabi::StateMutability::View,
            };
            
            let call_data = function.encode_input(&[ethabi::Token::Address(addr.into())])
                .map_err(|e| RouterError::TranslationError(format!("ABI encoding failed: {}", e)))?;
            
            let tx = TransactionRequest::new()
                .to(token_addr)
                .data(Bytes::from(call_data));
            
            let result = self.http_provider.call(&tx.into(), None).await
                .map_err(|e| RouterError::TranslationError(format!("Contract call failed: {}", e)))?;
            
            let tokens = function.decode_output(&result)
                .map_err(|e| RouterError::TranslationError(format!("ABI decoding failed: {}", e)))?;
            
            if let Some(ethabi::Token::Uint(balance)) = tokens.first() {
                Ok(balance.as_u64())
            } else {
                Err(RouterError::TranslationError("Invalid balance response".to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[ignore]
    async fn test_polygon_connection() {
        let translator = Arc::new(IntentTranslator::new());
        let adapter = PolygonAdapter::new(
            "wss://polygon-mainnet.g.alchemy.com/v2/demo".to_string(),
            "https://polygon-mainnet.g.alchemy.com/v2/demo".to_string(),
            137,
            translator,
        ).await;
        assert!(adapter.is_ok());
    }
    
    #[tokio::test]
    #[ignore]
    async fn test_polygon_balance_query() {
        let translator = Arc::new(IntentTranslator::new());
        let adapter = PolygonAdapter::new(
            "wss://polygon-mainnet.g.alchemy.com/v2/demo".to_string(),
            "https://polygon-mainnet.g.alchemy.com/v2/demo".to_string(),
            137,
            translator,
        ).await.unwrap();
        
        let balance = adapter.query_balance(
            "0x0000000000000000000000000000000000000000",
            "MATIC"
        ).await;
        assert!(balance.is_ok());
    }
}
