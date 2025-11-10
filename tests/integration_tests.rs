// Integration tests for Sentium Bridge Protocol
use sentium_bridge::core::router::{Router, Intent, IntentTranslator, EthereumAdapter, PolkadotAdapter};
use sentium_bridge::light_clients::{LightClient, LightClientManager, StateProof, Validator, QuantumSignature};
use sentium_bridge::core::context::{SemanticContext, UserPreferences, RiskLevel, ContextPreserver, InMemoryStorage};
use std::sync::Arc;

#[tokio::test]
async fn test_end_to_end_cross_chain_intent() {
    // Setup router
    let router = Router::new();
    let translator = Arc::new(IntentTranslator::new());
    
    // Add adapters
    let eth_adapter = Arc::new(EthereumAdapter::new(
        "http://localhost:8545".to_string(),
        translator.clone(),
    ));
    let dot_adapter = Arc::new(PolkadotAdapter::new(
        "wss://rpc.polkadot.io".to_string(),
        translator.clone(),
    ));
    
    router.add_adapter(eth_adapter).await;
    router.add_adapter(dot_adapter).await;
    
    // Create intent
    let intent = Intent {
        id: "test-intent-1".to_string(),
        from_chain: "ethereum".to_string(),
        to_chain: "polkadot".to_string(),
        action: "transfer".to_string(),
        params: vec![],
        context: vec![],
    };
    
    // Route intent
    let result = router.route_intent(&intent).await;
    assert!(result.is_ok());
    
    let translated = result.unwrap();
    assert_eq!(translated.original_intent.id, "test-intent-1");
    assert!(translated.target_format.len() > 0);
}

#[tokio::test]
async fn test_light_client_state_verification() {
    let manager = LightClientManager::new();
    
    // Create light client
    let mut client = LightClient::new("test-chain".to_string());
    
    // Add validators
    let validator = Validator {
        address: vec![1, 2, 3, 4],
        public_key: vec![0u8; 2592], // Dilithium5 public key size
        voting_power: 100,
    };
    client.add_validator(validator);
    
    // Add client to manager
    manager.add_client("test-chain".to_string(), client).await;
    
    // Create state proof with proper structure
    let proof = StateProof {
        height: 100,
        state_root: vec![1, 2, 3, 4],
        signatures: vec![],
        timestamp: 1234567890,
    };
    
    // Note: This will fail without valid signatures, which is expected
    let result = manager.verify_state("test-chain", &proof).await;
    assert!(result.is_err() || !result.unwrap());
}

#[tokio::test]
async fn test_context_preservation() {
    let storage = Arc::new(InMemoryStorage::new());
    let preserver = ContextPreserver::new(storage);
    
    // Create context
    let prefs = UserPreferences {
        slippage_tolerance: 0.01,
        max_gas_price: 100,
        min_confirmations: 6,
        preferred_routes: vec![],
        risk_tolerance: RiskLevel::Medium,
    };
    
    let context = SemanticContext::new(
        "intent-1".to_string(),
        "ethereum".to_string(),
        "polkadot".to_string(),
        prefs,
    );
    
    let context_id = context.id.clone();
    
    // Save context
    preserver.save_context(context).await.unwrap();
    
    // Load context
    let loaded = preserver.load_context(&context_id).await.unwrap();
    assert_eq!(loaded.intent_id, "intent-1");
    assert_eq!(loaded.source_chain, "ethereum");
    assert_eq!(loaded.target_chain, "polkadot");
    assert!(loaded.verify_integrity());
}

#[tokio::test]
async fn test_multi_hop_routing() {
    let router = Router::new();
    
    let intent = Intent {
        id: "multi-hop-1".to_string(),
        from_chain: "ethereum".to_string(),
        to_chain: "polkadot".to_string(),
        action: "transfer".to_string(),
        params: vec![],
        context: vec![],
    };
    
    // Find route
    let route = router.find_route(&intent).await;
    assert!(route.is_ok());
    
    let route = route.unwrap();
    assert_eq!(route.source_chain, "ethereum");
    assert_eq!(route.target_chain, "polkadot");
    assert!(route.hops.len() > 0);
}

#[tokio::test]
async fn test_all_routes_discovery() {
    let router = Router::new();
    
    // Get all possible routes
    let routes = router.get_all_routes("ethereum", "polkadot", 3).await;
    
    assert!(routes.len() > 0);
    
    // Verify all routes are valid
    for route in routes {
        assert_eq!(route.source_chain, "ethereum");
        assert_eq!(route.target_chain, "polkadot");
        assert!(route.hops.len() <= 3);
        assert!(route.estimated_cost > 0);
    }
}

#[test]
fn test_intent_translation_ethereum() {
    let translator = IntentTranslator::new();
    
    let intent = Intent {
        id: "eth-transfer-1".to_string(),
        from_chain: "sentium".to_string(),
        to_chain: "ethereum".to_string(),
        action: "transfer".to_string(),
        params: vec![],
        context: vec![],
    };
    
    let result = translator.translate(&intent);
    assert!(result.is_ok());
    
    let translated = result.unwrap();
    assert!(translated.target_format.len() > 0);
    assert!(translated.translation_metadata.gas_estimate > 0);
}

#[test]
fn test_intent_translation_polkadot() {
    let translator = IntentTranslator::new();
    
    let intent = Intent {
        id: "dot-transfer-1".to_string(),
        from_chain: "sentium".to_string(),
        to_chain: "polkadot".to_string(),
        action: "transfer".to_string(),
        params: vec![],
        context: vec![],
    };
    
    let result = translator.translate(&intent);
    assert!(result.is_ok());
    
    let translated = result.unwrap();
    assert!(translated.target_format.len() > 0);
}

#[test]
fn test_intent_translation_bitcoin() {
    let translator = IntentTranslator::new();
    
    let intent = Intent {
        id: "btc-transfer-1".to_string(),
        from_chain: "sentium".to_string(),
        to_chain: "bitcoin".to_string(),
        action: "transfer".to_string(),
        params: vec![],
        context: vec![],
    };
    
    let result = translator.translate(&intent);
    assert!(result.is_ok());
}

#[test]
fn test_unsupported_chain() {
    let translator = IntentTranslator::new();
    
    let intent = Intent {
        id: "unknown-1".to_string(),
        from_chain: "sentium".to_string(),
        to_chain: "unknown-chain".to_string(),
        action: "transfer".to_string(),
        params: vec![],
        context: vec![],
    };
    
    let result = translator.translate(&intent);
    assert!(result.is_err());
}

#[test]
fn test_context_integrity() {
    let prefs = UserPreferences {
        slippage_tolerance: 0.01,
        max_gas_price: 100,
        min_confirmations: 6,
        preferred_routes: vec![],
        risk_tolerance: RiskLevel::Medium,
    };
    
    let mut context = SemanticContext::new(
        "intent-1".to_string(),
        "ethereum".to_string(),
        "polkadot".to_string(),
        prefs,
    );
    
    // Verify initial integrity
    assert!(context.verify_integrity());
    
    // Modify context
    context.add_metadata("key1".to_string(), "value1".to_string());
    
    // Verify integrity after modification
    assert!(context.verify_integrity());
}
