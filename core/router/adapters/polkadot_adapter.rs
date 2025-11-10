// REAL Substrate/Polkadot Adapter - Production-ready implementation with subxt
use async_trait::async_trait;
use subxt::{OnlineClient, PolkadotConfig, tx::TxPayload};
use subxt::utils::{AccountId32, MultiAddress, MultiSignature};
use sp_core::{sr25519, Pair, crypto::Ss58Codec, H256, Blake2Hasher};
use sp_runtime::traits::{IdentifyAccount, Verify, BlakeTwo256};
use sp_trie::{TrieDBBuilder, LayoutV1, StorageProof, MemoryDB};
use parity_scale_codec::{Encode, Decode};
use std::sync::Arc;
use std::str::FromStr;

use super::{Intent, RouterError};
use super::intent_translator::{IntentTranslator, TranslatedIntent};

// Define Polkadot runtime types
#[subxt::subxt(runtime_metadata_path = "polkadot_metadata.scale")]
pub mod polkadot {}

pub struct RealSubstrateAdapter {
    chain_name: String,
    chain_id: String,
    client: Arc<OnlineClient<PolkadotConfig>>,
    translator: Arc<IntentTranslator>,
}

impl RealSubstrateAdapter {
    pub async fn new(
        rpc_url: String,
        chain_name: String,
        chain_id: String,
        translator: Arc<IntentTranslator>,
    ) -> Result<Self, RouterError> {
        // Connect to Substrate node
        let client = OnlineClient::<PolkadotConfig>::from_url(&rpc_url).await
            .map_err(|e| RouterError::TranslationError(format!("Substrate connection failed: {}", e)))?;
        
        Ok(Self {
            chain_name,
            chain_id,
            client: Arc::new(client),
            translator,
        })
    }
    
    async fn create_transfer_call(
        &self,
        dest: AccountId32,
        value: u128,
    ) -> Result<Vec<u8>, RouterError> {
        // Create Balances::transfer call
        // Pallet index: 5, Call index: 0 (standard for most Substrate chains)
        
        #[derive(Encode)]
        struct TransferCall {
            dest: MultiAddress<AccountId32, ()>,
            value: u128,
        }
        
        let call = TransferCall {
            dest: MultiAddress::Id(dest),
            value,
        };
        
        // Encode as SCALE
        let mut encoded = vec![5u8, 0u8]; // Pallet and call indices
        encoded.extend(call.encode());
        
        Ok(encoded)
    }
    
    async fn create_staking_bond_call(
        &self,
        controller: AccountId32,
        value: u128,
        payee: u8, // 0 = Staked, 1 = Stash, 2 = Controller
    ) -> Result<Vec<u8>, RouterError> {
        // Create Staking::bond call
        // Pallet index: 6, Call index: 0
        
        #[derive(Encode)]
        struct BondCall {
            controller: MultiAddress<AccountId32, ()>,
            value: u128,
            payee: u8,
        }
        
        let call = BondCall {
            controller: MultiAddress::Id(controller),
            value,
            payee,
        };
        
        let mut encoded = vec![6u8, 0u8];
        encoded.extend(call.encode());
        
        Ok(encoded)
    }
    
    fn verify_storage_proof(
        &self,
        key: &[u8],
        value: Option<&[u8]>,
        proof: &[Vec<u8>],
        state_root: &[u8; 32],
    ) -> Result<bool, RouterError> {
        // Verify Merkle proof for storage item using sp-trie
        
        if proof.is_empty() {
            return Err(RouterError::VerificationError("Empty proof".to_string()));
        }
        
        // Convert proof nodes to StorageProof
        let storage_proof = StorageProof::new(proof.iter().cloned());
        
        // Build trie from proof
        let db = storage_proof.into_memory_db::<Blake2Hasher>();
        let state_root_h256 = H256::from_slice(state_root);
        
        let trie = TrieDBBuilder::<LayoutV1<BlakeTwo256>>::new(&db, &state_root_h256)
            .build();
        
        // Get value from trie
        let trie_value = trie.get(key)
            .map_err(|e| RouterError::VerificationError(format!("Trie lookup failed: {:?}", e)))?;
        
        // Compare with expected value
        Ok(trie_value.as_deref() == value)
    }
}

#[async_trait]
impl super::chain_adapter::ChainAdapter for RealSubstrateAdapter {
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
        // Verify Substrate state proof using sp-trie
        
        if proof.is_empty() {
            return Err(RouterError::VerificationError("Empty proof".to_string()));
        }
        
        // Decode SCALE-encoded proof data
        // Expected format: state_root (32 bytes) + key_len (4 bytes) + key + value_len (4 bytes) + value + proof_nodes
        if proof.len() < 32 {
            return Err(RouterError::VerificationError("Proof too short for state root".to_string()));
        }
        
        // Extract state root (first 32 bytes)
        let mut state_root_bytes = [0u8; 32];
        state_root_bytes.copy_from_slice(&proof[0..32]);
        
        // Parse the rest of the proof
        let mut offset = 32;
        
        // Extract key length and key
        if proof.len() < offset + 4 {
            return Err(RouterError::VerificationError("Proof too short for key length".to_string()));
        }
        let key_len = u32::from_le_bytes([proof[offset], proof[offset+1], proof[offset+2], proof[offset+3]]) as usize;
        offset += 4;
        
        if proof.len() < offset + key_len {
            return Err(RouterError::VerificationError("Proof too short for key".to_string()));
        }
        let key = &proof[offset..offset + key_len];
        offset += key_len;
        
        // Extract value length and value (expected value)
        if proof.len() < offset + 4 {
            return Err(RouterError::VerificationError("Proof too short for value length".to_string()));
        }
        let value_len = u32::from_le_bytes([proof[offset], proof[offset+1], proof[offset+2], proof[offset+3]]) as usize;
        offset += 4;
        
        let expected_value = if value_len > 0 {
            if proof.len() < offset + value_len {
                return Err(RouterError::VerificationError("Proof too short for value".to_string()));
            }
            Some(&proof[offset..offset + value_len])
        } else {
            None
        };
        offset += value_len;
        
        // Extract number of proof nodes
        if proof.len() < offset + 4 {
            return Err(RouterError::VerificationError("Proof too short for proof nodes count".to_string()));
        }
        let nodes_count = u32::from_le_bytes([proof[offset], proof[offset+1], proof[offset+2], proof[offset+3]]) as usize;
        offset += 4;
        
        // Extract proof nodes
        let mut proof_nodes = Vec::new();
        for _ in 0..nodes_count {
            if proof.len() < offset + 4 {
                return Err(RouterError::VerificationError("Proof too short for node length".to_string()));
            }
            let node_len = u32::from_le_bytes([proof[offset], proof[offset+1], proof[offset+2], proof[offset+3]]) as usize;
            offset += 4;
            
            if proof.len() < offset + node_len {
                return Err(RouterError::VerificationError("Proof too short for node data".to_string()));
            }
            proof_nodes.push(proof[offset..offset + node_len].to_vec());
            offset += node_len;
        }
        
        // Verify the storage proof using sp-trie
        self.verify_storage_proof(key, expected_value, &proof_nodes, &state_root_bytes)
    }
    
    async fn submit_transaction(&self, tx_data: &[u8]) -> Result<String, RouterError> {
        // Submit extrinsic using subxt and wait for finalization
        
        if tx_data.is_empty() {
            return Err(RouterError::TranslationError("Empty transaction data".to_string()));
        }
        
        // Submit extrinsic and watch for finalization
        let mut tx_progress = self.client
            .tx()
            .submit_and_watch(tx_data)
            .await
            .map_err(|e| RouterError::TranslationError(format!("Failed to submit extrinsic: {}", e)))?;
        
        // Monitor transaction status through different stages
        while let Some(status) = tx_progress.next().await {
            match status {
                Ok(status) => {
                    match status {
                        subxt::tx::TxStatus::InBlock(details) => {
                            // Transaction is in a block, continue waiting for finalization
                            continue;
                        }
                        subxt::tx::TxStatus::Finalized(details) => {
                            // Transaction is finalized
                            let tx_hash = details.extrinsic_hash();
                            let block_hash = details.block_hash();
                            
                            // Return comprehensive finalization information
                            return Ok(format!(
                                "0x{}:finalized_in_block:0x{}",
                                hex::encode(tx_hash),
                                hex::encode(block_hash)
                            ));
                        }
                        subxt::tx::TxStatus::Error { message } => {
                            return Err(RouterError::TranslationError(
                                format!("Transaction error: {}", message)
                            ));
                        }
                        subxt::tx::TxStatus::Invalid { message } => {
                            return Err(RouterError::TranslationError(
                                format!("Transaction invalid: {}", message)
                            ));
                        }
                        subxt::tx::TxStatus::Dropped { message } => {
                            return Err(RouterError::TranslationError(
                                format!("Transaction dropped: {}", message)
                            ));
                        }
                        _ => {
                            // Other statuses like Validated, Broadcasted, etc.
                            continue;
                        }
                    }
                }
                Err(e) => {
                    return Err(RouterError::TranslationError(
                        format!("Transaction status error: {}", e)
                    ));
                }
            }
        }
        
        // If we exit the loop without finalization, try wait_for_finalized as fallback
        let tx_in_block = tx_progress
            .wait_for_finalized()
            .await
            .map_err(|e| RouterError::TranslationError(format!("Failed to wait for finalization: {}", e)))?;
        
        let tx_hash = tx_in_block.extrinsic_hash();
        let block_hash = tx_in_block.block_hash();
        
        Ok(format!(
            "0x{}:finalized_in_block:0x{}",
            hex::encode(tx_hash),
            hex::encode(block_hash)
        ))
    }
    
    async fn query_balance(&self, address: &str, _asset: &str) -> Result<u64, RouterError> {
        // Parse SS58 address
        let account = AccountId32::from_str(address)
            .map_err(|e| RouterError::TranslationError(format!("Invalid SS58 address: {}", e)))?;
        
        // Query account info
        let account_info_address = subxt::dynamic::storage(
            "System",
            "Account",
            vec![subxt::dynamic::Value::from_bytes(&account)],
        );
        
        let account_info = self.client
            .storage()
            .at_latest()
            .await
            .map_err(|e| RouterError::TranslationError(format!("Failed to get storage: {}", e)))?
            .fetch(&account_info_address)
            .await
            .map_err(|e| RouterError::TranslationError(format!("Failed to fetch account: {}", e)))?;
        
        if let Some(info) = account_info {
            // Decode account info
            // Structure: { nonce, consumers, providers, sufficients, data: { free, reserved, ... } }
            let value = info.to_value()
                .map_err(|e| RouterError::TranslationError(format!("Failed to decode account info: {}", e)))?;
            
            // Extract free balance
            if let Some(data) = value.at("data") {
                if let Some(free) = data.at("free") {
                    if let Some(balance) = free.as_u128() {
                        return Ok(balance as u64);
                    }
                }
            }
        }
        
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[ignore] // Requires Polkadot node
    async fn test_substrate_connection() {
        let translator = Arc::new(IntentTranslator::new());
        
        // Connect to Polkadot public RPC
        let adapter = RealSubstrateAdapter::new(
            "wss://rpc.polkadot.io".to_string(),
            "polkadot".to_string(),
            "polkadot-0".to_string(),
            translator,
        ).await;
        
        assert!(adapter.is_ok());
    }
    
    #[tokio::test]
    #[ignore] // Requires Polkadot node
    async fn test_query_balance() {
        let translator = Arc::new(IntentTranslator::new());
        
        let adapter = RealSubstrateAdapter::new(
            "wss://rpc.polkadot.io".to_string(),
            "polkadot".to_string(),
            "polkadot-0".to_string(),
            translator,
        ).await.unwrap();
        
        // Query a known address (Polkadot treasury)
        let balance = adapter.query_balance(
            "13UVJyLnbVp9RBZYFwFGyDvVd1y27Tt8tkntv6Q7JVPhFsTB",
            "DOT"
        ).await;
        
        assert!(balance.is_ok());
    }
    
    #[tokio::test]
    async fn test_create_transfer_call() {
        let translator = Arc::new(IntentTranslator::new());
        
        // Create adapter without connecting
        let client = OnlineClient::<PolkadotConfig>::from_url("wss://rpc.polkadot.io")
            .await
            .unwrap();
        
        let adapter = RealSubstrateAdapter {
            chain_name: "polkadot".to_string(),
            chain_id: "polkadot-0".to_string(),
            client: Arc::new(client),
            translator,
        };
        
        let dest = AccountId32::from_str("5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY")
            .unwrap();
        
        let call = adapter.create_transfer_call(dest, 1_000_000_000_000).await;
        
        assert!(call.is_ok());
        let encoded = call.unwrap();
        
        // Check pallet and call indices
        assert_eq!(encoded[0], 5); // Balances pallet
        assert_eq!(encoded[1], 0); // transfer call
    }
}
