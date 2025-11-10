// Context Preserver - Maintains semantic context across chains
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_512};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticContext {
    pub id: String,
    pub intent_id: String,
    pub source_chain: String,
    pub target_chain: String,
    pub user_preferences: UserPreferences,
    pub transaction_history: Vec<TransactionRecord>,
    pub metadata: HashMap<String, String>,
    pub timestamp: u64,
    pub integrity_hash: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    pub slippage_tolerance: f64,
    pub max_gas_price: u64,
    pub min_confirmations: u32,
    pub preferred_routes: Vec<String>,
    pub risk_tolerance: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRecord {
    pub chain: String,
    pub tx_hash: String,
    pub status: TransactionStatus,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
}

pub struct ContextPreserver {
    contexts: Arc<RwLock<HashMap<String, SemanticContext>>>,
    storage: Arc<dyn ContextStorage>,
}

#[async_trait::async_trait]
pub trait ContextStorage: Send + Sync {
    async fn save(&self, context: &SemanticContext) -> Result<(), ContextError>;
    async fn load(&self, id: &str) -> Result<Option<SemanticContext>, ContextError>;
    async fn delete(&self, id: &str) -> Result<(), ContextError>;
    async fn list(&self) -> Result<Vec<String>, ContextError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("Context not found: {0}")]
    NotFound(String),
    
    #[error("Invalid context: {0}")]
    Invalid(String),
    
    #[error("Integrity check failed")]
    IntegrityCheckFailed,
    
    #[error("Storage error: {0}")]
    StorageError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

impl SemanticContext {
    pub fn new(
        intent_id: String,
        source_chain: String,
        target_chain: String,
        user_preferences: UserPreferences,
    ) -> Self {
        let id = Self::generate_id(&intent_id, &source_chain, &target_chain);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut context = Self {
            id,
            intent_id,
            source_chain,
            target_chain,
            user_preferences,
            transaction_history: Vec::new(),
            metadata: HashMap::new(),
            timestamp,
            integrity_hash: Vec::new(),
        };
        
        context.update_integrity_hash();
        context
    }
    
    fn generate_id(intent_id: &str, source: &str, target: &str) -> String {
        let mut hasher = Sha3_512::new();
        hasher.update(intent_id.as_bytes());
        hasher.update(source.as_bytes());
        hasher.update(target.as_bytes());
        let hash = hasher.finalize();
        hex::encode(&hash[..16])
    }
    
    pub fn add_transaction(&mut self, record: TransactionRecord) {
        self.transaction_history.push(record);
        self.update_integrity_hash();
    }
    
    pub fn add_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
        self.update_integrity_hash();
    }
    
    pub fn update_integrity_hash(&mut self) {
        let mut hasher = Sha3_512::new();
        
        // Hash all context data
        hasher.update(self.id.as_bytes());
        hasher.update(self.intent_id.as_bytes());
        hasher.update(self.source_chain.as_bytes());
        hasher.update(self.target_chain.as_bytes());
        hasher.update(&self.timestamp.to_le_bytes());
        
        // Hash user preferences
        hasher.update(&self.user_preferences.slippage_tolerance.to_le_bytes());
        hasher.update(&self.user_preferences.max_gas_price.to_le_bytes());
        hasher.update(&self.user_preferences.min_confirmations.to_le_bytes());
        
        // Hash transaction history
        for tx in &self.transaction_history {
            hasher.update(tx.chain.as_bytes());
            hasher.update(tx.tx_hash.as_bytes());
            hasher.update(&tx.timestamp.to_le_bytes());
        }
        
        // Hash metadata
        let mut keys: Vec<_> = self.metadata.keys().collect();
        keys.sort();
        for key in keys {
            hasher.update(key.as_bytes());
            hasher.update(self.metadata[key].as_bytes());
        }
        
        self.integrity_hash = hasher.finalize().to_vec();
    }
    
    pub fn verify_integrity(&self) -> bool {
        let mut temp_context = self.clone();
        let original_hash = self.integrity_hash.clone();
        temp_context.update_integrity_hash();
        
        temp_context.integrity_hash == original_hash
    }
}

impl ContextPreserver {
    pub fn new(storage: Arc<dyn ContextStorage>) -> Self {
        Self {
            contexts: Arc::new(RwLock::new(HashMap::new())),
            storage,
        }
    }
    
    pub async fn save_context(&self, context: SemanticContext) -> Result<(), ContextError> {
        // Verify integrity before saving
        if !context.verify_integrity() {
            return Err(ContextError::IntegrityCheckFailed);
        }
        
        // Save to storage
        self.storage.save(&context).await?;
        
        // Cache in memory
        let mut contexts = self.contexts.write().await;
        contexts.insert(context.id.clone(), context);
        
        Ok(())
    }
    
    pub async fn load_context(&self, id: &str) -> Result<SemanticContext, ContextError> {
        // Check cache first
        {
            let contexts = self.contexts.read().await;
            if let Some(context) = contexts.get(id) {
                return Ok(context.clone());
            }
        }
        
        // Load from storage
        let context = self.storage.load(id).await?
            .ok_or_else(|| ContextError::NotFound(id.to_string()))?;
        
        // Verify integrity
        if !context.verify_integrity() {
            return Err(ContextError::IntegrityCheckFailed);
        }
        
        // Cache it
        let mut contexts = self.contexts.write().await;
        contexts.insert(id.to_string(), context.clone());
        
        Ok(context)
    }
    
    pub async fn update_context<F>(&self, id: &str, update_fn: F) -> Result<(), ContextError>
    where
        F: FnOnce(&mut SemanticContext),
    {
        let mut context = self.load_context(id).await?;
        update_fn(&mut context);
        context.update_integrity_hash();
        self.save_context(context).await
    }
    
    pub async fn delete_context(&self, id: &str) -> Result<(), ContextError> {
        // Remove from cache
        {
            let mut contexts = self.contexts.write().await;
            contexts.remove(id);
        }
        
        // Delete from storage
        self.storage.delete(id).await
    }
    
    pub async fn list_contexts(&self) -> Result<Vec<String>, ContextError> {
        self.storage.list().await
    }
}

// In-memory storage implementation for testing
pub struct InMemoryStorage {
    data: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl ContextStorage for InMemoryStorage {
    async fn save(&self, context: &SemanticContext) -> Result<(), ContextError> {
        let serialized = serde_json::to_vec(context)
            .map_err(|e| ContextError::SerializationError(e.to_string()))?;
        
        let mut data = self.data.write().await;
        data.insert(context.id.clone(), serialized);
        
        Ok(())
    }
    
    async fn load(&self, id: &str) -> Result<Option<SemanticContext>, ContextError> {
        let data = self.data.read().await;
        
        if let Some(serialized) = data.get(id) {
            let context = serde_json::from_slice(serialized)
                .map_err(|e| ContextError::SerializationError(e.to_string()))?;
            Ok(Some(context))
        } else {
            Ok(None)
        }
    }
    
    async fn delete(&self, id: &str) -> Result<(), ContextError> {
        let mut data = self.data.write().await;
        data.remove(id);
        Ok(())
    }
    
    async fn list(&self) -> Result<Vec<String>, ContextError> {
        let data = self.data.read().await;
        Ok(data.keys().cloned().collect())
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_semantic_context_creation() {
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
        
        assert_eq!(context.intent_id, "intent-1");
        assert_eq!(context.source_chain, "ethereum");
        assert_eq!(context.target_chain, "polkadot");
        assert!(context.integrity_hash.len() > 0);
    }
    
    #[test]
    fn test_integrity_verification() {
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
        
        assert!(context.verify_integrity());
    }
    
    #[test]
    fn test_add_transaction() {
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
        
        let tx = TransactionRecord {
            chain: "ethereum".to_string(),
            tx_hash: "0x123".to_string(),
            status: TransactionStatus::Confirmed,
            timestamp: 1234567890,
        };
        
        context.add_transaction(tx);
        assert_eq!(context.transaction_history.len(), 1);
        assert!(context.verify_integrity());
    }
    
    #[tokio::test]
    async fn test_context_preserver() {
        let storage = Arc::new(InMemoryStorage::new());
        let preserver = ContextPreserver::new(storage);
        
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
        
        // List contexts
        let list = preserver.list_contexts().await.unwrap();
        assert_eq!(list.len(), 1);
    }
}
