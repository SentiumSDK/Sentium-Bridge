// Sentium Bridge - Intent Router
// This module handles cross-chain intent translation and routing

pub mod intent_translator;
pub mod chain_adapter;
pub mod routing_logic;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

pub use intent_translator::{IntentTranslator, TranslatedIntent, ActionType};
pub use chain_adapter::{ChainAdapter, EthereumAdapter, PolkadotAdapter, BitcoinAdapter, CosmosAdapter, SentiumAdapter};
pub use routing_logic::{RoutingEngine, Route, RouteHop, BridgeType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    pub id: String,
    pub from_chain: String,
    pub to_chain: String,
    pub action: String,
    pub params: Vec<u8>,
    pub context: Vec<u8>,
}

pub struct Router {
    adapters: Arc<RwLock<Vec<Arc<dyn ChainAdapter>>>>,
    #[allow(dead_code)]
    translator: Arc<IntentTranslator>,
    routing_engine: Arc<RwLock<RoutingEngine>>,
}

#[derive(Debug, thiserror::Error)]
pub enum RouterError {
    #[error("Chain not supported: {0}")]
    UnsupportedChain(String),
    
    #[error("Translation failed: {0}")]
    TranslationError(String),
    
    #[error("Verification failed: {0}")]
    VerificationError(String),
    
    #[error("Routing failed: {0}")]
    RoutingError(String),
}

impl Router {
    pub fn new() -> Self {
        Self {
            adapters: Arc::new(RwLock::new(Vec::new())),
            translator: Arc::new(IntentTranslator::new()),
            routing_engine: Arc::new(RwLock::new(RoutingEngine::new())),
        }
    }
    
    pub async fn add_adapter(&self, adapter: Arc<dyn ChainAdapter>) {
        let mut adapters = self.adapters.write().await;
        adapters.push(adapter);
    }
    
    pub async fn route_intent(&self, intent: &Intent) -> Result<TranslatedIntent, RouterError> {
        // Find optimal route
        let mut routing_engine = self.routing_engine.write().await;
        let _route = routing_engine.find_route(intent)
            .map_err(|e| RouterError::RoutingError(e.to_string()))?;
        
        // Find adapter for target chain
        let adapters = self.adapters.read().await;
        let adapter = adapters
            .iter()
            .find(|a| a.chain_name() == intent.to_chain)
            .ok_or_else(|| RouterError::UnsupportedChain(intent.to_chain.clone()))?;
        
        // Translate intent
        adapter.translate_intent(intent).await
    }
    
    pub async fn find_route(&self, intent: &Intent) -> Result<Route, RouterError> {
        let mut routing_engine = self.routing_engine.write().await;
        routing_engine.find_route(intent)
            .map_err(|e| RouterError::RoutingError(e.to_string()))
    }
    
    pub async fn get_all_routes(&self, from: &str, to: &str, max_hops: usize) -> Vec<Route> {
        let routing_engine = self.routing_engine.read().await;
        routing_engine.get_all_routes(from, to, max_hops)
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_router_creation() {
        let router = Router::new();
        let adapters = router.adapters.read().await;
        assert_eq!(adapters.len(), 0);
    }
    
    #[tokio::test]
    async fn test_add_adapter() {
        let router = Router::new();
        let translator = Arc::new(IntentTranslator::new());
        let adapter = Arc::new(EthereumAdapter::new("http://localhost:8545".to_string(), translator));
        
        router.add_adapter(adapter).await;
        
        let adapters = router.adapters.read().await;
        assert_eq!(adapters.len(), 1);
    }
}
