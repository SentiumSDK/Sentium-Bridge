// Sentium Bridge - Quantum-Safe Light Clients
// This module implements light clients for various blockchains with quantum-resistant verification

pub mod ethereum;
pub mod bitcoin;
pub mod polkadot;
pub mod manager;

use pqcrypto_dilithium::dilithium5;
use pqcrypto_traits::sign::{PublicKey as PQPublicKey, SignedMessage};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_512};

pub use manager::LightClientManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightClient {
    pub chain_id: String,
    pub latest_height: u64,
    pub state_root: Vec<u8>,
    pub validator_set: Vec<Validator>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Validator {
    pub address: Vec<u8>,
    pub public_key: Vec<u8>, // Dilithium5 public key
    pub voting_power: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateProof {
    pub height: u64,
    pub state_root: Vec<u8>,
    pub signatures: Vec<QuantumSignature>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumSignature {
    pub validator_address: Vec<u8>,
    pub signature: Vec<u8>, // Dilithium5 signature (4595 bytes)
}

impl LightClient {
    pub fn new(chain_id: String) -> Self {
        Self {
            chain_id,
            latest_height: 0,
            state_root: Vec::new(),
            validator_set: Vec::new(),
        }
    }
    
    pub fn add_validator(&mut self, validator: Validator) {
        self.validator_set.push(validator);
    }
    
    pub fn verify_state_proof(&self, proof: &StateProof) -> Result<bool, LightClientError> {
        // Verify that we have enough signatures (>2/3 voting power)
        let total_power: u64 = self.validator_set.iter().map(|v| v.voting_power).sum();
        if total_power == 0 {
            return Err(LightClientError::NoValidators);
        }
        
        let threshold = (total_power * 2) / 3;
        let mut signed_power = 0u64;
        
        // Construct message to verify
        let message = self.construct_proof_message(proof);
        
        for sig in &proof.signatures {
            // Find validator
            let validator = self.validator_set
                .iter()
                .find(|v| v.address == sig.validator_address)
                .ok_or(LightClientError::UnknownValidator)?;
            
            // Verify Dilithium5 signature
            if self.verify_dilithium_signature(&message, &sig.signature, &validator.public_key)? {
                signed_power += validator.voting_power;
            }
        }
        
        Ok(signed_power > threshold)
    }
    
    fn construct_proof_message(&self, proof: &StateProof) -> Vec<u8> {
        // Construct message: chain_id || height || state_root || timestamp
        let mut message = Vec::new();
        message.extend_from_slice(self.chain_id.as_bytes());
        message.extend_from_slice(&proof.height.to_le_bytes());
        message.extend_from_slice(&proof.state_root);
        message.extend_from_slice(&proof.timestamp.to_le_bytes());
        
        // Hash the message with SHA3-512 for quantum resistance
        let mut hasher = Sha3_512::new();
        hasher.update(&message);
        hasher.finalize().to_vec()
    }
    
    fn verify_dilithium_signature(
        &self,
        message: &[u8],
        signature: &[u8],
        public_key_bytes: &[u8],
    ) -> Result<bool, LightClientError> {
        // Verify Dilithium5 signature
        // Dilithium5 signature is 4595 bytes
        if signature.len() != 4595 {
            return Err(LightClientError::InvalidSignatureSize);
        }
        
        // Parse public key
        let public_key = dilithium5::PublicKey::from_bytes(public_key_bytes)
            .map_err(|_| LightClientError::InvalidPublicKey)?;
        
        // Create signed message (signature + message)
        let mut signed_msg = signature.to_vec();
        signed_msg.extend_from_slice(message);
        
        let signed_message = dilithium5::SignedMessage::from_bytes(&signed_msg)
            .map_err(|_| LightClientError::InvalidSignature)?;
        
        // Verify signature
        match dilithium5::open(&signed_message, &public_key) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
    
    pub fn update_state(&mut self, proof: StateProof) -> Result<(), LightClientError> {
        // Verify proof
        if !self.verify_state_proof(&proof)? {
            return Err(LightClientError::InvalidProof);
        }
        
        // Check height is increasing
        if proof.height <= self.latest_height {
            return Err(LightClientError::InvalidHeight);
        }
        
        // Update state
        self.latest_height = proof.height;
        self.state_root = proof.state_root;
        
        Ok(())
    }
    
    pub fn update_validator_set(&mut self, new_validators: Vec<Validator>) {
        self.validator_set = new_validators;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LightClientError {
    #[error("Unknown validator")]
    UnknownValidator,
    
    #[error("Invalid proof")]
    InvalidProof,
    
    #[error("Signature verification failed")]
    SignatureVerificationFailed,
    
    #[error("Invalid signature size")]
    InvalidSignatureSize,
    
    #[error("Invalid public key")]
    InvalidPublicKey,
    
    #[error("Invalid signature")]
    InvalidSignature,
    
    #[error("No validators configured")]
    NoValidators,
    
    #[error("Invalid height")]
    InvalidHeight,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_light_client_creation() {
        let client = LightClient::new("ethereum-1".to_string());
        assert_eq!(client.chain_id, "ethereum-1");
        assert_eq!(client.latest_height, 0);
    }
    
    #[test]
    fn test_add_validator() {
        let mut client = LightClient::new("test-chain".to_string());
        
        let validator = Validator {
            address: vec![1, 2, 3, 4],
            public_key: vec![0u8; 2592], // Dilithium5 public key size
            voting_power: 100,
        };
        
        client.add_validator(validator);
        assert_eq!(client.validator_set.len(), 1);
    }
    
    #[test]
    fn test_construct_proof_message() {
        let client = LightClient::new("test-chain".to_string());
        let proof = StateProof {
            height: 100,
            state_root: vec![1, 2, 3, 4],
            signatures: vec![],
            timestamp: 1234567890,
        };
        
        let message = client.construct_proof_message(&proof);
        assert!(message.len() > 0);
        assert_eq!(message.len(), 64); // SHA3-512 output
    }
}
