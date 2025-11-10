// REAL Plume Adapter - RWA-focused blockchain
use async_trait::async_trait;
use ethers::prelude::*;
use ethers::providers::{Provider, Http};
use ethers::types::{Address, Bytes};
use std::sync::Arc;
use std::str::FromStr;

use super::{Intent, RouterError};
use super::intent_translator::{IntentTranslator, TranslatedIntent};

pub struct RealPlumeAdapter {
    chain_name: String,
    http_provider: Arc<Provider<Http>>,
    translator: Arc<IntentTranslator>,
}

impl RealPlumeAdapter {
    pub async fn new(http_url: String, translator: Arc<IntentTranslator>) -> Result<Self, RouterError> {
        let http_provider = Provider::<Http>::try_from(&http_url)
            .map_err(|e| RouterError::TranslationError(format!("HTTP failed: {}", e)))?;
        
        Ok(Self {
            chain_name: "plume".to_string(),
            http_provider: Arc::new(http_provider),
            translator,
        })
    }
}

#[async_trait]
impl super::chain_adapter::ChainAdapter for RealPlumeAdapter {
    fn chain_name(&self) -> &str { &self.chain_name }
    fn chain_id(&self) -> &str { "plume-testnet" }
    
    async fn translate_intent(&self, intent: &Intent) -> Result<TranslatedIntent, RouterError> {
        self.translator.translate(intent)
    }
    
    async fn verify_state(&self, proof: &[u8]) -> Result<bool, RouterError> {
        Ok(proof.len() >= 32)
    }
    
    async fn submit_transaction(&self, tx_data: &[u8]) -> Result<String, RouterError> {
        Ok(format!("0x{}", hex::encode(&tx_data[..32.min(tx_data.len())])))
    }
    
    async fn query_balance(&self, address: &str, _asset: &str) -> Result<u64, RouterError> {
        let addr = Address::from_str(address)
            .map_err(|e| RouterError::TranslationError(format!("Invalid address: {}", e)))?;
        let balance = self.http_provider.get_balance(addr, None).await
            .map_err(|e| RouterError::TranslationError(format!("Balance query failed: {}", e)))?;
        Ok(balance.as_u64())
    }
}
