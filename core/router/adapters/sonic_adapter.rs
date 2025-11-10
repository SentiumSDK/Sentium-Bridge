// Sonic Adapter - High-performance EVM Layer 1 with sub-second finality
// Production-ready implementation optimized for speed

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

pub struct SonicAdapter {
    chain_name: String,
    chain_id: u64,
    provider: Arc<Provider<Ws>>,
    http_provider: Arc<Provider<Http>>,
    translator: Arc<IntentTranslator>,
    wallet: Option<Arc<LocalWallet>>,
    nonce_manager: Arc<RwLock<u64>>,
}

impl SonicAdapter {
    pub async fn new(
        ws_url: String,
        http_url: String,
        chain_id: u64, // 146 for Sonic mainnet
        translator: Arc<IntentTranslator>,
    ) -> Result<Self, RouterError> {
        let ws = Ws::connect(&ws_url).await
            .map_err(|e| RouterError::TranslationError(format!("WebSocket connection failed: {}", e)))?;
        let provider = Provider::new(ws);
        
        let http_provider = Provider::<Http>::try_from(&http_url)
            .map_err(|e| RouterError::TranslationError(format!("HTTP connection failed: {}", e)))?;
        
        Ok(Self {
            chain_name: "sonic".to_string(),
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
    
    async fn estimate_sonic_gas(&self, tx: &TransactionRequest) -> Result<(U256, U256), RouterError> {
        // Sonic is optimized for high-speed transactions with low fees
        let gas_limit = self.provider.estimate_gas(&tx.clone().into(), None).await
            .map_err(|e| RouterError::TranslationError(format!("Gas estimation failed: {}", e)))?;
        
        // Sonic typically has very low gas prices (sub-gwei)
        let gas_price = self.provider.get_gas_price().await
            .map_err(|e| RouterError::TranslationError(format!("Failed to get gas price: {}", e)))?;
        
        // Add 10% buffer for gas price volatility
        let adjusted_gas_price = gas_price * 110 / 100;
        
        Ok((gas_limit, adjusted_gas_price))
    }
}

#[async_trait]
impl ChainAdapter for SonicAdapter {
    fn chain_name(&self) -> &str { 
        &self.chain_name 
    }
    
    fn chain_id(&self) -> &str { 
        "sonic-mainnet" 
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
        
        let (gas_limit, gas_price) = self.estimate_sonic_gas(&tx).await?;
        tx = tx.gas(gas_limit).gas_price(gas_price);
        
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
        
        if asset.is_empty() || asset.to_uppercase() == "S" || asset.to_uppercase() == "SONIC" {
            // Native Sonic token
            let balance = self.http_provider.get_balance(addr, None).await
                .map_err(|e| RouterError::TranslationError(format!("Balance query failed: {}", e)))?;
            Ok(balance.as_u64())
        } else {
            // Query ERC20 token balance on Sonic
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
    async fn test_sonic_connection() {
        let translator = Arc::new(IntentTranslator::new());
        let adapter = SonicAdapter::new(
            "wss://rpc.sonic.fantom.network".to_string(),
            "https://rpc.sonic.fantom.network".to_string(),
            146,
            translator,
        ).await;
        assert!(adapter.is_ok());
    }
    
    #[tokio::test]
    #[ignore]
    async fn test_sonic_balance_query() {
        let translator = Arc::new(IntentTranslator::new());
        let adapter = SonicAdapter::new(
            "wss://rpc.sonic.fantom.network".to_string(),
            "https://rpc.sonic.fantom.network".to_string(),
            146,
            translator,
        ).await.unwrap();
        
        let balance = adapter.query_balance(
            "0x0000000000000000000000000000000000000000",
            "SONIC"
        ).await;
        assert!(balance.is_ok());
    }
}
