// Light Client Manager - Manages multiple light clients for different chains
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{LightClient, StateProof, Validator, LightClientError};

pub struct LightClientManager {
    clients: Arc<RwLock<HashMap<String, LightClient>>>,
}

impl LightClientManager {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn add_client(&self, chain_id: String, client: LightClient) {
        let mut clients = self.clients.write().await;
        clients.insert(chain_id, client);
    }
    
    pub async fn get_client(&self, chain_id: &str) -> Option<LightClient> {
        let clients = self.clients.read().await;
        clients.get(chain_id).cloned()
    }
    
    pub async fn verify_state(&self, chain_id: &str, proof: &StateProof) -> Result<bool, LightClientError> {
        let clients = self.clients.read().await;
        let client = clients
            .get(chain_id)
            .ok_or_else(|| LightClientError::InvalidProof)?;
        
        client.verify_state_proof(proof)
    }
    
    pub async fn update_state(&self, chain_id: &str, proof: StateProof) -> Result<(), LightClientError> {
        let mut clients = self.clients.write().await;
        let client = clients
            .get_mut(chain_id)
            .ok_or_else(|| LightClientError::InvalidProof)?;
        
        client.update_state(proof)
    }
    
    pub async fn update_validators(&self, chain_id: &str, validators: Vec<Validator>) -> Result<(), LightClientError> {
        let mut clients = self.clients.write().await;
        let client = clients
            .get_mut(chain_id)
            .ok_or_else(|| LightClientError::InvalidProof)?;
        
        client.update_validator_set(validators);
        Ok(())
    }
    
    pub async fn get_latest_height(&self, chain_id: &str) -> Option<u64> {
        let clients = self.clients.read().await;
        clients.get(chain_id).map(|c| c.latest_height)
    }
    
    pub async fn get_state_root(&self, chain_id: &str) -> Option<Vec<u8>> {
        let clients = self.clients.read().await;
        clients.get(chain_id).map(|c| c.state_root.clone())
    }
    
    pub async fn list_chains(&self) -> Vec<String> {
        let clients = self.clients.read().await;
        clients.keys().cloned().collect()
    }
}

impl Default for LightClientManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_manager_creation() {
        let manager = LightClientManager::new();
        let chains = manager.list_chains().await;
        assert_eq!(chains.len(), 0);
    }
    
    #[tokio::test]
    async fn test_add_client() {
        let manager = LightClientManager::new();
        let client = LightClient::new("ethereum-1".to_string());
        
        manager.add_client("ethereum-1".to_string(), client).await;
        
        let chains = manager.list_chains().await;
        assert_eq!(chains.len(), 1);
        assert!(chains.contains(&"ethereum-1".to_string()));
    }
    
    #[tokio::test]
    async fn test_get_client() {
        let manager = LightClientManager::new();
        let client = LightClient::new("ethereum-1".to_string());
        
        manager.add_client("ethereum-1".to_string(), client).await;
        
        let retrieved = manager.get_client("ethereum-1").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().chain_id, "ethereum-1");
    }
    
    #[tokio::test]
    async fn test_get_latest_height() {
        let manager = LightClientManager::new();
        let mut client = LightClient::new("ethereum-1".to_string());
        client.latest_height = 12345;
        
        manager.add_client("ethereum-1".to_string(), client).await;
        
        let height = manager.get_latest_height("ethereum-1").await;
        assert_eq!(height, Some(12345));
    }
}
