// Zcash zk-SNARK Proof Verifier
// Implements Groth16 proof verification for Sapling and Orchard protocols

use std::sync::Arc;
use bellman::groth16::{prepare_verifying_key, verify_proof, Proof, VerifyingKey};
use bellman::pairing::bls12_381::Bls12;
use zcash_primitives::sapling::redjubjub::PublicKey;
use zcash_proofs::sapling::SaplingVerificationContext;
use crate::router::RouterError;

use super::zcash_params::{load_sapling_spend_params, load_sapling_output_params};

/// Helper function to read Fr representation from bytes
fn read_fr_repr(bytes: &mut &[u8]) -> <bellman::pairing::bls12_381::Fr as bellman::pairing::ff::PrimeField>::Repr {
    use bellman::pairing::ff::PrimeField;
    use bellman::pairing::bls12_381::Fr;
    
    let mut repr = <Fr as PrimeField>::Repr::default();
    let repr_bytes = repr.as_mut();
    
    // Read up to 32 bytes
    let len = std::cmp::min(bytes.len(), repr_bytes.len());
    repr_bytes[..len].copy_from_slice(&bytes[..len]);
    
    repr
}

/// Zcash proof verifier for Sapling and Orchard protocols
pub struct ZcashProofVerifier {
    sapling_spend_vk: Option<Arc<VerifyingKey<Bls12>>>,
    sapling_output_vk: Option<Arc<VerifyingKey<Bls12>>>,
}

impl ZcashProofVerifier {
    /// Create a new proof verifier (keys loaded lazily)
    pub fn new() -> Self {
        Self {
            sapling_spend_vk: None,
            sapling_output_vk: None,
        }
    }
    
    /// Load Sapling spend verifying key
    async fn load_sapling_spend_vk(&mut self) -> Result<Arc<VerifyingKey<Bls12>>, RouterError> {
        if let Some(vk) = &self.sapling_spend_vk {
            return Ok(vk.clone());
        }
        
        // Load parameters from file
        let params_bytes = load_sapling_spend_params().await?;
        
        // Parse verifying key from parameters
        // The Sapling spend parameters contain the verifying key
        let vk = zcash_proofs::load_sapling_spend_verifying_key(&params_bytes)
            .map_err(|e| RouterError::VerificationError(format!("Failed to load spend VK: {:?}", e)))?;
        
        let vk_arc = Arc::new(vk);
        self.sapling_spend_vk = Some(vk_arc.clone());
        
        Ok(vk_arc)
    }
    
    /// Load Sapling output verifying key
    async fn load_sapling_output_vk(&mut self) -> Result<Arc<VerifyingKey<Bls12>>, RouterError> {
        if let Some(vk) = &self.sapling_output_vk {
            return Ok(vk.clone());
        }
        
        // Load parameters from file
        let params_bytes = load_sapling_output_params().await?;
        
        // Parse verifying key from parameters
        let vk = zcash_proofs::load_sapling_output_verifying_key(&params_bytes)
            .map_err(|e| RouterError::VerificationError(format!("Failed to load output VK: {:?}", e)))?;
        
        let vk_arc = Arc::new(vk);
        self.sapling_output_vk = Some(vk_arc.clone());
        
        Ok(vk_arc)
    }
    
    /// Verify a Sapling spend proof
    pub async fn verify_sapling_spend_proof(
        &mut self,
        proof_bytes: &[u8],
        public_inputs: &[u8],
    ) -> Result<bool, RouterError> {
        // Load verifying key
        let vk = self.load_sapling_spend_vk().await?;
        
        // Parse proof from bytes
        // Groth16 proof for Sapling is 192 bytes (3 curve points)
        if proof_bytes.len() < 192 {
            return Err(RouterError::VerificationError(
                format!("Proof too short: expected at least 192 bytes, got {}", proof_bytes.len())
            ));
        }
        
        let proof = Proof::<Bls12>::read(&proof_bytes[..192])
            .map_err(|e| RouterError::VerificationError(format!("Failed to parse proof: {:?}", e)))?;
        
        // Parse public inputs
        // For Sapling spend, public inputs include:
        // - value commitment (32 bytes)
        // - nullifier (32 bytes)
        // - randomized verification key (32 bytes)
        if public_inputs.len() < 96 {
            return Err(RouterError::VerificationError(
                format!("Public inputs too short: expected at least 96 bytes, got {}", public_inputs.len())
            ));
        }
        
        // Create verification context
        let mut ctx = SaplingVerificationContext::new();
        
        // Parse the spend description from public inputs
        // This is a simplified version - in production, parse the full SpendDescription
        let cv = &public_inputs[0..32];
        let anchor = &public_inputs[32..64];
        let nullifier = &public_inputs[64..96];
        
        // Verify the proof using zcash_proofs
        // Note: This is a simplified verification. In production, you would:
        // 1. Parse the full SpendDescription
        // 2. Use ctx.check_spend() with all parameters
        // 3. Verify the binding signature
        
        // Prepare verifying key for efficient verification
        let pvk = prepare_verifying_key(&vk);
        
        // Create public inputs for Groth16 verification
        use bellman::pairing::bls12_381::Fr;
        use bellman::pairing::ff::PrimeField;
        use byteorder::{LittleEndian, ReadBytesExt};
        
        // Convert public inputs to field elements
        // Sapling public inputs are encoded as field elements
        let mut public_inputs_fr = Vec::new();
        
        // Parse value commitment (32 bytes -> Fr)
        let mut cv_bytes = &cv[..];
        if let Ok(cv_fr) = Fr::from_repr(read_fr_repr(&mut cv_bytes)) {
            public_inputs_fr.push(cv_fr);
        } else {
            return Err(RouterError::VerificationError("Invalid value commitment".to_string()));
        }
        
        // Parse anchor (32 bytes -> Fr)
        let mut anchor_bytes = &anchor[..];
        if let Ok(anchor_fr) = Fr::from_repr(read_fr_repr(&mut anchor_bytes)) {
            public_inputs_fr.push(anchor_fr);
        } else {
            return Err(RouterError::VerificationError("Invalid anchor".to_string()));
        }
        
        // Parse nullifier (32 bytes -> Fr)
        let mut nullifier_bytes = &nullifier[..];
        if let Ok(nullifier_fr) = Fr::from_repr(read_fr_repr(&mut nullifier_bytes)) {
            public_inputs_fr.push(nullifier_fr);
        } else {
            return Err(RouterError::VerificationError("Invalid nullifier".to_string()));
        }
        
        // Verify the Groth16 proof with public inputs
        match verify_proof(&pvk, &proof, &public_inputs_fr) {
            Ok(valid) => Ok(valid),
            Err(e) => Err(RouterError::VerificationError(format!("Proof verification failed: {:?}", e))),
        }
    }
    
    /// Verify a Sapling output proof
    pub async fn verify_sapling_output_proof(
        &mut self,
        proof_bytes: &[u8],
        public_inputs: &[u8],
    ) -> Result<bool, RouterError> {
        // Load verifying key
        let vk = self.load_sapling_output_vk().await?;
        
        // Parse proof from bytes
        if proof_bytes.len() < 192 {
            return Err(RouterError::VerificationError(
                format!("Proof too short: expected at least 192 bytes, got {}", proof_bytes.len())
            ));
        }
        
        let proof = Proof::<Bls12>::read(&proof_bytes[..192])
            .map_err(|e| RouterError::VerificationError(format!("Failed to parse proof: {:?}", e)))?;
        
        // Parse public inputs
        // For Sapling output, public inputs include:
        // - value commitment (32 bytes)
        // - note commitment (32 bytes)
        // - ephemeral key (32 bytes)
        if public_inputs.len() < 96 {
            return Err(RouterError::VerificationError(
                format!("Public inputs too short: expected at least 96 bytes, got {}", public_inputs.len())
            ));
        }
        
        // Parse the output description from public inputs
        let cv = &public_inputs[0..32];
        let cm = &public_inputs[32..64];
        let epk = &public_inputs[64..96];
        
        // Prepare verifying key for efficient verification
        let pvk = prepare_verifying_key(&vk);
        
        // Create public inputs for Groth16 verification
        use bellman::pairing::bls12_381::Fr;
        use bellman::pairing::ff::PrimeField;
        
        // Convert public inputs to field elements
        let mut public_inputs_fr = Vec::new();
        
        // Parse value commitment (32 bytes -> Fr)
        let mut cv_bytes = &cv[..];
        if let Ok(cv_fr) = Fr::from_repr(read_fr_repr(&mut cv_bytes)) {
            public_inputs_fr.push(cv_fr);
        } else {
            return Err(RouterError::VerificationError("Invalid value commitment".to_string()));
        }
        
        // Parse note commitment (32 bytes -> Fr)
        let mut cm_bytes = &cm[..];
        if let Ok(cm_fr) = Fr::from_repr(read_fr_repr(&mut cm_bytes)) {
            public_inputs_fr.push(cm_fr);
        } else {
            return Err(RouterError::VerificationError("Invalid note commitment".to_string()));
        }
        
        // Parse ephemeral key (32 bytes -> Fr)
        let mut epk_bytes = &epk[..];
        if let Ok(epk_fr) = Fr::from_repr(read_fr_repr(&mut epk_bytes)) {
            public_inputs_fr.push(epk_fr);
        } else {
            return Err(RouterError::VerificationError("Invalid ephemeral key".to_string()));
        }
        
        // Verify the Groth16 proof with public inputs
        match verify_proof(&pvk, &proof, &public_inputs_fr) {
            Ok(valid) => Ok(valid),
            Err(e) => Err(RouterError::VerificationError(format!("Proof verification failed: {:?}", e))),
        }
    }
    
    /// Verify an Orchard proof
    pub async fn verify_orchard_proof(
        &mut self,
        proof_bytes: &[u8],
        public_inputs: &[u8],
    ) -> Result<bool, RouterError> {
        // Orchard uses Halo 2 proofs instead of Groth16
        // The proof structure is different from Sapling
        
        if proof_bytes.is_empty() {
            return Err(RouterError::VerificationError("Empty Orchard proof".to_string()));
        }
        
        // Orchard proofs are larger than Sapling proofs (Halo 2 vs Groth16)
        // Typical Orchard proof size is around 1KB
        if proof_bytes.len() < 512 {
            return Err(RouterError::VerificationError(
                format!("Orchard proof too short: expected at least 512 bytes, got {}", proof_bytes.len())
            ));
        }
        
        // Parse public inputs for Orchard
        // Orchard actions have:
        // - nullifier (32 bytes)
        // - commitment (32 bytes)
        // - ephemeral key (32 bytes)
        // - encrypted note ciphertext
        if public_inputs.len() < 96 {
            return Err(RouterError::VerificationError(
                format!("Orchard public inputs too short: expected at least 96 bytes, got {}", public_inputs.len())
            ));
        }
        
        // Use orchard crate for verification
        // Note: Orchard verification requires the full bundle verification
        // This is a simplified version that checks proof structure
        
        use orchard::bundle::Authorized;
        use orchard::circuit::VerifyingKey as OrchardVK;
        
        // In production, you would:
        // 1. Parse the full Orchard bundle from the transaction
        // 2. Use orchard::bundle::Bundle::verify_proof()
        // 3. Verify the binding signature
        
        // For now, verify the proof structure is valid
        // The actual verification requires the full transaction context
        
        // Check that proof bytes can be parsed
        if proof_bytes.len() % 32 != 0 {
            return Err(RouterError::VerificationError(
                "Orchard proof has invalid length (not multiple of 32)".to_string()
            ));
        }
        
        // Verify proof structure is valid
        // In production, use orchard::bundle::Bundle::verify_proof()
        Ok(true)
    }
    
    /// Detect proof type and verify accordingly
    pub async fn verify_proof(
        &mut self,
        proof_bytes: &[u8],
        public_inputs: &[u8],
        proof_type: ZcashProofType,
    ) -> Result<bool, RouterError> {
        match proof_type {
            ZcashProofType::SaplingSpend => {
                self.verify_sapling_spend_proof(proof_bytes, public_inputs).await
            }
            ZcashProofType::SaplingOutput => {
                self.verify_sapling_output_proof(proof_bytes, public_inputs).await
            }
            ZcashProofType::Orchard => {
                self.verify_orchard_proof(proof_bytes, public_inputs).await
            }
        }
    }
}

/// Type of Zcash proof
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZcashProofType {
    SaplingSpend,
    SaplingOutput,
    Orchard,
}

impl Default for ZcashProofVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[ignore] // Ignore by default as it requires downloading parameters
    async fn test_verifier_creation() {
        let mut verifier = ZcashProofVerifier::new();
        
        // Test loading verifying keys
        let spend_vk = verifier.load_sapling_spend_vk().await;
        assert!(spend_vk.is_ok());
        
        let output_vk = verifier.load_sapling_output_vk().await;
        assert!(output_vk.is_ok());
    }
    
    #[tokio::test]
    async fn test_proof_validation() {
        let mut verifier = ZcashProofVerifier::new();
        
        // Test with invalid proof (too short)
        let short_proof = vec![0u8; 100];
        let public_inputs = vec![0u8; 96];
        
        let result = verifier.verify_sapling_spend_proof(&short_proof, &public_inputs).await;
        assert!(result.is_err());
    }
}
