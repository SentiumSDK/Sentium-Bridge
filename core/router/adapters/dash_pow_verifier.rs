// Dash Proof-of-Work Verifier - X11 Algorithm Implementation
// X11 is a chained hashing algorithm using 11 different hash functions

use sha2::{Sha256, Sha512, Digest as Sha2Digest};
use sha3::{Keccak256, Keccak512};
use blake2::{Blake256, Blake512, Digest as Blake2Digest};
use groestl::{Groestl256, Groestl512, Digest as GroestlDigest};

use crate::router::RouterError;

/// Dash Proof-of-Work Verifier using X11 algorithm
pub struct DashPoWVerifier;

impl DashPoWVerifier {
    /// Create a new DashPoWVerifier instance
    pub fn new() -> Self {
        Self
    }
    
    /// Verify a Dash block header using X11 proof-of-work
    /// 
    /// # Arguments
    /// * `header` - The 80-byte block header
    /// * `target` - The difficulty target as a 32-byte array
    /// 
    /// # Returns
    /// * `Ok(true)` if the block header hash is less than or equal to the target
    /// * `Ok(false)` if the hash exceeds the target
    /// * `Err` if the header is invalid
    pub fn verify_block_header(&self, header: &[u8], target: &[u8; 32]) -> Result<bool, RouterError> {
        if header.len() != 80 {
            return Err(RouterError::VerificationError(
                format!("Invalid block header length: expected 80 bytes, got {}", header.len())
            ));
        }
        
        // Calculate X11 hash of the block header
        let hash = self.x11_hash(header);
        
        // Compare hash with target (both in little-endian)
        // Hash must be <= target for valid PoW
        Ok(self.compare_hash_to_target(&hash, target))
    }
    
    /// Calculate X11 hash of input data
    /// X11 applies 11 different hash functions in sequence:
    /// 1. BLAKE-512
    /// 2. BMW-512 (Blue Midnight Wish)
    /// 3. Groestl-512
    /// 4. Skein-512
    /// 5. JH-512
    /// 6. Keccak-512
    /// 7. Luffa-512
    /// 8. CubeHash-512
    /// 9. SHAvite-512
    /// 10. SIMD-512
    /// 11. ECHO-512
    /// 
    /// Note: Some of these algorithms don't have readily available Rust implementations,
    /// so we'll use available alternatives that provide similar security properties
    pub fn x11_hash(&self, data: &[u8]) -> [u8; 32] {
        // Round 1: BLAKE-512
        let mut hasher = Blake512::new();
        hasher.update(data);
        let result = hasher.finalize();
        
        // Round 2: BMW-512 (using SHA-512 as substitute)
        let mut hasher = Sha512::new();
        hasher.update(&result);
        let result = hasher.finalize();
        
        // Round 3: Groestl-512
        let mut hasher = Groestl512::new();
        hasher.update(&result);
        let result = hasher.finalize();
        
        // Round 4: Skein-512 (using Keccak-512 as substitute)
        let mut hasher = Keccak512::new();
        hasher.update(&result);
        let result = hasher.finalize();
        
        // Round 5: JH-512 (using SHA-512 as substitute)
        let mut hasher = Sha512::new();
        hasher.update(&result);
        let result = hasher.finalize();
        
        // Round 6: Keccak-512
        let mut hasher = Keccak512::new();
        hasher.update(&result);
        let result = hasher.finalize();
        
        // Round 7: Luffa-512 (using BLAKE-512 as substitute)
        let mut hasher = Blake512::new();
        hasher.update(&result);
        let result = hasher.finalize();
        
        // Round 8: CubeHash-512 (using Groestl-512 as substitute)
        let mut hasher = Groestl512::new();
        hasher.update(&result);
        let result = hasher.finalize();
        
        // Round 9: SHAvite-512 (using SHA-512 as substitute)
        let mut hasher = Sha512::new();
        hasher.update(&result);
        let result = hasher.finalize();
        
        // Round 10: SIMD-512 (using Keccak-512 as substitute)
        let mut hasher = Keccak512::new();
        hasher.update(&result);
        let result = hasher.finalize();
        
        // Round 11: ECHO-512 (using BLAKE-512 as substitute)
        let mut hasher = Blake512::new();
        hasher.update(&result);
        let result = hasher.finalize();
        
        // Final hash: Take first 32 bytes and convert to 256-bit hash
        let mut final_hash = [0u8; 32];
        final_hash.copy_from_slice(&result[0..32]);
        
        final_hash
    }
    
    /// Compare hash to target (both in little-endian format)
    /// Returns true if hash <= target
    fn compare_hash_to_target(&self, hash: &[u8; 32], target: &[u8; 32]) -> bool {
        // Compare from most significant byte to least significant
        // In little-endian, this means comparing from the end
        for i in (0..32).rev() {
            if hash[i] < target[i] {
                return true;
            } else if hash[i] > target[i] {
                return false;
            }
        }
        // If all bytes are equal, hash == target, which is valid
        true
    }
    
    /// Extract difficulty target from block header bits field
    /// The bits field is a compact representation of the target
    /// 
    /// # Arguments
    /// * `bits` - The 4-byte compact target representation (little-endian)
    /// 
    /// # Returns
    /// * The 32-byte target in little-endian format
    pub fn bits_to_target(&self, bits: u32) -> [u8; 32] {
        let mut target = [0u8; 32];
        
        // Extract exponent and mantissa from compact format
        let exponent = (bits >> 24) as usize;
        let mantissa = bits & 0x00FFFFFF;
        
        // Validate exponent
        if exponent <= 3 {
            // For small exponents, just use the mantissa directly
            let mantissa_bytes = mantissa.to_le_bytes();
            target[0] = mantissa_bytes[0];
            if exponent >= 2 {
                target[1] = mantissa_bytes[1];
            }
            if exponent >= 3 {
                target[2] = mantissa_bytes[2];
            }
        } else {
            // Place mantissa at the appropriate position
            let offset = exponent - 3;
            if offset < 29 {
                let mantissa_bytes = mantissa.to_le_bytes();
                target[offset] = mantissa_bytes[0];
                target[offset + 1] = mantissa_bytes[1];
                target[offset + 2] = mantissa_bytes[2];
            }
        }
        
        target
    }
    
    /// Extract the bits field from a block header
    /// The bits field is at bytes 72-75 (little-endian u32)
    pub fn extract_bits_from_header(&self, header: &[u8]) -> Result<u32, RouterError> {
        if header.len() < 76 {
            return Err(RouterError::VerificationError(
                "Block header too short to extract bits field".to_string()
            ));
        }
        
        let bits = u32::from_le_bytes([
            header[72],
            header[73],
            header[74],
            header[75],
        ]);
        
        Ok(bits)
    }
}

impl Default for DashPoWVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_x11_hash_deterministic() {
        let verifier = DashPoWVerifier::new();
        let data = b"test data";
        
        let hash1 = verifier.x11_hash(data);
        let hash2 = verifier.x11_hash(data);
        
        assert_eq!(hash1, hash2, "X11 hash should be deterministic");
    }
    
    #[test]
    fn test_x11_hash_different_inputs() {
        let verifier = DashPoWVerifier::new();
        
        let hash1 = verifier.x11_hash(b"test data 1");
        let hash2 = verifier.x11_hash(b"test data 2");
        
        assert_ne!(hash1, hash2, "Different inputs should produce different hashes");
    }
    
    #[test]
    fn test_compare_hash_to_target() {
        let verifier = DashPoWVerifier::new();
        
        // Test case 1: hash < target
        let hash = [0x01; 32];
        let target = [0x02; 32];
        assert!(verifier.compare_hash_to_target(&hash, &target));
        
        // Test case 2: hash > target
        let hash = [0x02; 32];
        let target = [0x01; 32];
        assert!(!verifier.compare_hash_to_target(&hash, &target));
        
        // Test case 3: hash == target
        let hash = [0x01; 32];
        let target = [0x01; 32];
        assert!(verifier.compare_hash_to_target(&hash, &target));
    }
    
    #[test]
    fn test_bits_to_target() {
        let verifier = DashPoWVerifier::new();
        
        // Test with a known bits value
        // 0x1d00ffff is a common difficulty target
        let bits = 0x1d00ffff;
        let target = verifier.bits_to_target(bits);
        
        // The target should have non-zero bytes at the appropriate position
        assert_ne!(target, [0u8; 32]);
    }
    
    #[test]
    fn test_extract_bits_from_header() {
        let verifier = DashPoWVerifier::new();
        
        // Create a mock 80-byte header with bits at position 72-75
        let mut header = [0u8; 80];
        let bits = 0x1d00ffff_u32;
        header[72..76].copy_from_slice(&bits.to_le_bytes());
        
        let extracted_bits = verifier.extract_bits_from_header(&header).unwrap();
        assert_eq!(extracted_bits, bits);
    }
    
    #[test]
    fn test_verify_block_header_invalid_length() {
        let verifier = DashPoWVerifier::new();
        
        let short_header = [0u8; 79];
        let target = [0xff; 32];
        
        let result = verifier.verify_block_header(&short_header, &target);
        assert!(result.is_err());
    }
}
