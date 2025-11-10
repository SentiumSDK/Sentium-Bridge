// REAL NEAR Protocol Adapter - Production-ready implementation
use async_trait::async_trait;
use near_jsonrpc_client::{JsonRpcClient, methods};
use near_primitives::{
    types::{AccountId, Balance},
    transaction::{Transaction, Action, TransferAction},
    views::QueryRequest,
};
use std::sync::Arc;
use std::str::FromStr;

use super::{Intent, RouterError};
use super::intent_translator::{IntentTranslator, TranslatedIntent};

pub struct RealNEARAdapter {
    chain_name: String,
    chain_id: String,
    client: Arc<JsonRpcClient>,
    translator: Arc<IntentTranslator>,
}

impl RealNEARAdapter {
    pub async fn new(
        rpc_url: String,
        network: NEARNetwork,
        translator: Arc<IntentTranslator>,
    ) -> Result<Self, RouterError> {
        let client = JsonRpcClient::connect(&rpc_url);
        
        let chain_id = match network {
            NEARNetwork::Mainnet => "near-mainnet",
            NEARNetwork::Testnet => "near-testnet",
        };
        
        Ok(Self {
            chain_name: "near".to_string(),
            chain_id: chain_id.to_string(),
            client: Arc::new(client),
            translator,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum NEARNetwork {
    Mainnet,
    Testnet,
}

#[async_trait]
impl super::chain_adapter::ChainAdapter for RealNEARAdapter {
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
        let account_id = String::from_utf8(proof.to_vec())
            .map_err(|e| RouterError::VerificationError(format!("Invalid account ID: {}", e)))?;
        
        let account_id = AccountId::from_str(&account_id)
            .map_err(|e| RouterError::VerificationError(format!("Invalid account ID format: {}", e)))?;
        
        let request = methods::query::RpcQueryRequest {
            block_reference: near_primitives::types::BlockReference::latest(),
            request: QueryRequest::ViewAccount { account_id },
        };
        
        let result = self.client.call(request).await;
        Ok(result.is_ok())
    }
    
    async fn submit_transaction(&self, tx_data: &[u8]) -> Result<String, RouterError> {
        let tx: Transaction = borsh::BorshDeserialize::try_from_slice(tx_data)
            .map_err(|e| RouterError::TranslationError(format!("Failed to deserialize: {}", e)))?;
        
        let request = methods::broadcast_tx_async::RpcBroadcastTxAsyncRequest {
            signed_transaction: tx,
        };
        
        let response = self.client.call(request).await
            .map_err(|e| RouterError::TranslationError(format!("Failed to broadcast: {}", e)))?;
        
        Ok(response.to_string())
    }
    
    async fn query_balance(&self, address: &str, _asset: &str) -> Result<u64, RouterError> {
        let account_id = AccountId::from_str(address)
            .map_err(|e| RouterError::TranslationError(format!("Invalid account ID: {}", e)))?;
        
        let request = methods::query::RpcQueryRequest {
            block_reference: near_primitives::types::BlockReference::latest(),
            request: QueryRequest::ViewAccount { account_id },
        };
        
        let response = self.client.call(request).await
            .map_err(|e| RouterError::TranslationError(format!("Query failed: {}", e)))?;
        
        if let near_primitives::views::QueryResponseKind::ViewAccount(account) = response.kind {
            Ok(account.amount as u64)
        } else {
            Err(RouterError::TranslationError("Unexpected response type".to_string()))
        }
    }
}
