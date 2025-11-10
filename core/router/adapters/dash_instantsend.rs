// Dash InstantSend Verification
// InstantSend uses BLS signatures from masternode quorums for instant transaction locking

use sha2::{Sha256, Digest};
use crate::router::RouterError;

/// InstantSend Lock Verifier
/// 
/// InstantSend is a Dash feature that provides instant transaction confirmation
/// through masternode quorum signatures using BLS (Boneh-Lynn-Shacham) signatures.
pub struct InstantSendVerifier {
    /// Minimum quorum size required for valid InstantSend lock
    min_quorum_size: usize,
}

impl InstantSendVerifier {
    /// Create a new InstantSend verifier
    /// 
    /// # Arguments
    /// * `min_quorum_size` - Minimum number of masternode signatures required (typically 60% of quorum)
    pub fn new(min_quorum_size: usize) -> Self {
        Self {
            min_quorum_size,
        }
    }
    
    /// Verify an InstantSend lock
    /// 
    /// # Arguments
    /// * `tx_hash` - The transaction hash being locked
    /// * `quorum_sig` - The aggregated BLS signature from the masternode quorum
    /// * `quorum_public_key` - The aggregated public key of the quorum
    /// * `quorum_size` - The number of masternodes in the quorum
    /// 
    /// # Returns
    /// * `Ok(true)` if the InstantSend lock is valid
    /// * `Ok(false)` if the signature is invalid
    /// * `Err` if verification fails
    pub fn verify_instantsend_lock(
        &self,
        tx_hash: &[u8],
        quorum_sig: &[u8],
        quorum_public_key: &[u8],
        quorum_size: usize,
    ) -> Result<bool, RouterError> {
        // Validate inputs
        if tx_hash.len() != 32 {
            return Err(RouterError::VerificationError(
                "Transaction hash must be 32 bytes".to_string()
            ));
        }
        
        if quorum_size < self.min_quorum_size {
            return Err(RouterError::VerificationError(
                format!("Quorum size {} is below minimum {}", quorum_size, self.min_quorum_size)
            ));
        }
        
        // Verify BLS signature
        self.verify_bls_signature(tx_hash, quorum_sig, quorum_public_key)
    }
    
    /// Verify a BLS signature
    /// 
    /// BLS (Boneh-Lynn-Shacham) signatures allow for signature aggregation,
    /// which is perfect for masternode quorums where multiple nodes sign the same message.
    fn verify_bls_signature(
        &self,
        message: &[u8],
        signature: &[u8],
        public_key: &[u8],
    ) -> Result<bool, RouterError> {
        use blst::min_pk::{PublicKey, Signature};
        
        // Parse the public key
        let pk = PublicKey::from_bytes(public_key)
            .map_err(|e| RouterError::VerificationError(
                format!("Invalid public key: {:?}", e)
            ))?;
        
        // Parse the signature
        let sig = Signature::from_bytes(signature)
            .map_err(|e| RouterError::VerificationError(
                format!("Invalid signature: {:?}", e)
            ))?;
        
        // Create the message hash for signing
        // Dash uses SHA256 hash of the transaction hash as the message
        let mut hasher = Sha256::new();
        hasher.update(message);
        let msg_hash = hasher.finalize();
        
        // Verify the signature
        // DST (Domain Separation Tag) for Dash InstantSend
        const DST: &[u8] = b"DASH_INSTANTSEND_V1";
        
        let result = sig.verify(true, &msg_hash, DST, &[], &pk, true);
        
        match result {
            blst::BLST_ERROR::BLST_SUCCESS => Ok(true),
            _ => Ok(false),
        }
    }
    
    /// Verify multiple InstantSend locks for a transaction
    /// 
    /// A transaction may have multiple locks from different quorums
    /// for different inputs. All locks must be valid.
    pub fn verify_multiple_locks(
        &self,
        tx_hash: &[u8],
        locks: &[InstantSendLock],
    ) -> Result<bool, RouterError> {
        if locks.is_empty() {
            return Err(RouterError::VerificationError(
                "No InstantSend locks provided".to_string()
            ));
        }
        
        for lock in locks {
            let is_valid = self.verify_instantsend_lock(
                tx_hash,
                &lock.signature,
                &lock.quorum_public_key,
                lock.quorum_size,
            )?;
            
            if !is_valid {
                return Ok(false);
            }
        }
        
        Ok(true)
    }
    
    /// Check if a transaction has sufficient InstantSend locks
    /// 
    /// # Arguments
    /// * `num_inputs` - Number of transaction inputs
    /// * `num_locks` - Number of InstantSend locks received
    /// 
    /// # Returns
    /// * `true` if there are enough locks (at least one per input)
    pub fn has_sufficient_locks(&self, num_inputs: usize, num_locks: usize) -> bool {
        num_locks >= num_inputs
    }
}

impl Default for InstantSendVerifier {
    fn default() -> Self {
        // Default minimum quorum size is 60% of a typical 50-node quorum = 30 nodes
        Self::new(30)
    }
}

/// Represents an InstantSend lock from a masternode quorum
#[derive(Debug, Clone)]
pub struct InstantSendLock {
    /// The aggregated BLS signature from the quorum
    pub signature: Vec<u8>,
    /// The aggregated public key of the quorum
    pub quorum_public_key: Vec<u8>,
    /// The number of masternodes in the quorum
    pub quorum_size: usize,
    /// The quorum hash (identifier)
    pub quorum_hash: [u8; 32],
}

impl InstantSendLock {
    /// Create a new InstantSend lock
    pub fn new(
        signature: Vec<u8>,
        quorum_public_key: Vec<u8>,
        quorum_size: usize,
        quorum_hash: [u8; 32],
    ) -> Self {
        Self {
            signature,
            quorum_public_key,
            quorum_size,
            quorum_hash,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_instantsend_verifier_creation() {
        let verifier = InstantSendVerifier::new(30);
        assert_eq!(verifier.min_quorum_size, 30);
    }
    
    #[test]
    fn test_default_verifier() {
        let verifier = InstantSendVerifier::default();
        assert_eq!(verifier.min_quorum_size, 30);
    }
    
    #[test]
    fn test_has_sufficient_locks() {
        let verifier = InstantSendVerifier::default();
        
        // Test with sufficient locks
        assert!(verifier.has_sufficient_locks(2, 2));
        assert!(verifier.has_sufficient_locks(2, 3));
        
        // Test with insufficient locks
        assert!(!verifier.has_sufficient_locks(3, 2));
        assert!(!verifier.has_sufficient_locks(1, 0));
    }
    
    #[test]
    fn test_verify_instantsend_lock_invalid_tx_hash() {
        let verifier = InstantSendVerifier::default();
        
        // Invalid tx_hash length
        let tx_hash = vec![0u8; 16]; // Should be 32 bytes
        let quorum_sig = vec![0u8; 96];
        let quorum_pk = vec![0u8; 48];
        
        let result = verifier.verify_instantsend_lock(
            &tx_hash,
            &quorum_sig,
            &quorum_pk,
            50,
        );
        
        assert!(result.is_err());
    }
    
    #[test]
    fn test_verify_instantsend_lock_insufficient_quorum() {
        let verifier = InstantSendVerifier::new(40);
        
        let tx_hash = [0u8; 32];
        let quorum_sig = vec![0u8; 96];
        let quorum_pk = vec![0u8; 48];
        
        // Quorum size below minimum
        let result = verifier.verify_instantsend_lock(
            &tx_hash,
            &quorum_sig,
            &quorum_pk,
            30, // Below minimum of 40
        );
        
        assert!(result.is_err());
    }
    
    #[test]
    fn test_instantsend_lock_creation() {
        let lock = InstantSendLock::new(
            vec![0u8; 96],
            vec![0u8; 48],
            50,
            [0u8; 32],
        );
        
        assert_eq!(lock.signature.len(), 96);
        assert_eq!(lock.quorum_public_key.len(), 48);
        assert_eq!(lock.quorum_size, 50);
    }
    
    #[test]
    fn test_verify_multiple_locks_empty() {
        let verifier = InstantSendVerifier::default();
        let tx_hash = [0u8; 32];
        let locks = vec![];
        
        let result = verifier.verify_multiple_locks(&tx_hash, &locks);
        assert!(result.is_err());
    }
}
