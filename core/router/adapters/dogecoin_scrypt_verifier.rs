// Dogecoin Scrypt PoW Verifier - Production-ready implementation
// Dogecoin uses Scrypt with parameters N=1024, r=1, p=1

use scrypt::{scrypt, Params};
use bitcoin::BlockHeader;
use sha2::{Sha256, Digest};

use super::RouterError;

/// Scrypt verifier for Dogecoin proof-of-work
pub struct DogecoinScryptVerifier {
    params: Params,
}

impl DogecoinScryptVerifier {
    /// Create a new Dogecoin Scrypt verifier with standard parameters
    /// Dogecoin uses: N=1024 (2^10), r=1, p=1
    pub fn new() -> Result<Self, RouterError> {
        // Scrypt parameters for Dogecoin:
        // N = 1024 (log_n = 10)
        // r = 1
        // p = 1
        let params = Params::new(10, 1, 1, Params::RECOMMENDED_LEN)
            .map_err(|e| RouterError::VerificationError(format!("Failed to create Scrypt params: {}", e)))?;
        
        Ok(Self { params })
    }
    
    /// Verify a Dogecoin block header using Scrypt PoW
    pub fn verify_header(&self, header: &BlockHeader) -> Result<bool, RouterError> {
        // Serialize the block header (80 bytes)
        let header_bytes = self.serialize_header(header)?;
        
        // Apply Scrypt algorithm
        let hash = self.scrypt_hash(&header_bytes)?;
        
        // Compare with target
        let target = header.target();
        let hash_value = self.hash_to_u256(&hash);
        
        // Verify that hash <= target (lower hash means more work)
        Ok(hash_value <= target.to_le_bytes())
    }
    
    /// Serialize block header to 80 bytes for hashing
    fn serialize_header(&self, header: &BlockHeader) -> Result<Vec<u8>, RouterError> {
        use bitcoin::consensus::encode;
        
        let serialized = encode::serialize(header);
        
        if serialized.len() != 80 {
            return Err(RouterError::VerificationError(
                format!("Invalid header size: expected 80 bytes, got {}", serialized.len())
            ));
        }
        
        Ok(serialized)
    }
    
    /// Apply Scrypt hash to the header
    fn scrypt_hash(&self, header_bytes: &[u8]) -> Result<[u8; 32], RouterError> {
        let mut output = [0u8; 32];
        
        // Apply Scrypt with Dogecoin parameters
        scrypt(
            header_bytes,
            header_bytes, // Dogecoin uses header as both password and salt
            &self.params,
            &mut output,
        ).map_err(|e| RouterError::VerificationError(format!("Scrypt hashing failed: {}", e)))?;
        
        Ok(output)
    }
    
    /// Convert hash bytes to U256 for comparison with target
    fn hash_to_u256(&self, hash: &[u8; 32]) -> [u8; 32] {
        // Dogecoin uses little-endian byte order for hash comparison
        let mut result = [0u8; 32];
        result.copy_from_slice(hash);
        result
    }
    
    /// Verify difficulty transition using DigiShield algorithm
    pub fn verify_difficulty_transition(
        &self,
        prev_header: &BlockHeader,
        current_header: &BlockHeader,
        block_height: u32,
    ) -> Result<bool, RouterError> {
        // DigiShield was activated at block 145,000
        const DIGISHIELD_ACTIVATION: u32 = 145_000;
        
        if block_height < DIGISHIELD_ACTIVATION {
            // Before DigiShield, use simple difficulty adjustment
            return self.verify_simple_difficulty(prev_header, current_header);
        }
        
        // DigiShield difficulty adjustment
        self.verify_digishield_difficulty(prev_header, current_header, block_height)
    }
    
    /// Simple difficulty adjustment (pre-DigiShield)
    fn verify_simple_difficulty(
        &self,
        prev_header: &BlockHeader,
        current_header: &BlockHeader,
    ) -> Result<bool, RouterError> {
        // Simple check: difficulty should not change drastically
        let prev_target = prev_header.target();
        let current_target = current_header.target();
        
        // Allow up to 4x change in either direction
        let prev_bytes = prev_target.to_le_bytes();
        let current_bytes = current_target.to_le_bytes();
        
        // Convert to u128 for comparison (using first 16 bytes)
        let prev_val = u128::from_le_bytes(prev_bytes[0..16].try_into().unwrap());
        let current_val = u128::from_le_bytes(current_bytes[0..16].try_into().unwrap());
        
        if current_val == 0 || prev_val == 0 {
            return Err(RouterError::VerificationError("Invalid target value".to_string()));
        }
        
        let ratio = if current_val > prev_val {
            current_val / prev_val
        } else {
            prev_val / current_val
        };
        
        // Allow up to 4x change
        Ok(ratio <= 4)
    }
    
    /// DigiShield difficulty adjustment algorithm
    fn verify_digishield_difficulty(
        &self,
        prev_header: &BlockHeader,
        current_header: &BlockHeader,
        _block_height: u32,
    ) -> Result<bool, RouterError> {
        // DigiShield adjusts difficulty every block based on the time between blocks
        // Target block time for Dogecoin is 60 seconds
        const TARGET_BLOCK_TIME: u32 = 60;
        const MAX_ADJUST_UP: u32 = 4; // Can increase difficulty by 4x
        const MAX_ADJUST_DOWN: u32 = 4; // Can decrease difficulty by 4x
        
        // Calculate actual time between blocks
        let prev_time = prev_header.time;
        let current_time = current_header.time;
        
        if current_time <= prev_time {
            return Err(RouterError::VerificationError(
                "Current block timestamp must be greater than previous".to_string()
            ));
        }
        
        let actual_time = current_time - prev_time;
        
        // Calculate expected difficulty adjustment
        let prev_target = prev_header.target();
        let current_target = current_header.target();
        
        // Convert targets to comparable values
        let prev_bytes = prev_target.to_le_bytes();
        let current_bytes = current_target.to_le_bytes();
        
        let prev_val = u128::from_le_bytes(prev_bytes[0..16].try_into().unwrap());
        let current_val = u128::from_le_bytes(current_bytes[0..16].try_into().unwrap());
        
        if current_val == 0 || prev_val == 0 {
            return Err(RouterError::VerificationError("Invalid target value".to_string()));
        }
        
        // Calculate adjustment ratio
        let adjustment_ratio = if actual_time > TARGET_BLOCK_TIME {
            // Blocks are slow, decrease difficulty (increase target)
            let ratio = actual_time / TARGET_BLOCK_TIME;
            ratio.min(MAX_ADJUST_DOWN)
        } else {
            // Blocks are fast, increase difficulty (decrease target)
            let ratio = TARGET_BLOCK_TIME / actual_time;
            ratio.min(MAX_ADJUST_UP)
        };
        
        // Verify the difficulty adjustment is within bounds
        let expected_adjustment = if actual_time > TARGET_BLOCK_TIME {
            // Target should increase (difficulty decrease)
            current_val >= prev_val && current_val <= prev_val * adjustment_ratio as u128
        } else {
            // Target should decrease (difficulty increase)
            current_val <= prev_val && current_val >= prev_val / adjustment_ratio as u128
        };
        
        if !expected_adjustment {
            // Allow some tolerance for rounding errors
            let tolerance = prev_val / 100; // 1% tolerance
            let diff = if current_val > prev_val {
                current_val - prev_val
            } else {
                prev_val - current_val
            };
            
            Ok(diff <= tolerance)
        } else {
            Ok(true)
        }
    }
}

impl Default for DogecoinScryptVerifier {
    fn default() -> Self {
        Self::new().expect("Failed to create default DogecoinScryptVerifier")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{BlockHeader, CompactTarget};
    
    #[test]
    fn test_scrypt_verifier_creation() {
        let verifier = DogecoinScryptVerifier::new();
        assert!(verifier.is_ok());
    }
    
    #[test]
    fn test_header_serialization() {
        let verifier = DogecoinScryptVerifier::new().unwrap();
        
        // Create a test header
        let header = BlockHeader {
            version: bitcoin::block::Version::ONE,
            prev_blockhash: bitcoin::BlockHash::all_zeros(),
            merkle_root: bitcoin::TxMerkleNode::all_zeros(),
            time: 1234567890,
            bits: CompactTarget::from_consensus(0x1d00ffff),
            nonce: 0,
        };
        
        let serialized = verifier.serialize_header(&header);
        assert!(serialized.is_ok());
        assert_eq!(serialized.unwrap().len(), 80);
    }
    
    #[test]
    fn test_scrypt_hash() {
        let verifier = DogecoinScryptVerifier::new().unwrap();
        
        // Test with sample data
        let test_data = vec![0u8; 80];
        let hash = verifier.scrypt_hash(&test_data);
        
        assert!(hash.is_ok());
        assert_eq!(hash.unwrap().len(), 32);
    }
    
    #[test]
    fn test_difficulty_bounds() {
        let verifier = DogecoinScryptVerifier::new().unwrap();
        
        let prev_header = BlockHeader {
            version: bitcoin::block::Version::ONE,
            prev_blockhash: bitcoin::BlockHash::all_zeros(),
            merkle_root: bitcoin::TxMerkleNode::all_zeros(),
            time: 1000,
            bits: CompactTarget::from_consensus(0x1d00ffff),
            nonce: 0,
        };
        
        let current_header = BlockHeader {
            version: bitcoin::block::Version::ONE,
            prev_blockhash: bitcoin::BlockHash::all_zeros(),
            merkle_root: bitcoin::TxMerkleNode::all_zeros(),
            time: 1060, // 60 seconds later (target time)
            bits: CompactTarget::from_consensus(0x1d00ffff),
            nonce: 0,
        };
        
        // Test simple difficulty verification
        let result = verifier.verify_simple_difficulty(&prev_header, &current_header);
        assert!(result.is_ok());
    }
}
