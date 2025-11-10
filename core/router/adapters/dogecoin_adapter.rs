// REAL Dogecoin Adapter - Production-ready implementation
// Dogecoin is a Bitcoin fork with faster block times and Scrypt mining
use async_trait::async_trait;
use bitcoin::{
    Address, Network, Transaction, TxOut,
    consensus::encode,
};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use std::sync::Arc;
use std::str::FromStr;

use super::{Intent, RouterError};
use super::intent_translator::{IntentTranslator, TranslatedIntent};
use super::dogecoin_scrypt_verifier::DogecoinScryptVerifier;

pub struct RealDogecoinAdapter {
    chain_name: String,
    chain_id: String,
    rpc_client: Arc<Client>,
    network: DogecoinNetwork,
    translator: Arc<IntentTranslator>,
    scrypt_verifier: DogecoinScryptVerifier,
}

impl RealDogecoinAdapter {
    pub fn new(
        rpc_url: String,
        rpc_user: String,
        rpc_password: String,
        network: DogecoinNetwork,
        translator: Arc<IntentTranslator>,
    ) -> Result<Self, RouterError> {
        let auth = Auth::UserPass(rpc_user, rpc_password);
        let client = Client::new(&rpc_url, auth)
            .map_err(|e| RouterError::TranslationError(format!("Dogecoin RPC connection failed: {}", e)))?;
        
        let chain_id = match network {
            DogecoinNetwork::Mainnet => "dogecoin-mainnet",
            DogecoinNetwork::Testnet => "dogecoin-testnet",
        };
        
        // Initialize Scrypt verifier
        let scrypt_verifier = DogecoinScryptVerifier::new()?;
        
        Ok(Self {
            chain_name: "dogecoin".to_string(),
            chain_id: chain_id.to_string(),
            rpc_client: Arc::new(client),
            network,
            translator,
            scrypt_verifier,
        })
    }
    
    fn get_bitcoin_network(&self) -> Network {
        match self.network {
            DogecoinNetwork::Mainnet => Network::Bitcoin,
            DogecoinNetwork::Testnet => Network::Testnet,
        }
    }
    
    fn create_transfer_transaction(
        &self,
        to_address: &str,
        amount_koinu: u64, // 1 DOGE = 100,000,000 koinu
    ) -> Result<Transaction, RouterError> {
        let to_addr = Address::from_str(to_address)
            .map_err(|e| RouterError::TranslationError(format!("Invalid address: {}", e)))?
            .require_network(self.get_bitcoin_network())
            .map_err(|e| RouterError::TranslationError(format!("Address network mismatch: {}", e)))?;
        
        let output = TxOut {
            value: amount_koinu,
            script_pubkey: to_addr.script_pubkey(),
        };
        
        let tx = Transaction {
            version: 1,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![output],
        };
        
        Ok(tx)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum DogecoinNetwork {
    Mainnet,
    Testnet,
}

#[async_trait]
impl super::chain_adapter::ChainAdapter for RealDogecoinAdapter {
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
        if proof.len() < 80 {
            return Err(RouterError::VerificationError("Proof too short for Dogecoin header".to_string()));
        }
        
        let header_bytes: [u8; 80] = proof[..80].try_into()
            .map_err(|_| RouterError::VerificationError("Invalid header size".to_string()))?;
        
        let header: bitcoin::BlockHeader = encode::deserialize(&header_bytes)
            .map_err(|e| RouterError::VerificationError(format!("Failed to parse header: {}", e)))?;
        
        // Use complete Scrypt verification for Dogecoin PoW
        self.scrypt_verifier.verify_header(&header)
    }
    
    async fn submit_transaction(&self, tx_data: &[u8]) -> Result<String, RouterError> {
        let tx: Transaction = encode::deserialize(tx_data)
            .map_err(|e| RouterError::TranslationError(format!("Failed to parse transaction: {}", e)))?;
        
        let txid = self.rpc_client.send_raw_transaction(&tx)
            .map_err(|e| RouterError::TranslationError(format!("Failed to send transaction: {}", e)))?;
        
        Ok(txid.to_string())
    }
    
    async fn query_balance(&self, address: &str, _asset: &str) -> Result<u64, RouterError> {
        let addr = Address::from_str(address)
            .map_err(|e| RouterError::TranslationError(format!("Invalid address: {}", e)))?
            .require_network(self.get_bitcoin_network())
            .map_err(|e| RouterError::TranslationError(format!("Address network mismatch: {}", e)))?;
        
        let unspent = self.rpc_client.list_unspent(
            Some(1),
            None,
            Some(&[&addr]),
            None,
            None,
        ).map_err(|e| RouterError::TranslationError(format!("Failed to query UTXOs: {}", e)))?;
        
        let balance_doge: f64 = unspent.iter().map(|utxo| utxo.amount.to_btc()).sum();
        let balance_koinu = (balance_doge * 100_000_000.0) as u64;
        
        Ok(balance_koinu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    #[ignore]
    fn test_dogecoin_connection() {
        let translator = Arc::new(IntentTranslator::new());
        let adapter = RealDogecoinAdapter::new(
            "http://localhost:44555".to_string(),
            "user".to_string(),
            "password".to_string(),
            DogecoinNetwork::Testnet,
            translator,
        );
        assert!(adapter.is_ok());
    }
    
    #[test]
    fn test_create_transfer() {
        let translator = Arc::new(IntentTranslator::new());
        let scrypt_verifier = DogecoinScryptVerifier::new().unwrap();
        let adapter = RealDogecoinAdapter {
            chain_name: "dogecoin".to_string(),
            chain_id: "dogecoin-testnet".to_string(),
            rpc_client: Arc::new(
                Client::new(
                    "http://localhost:44555",
                    Auth::UserPass("user".to_string(), "pass".to_string())
                ).unwrap()
            ),
            network: DogecoinNetwork::Testnet,
            translator,
            scrypt_verifier,
        };
        
        let tx = adapter.create_transfer_transaction(
            "noxKJyGPugPRN4wqvrwsrtUXVx4Nf1fVDd",
            100000000, // 1 DOGE
        );
        
        assert!(tx.is_ok());
    }
}
