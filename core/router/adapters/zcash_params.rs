// Zcash Sapling and Orchard parameter management
// Handles downloading and caching of verifying keys

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::OnceCell;
use crate::router::RouterError;

/// URLs for Zcash parameter files
const SAPLING_SPEND_PARAMS_URL: &str = "https://download.z.cash/downloads/sapling-spend.params";
const SAPLING_OUTPUT_PARAMS_URL: &str = "https://download.z.cash/downloads/sapling-output.params";

/// File sizes for validation
const SAPLING_SPEND_PARAMS_SIZE: usize = 47958396;
const SAPLING_OUTPUT_PARAMS_SIZE: usize = 3592860;

/// Global cache for Sapling parameters
static SAPLING_SPEND_PARAMS: OnceCell<Arc<Vec<u8>>> = OnceCell::const_new();
static SAPLING_OUTPUT_PARAMS: OnceCell<Arc<Vec<u8>>> = OnceCell::const_new();

/// Get the directory for storing Zcash parameters
fn get_params_dir() -> Result<PathBuf, RouterError> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| RouterError::VerificationError("Cannot determine home directory".to_string()))?;
    
    let params_dir = home_dir.join(".zcash-params");
    
    // Create directory if it doesn't exist
    if !params_dir.exists() {
        fs::create_dir_all(&params_dir)
            .map_err(|e| RouterError::VerificationError(format!("Failed to create params directory: {}", e)))?;
    }
    
    Ok(params_dir)
}

/// Download a parameter file if it doesn't exist locally
async fn download_params(url: &str, expected_size: usize, file_path: &Path) -> Result<(), RouterError> {
    // Check if file already exists and has correct size
    if file_path.exists() {
        let metadata = fs::metadata(file_path)
            .map_err(|e| RouterError::VerificationError(format!("Failed to read file metadata: {}", e)))?;
        
        if metadata.len() as usize == expected_size {
            tracing::info!("Parameter file already exists: {:?}", file_path);
            return Ok(());
        } else {
            tracing::warn!("Parameter file has incorrect size, re-downloading: {:?}", file_path);
        }
    }
    
    tracing::info!("Downloading Zcash parameters from {}", url);
    
    // Download the file
    let response = reqwest::get(url)
        .await
        .map_err(|e| RouterError::VerificationError(format!("Failed to download params: {}", e)))?;
    
    if !response.status().is_success() {
        return Err(RouterError::VerificationError(
            format!("Failed to download params: HTTP {}", response.status())
        ));
    }
    
    let bytes = response.bytes()
        .await
        .map_err(|e| RouterError::VerificationError(format!("Failed to read response: {}", e)))?;
    
    // Validate size
    if bytes.len() != expected_size {
        return Err(RouterError::VerificationError(
            format!("Downloaded file has incorrect size: expected {}, got {}", expected_size, bytes.len())
        ));
    }
    
    // Write to file
    fs::write(file_path, &bytes)
        .map_err(|e| RouterError::VerificationError(format!("Failed to write params file: {}", e)))?;
    
    tracing::info!("Successfully downloaded parameter file: {:?}", file_path);
    
    Ok(())
}

/// Load Sapling spend parameters
pub async fn load_sapling_spend_params() -> Result<Arc<Vec<u8>>, RouterError> {
    SAPLING_SPEND_PARAMS.get_or_try_init(|| async {
        let params_dir = get_params_dir()?;
        let file_path = params_dir.join("sapling-spend.params");
        
        // Download if necessary
        download_params(SAPLING_SPEND_PARAMS_URL, SAPLING_SPEND_PARAMS_SIZE, &file_path).await?;
        
        // Load from file
        let bytes = fs::read(&file_path)
            .map_err(|e| RouterError::VerificationError(format!("Failed to read params file: {}", e)))?;
        
        Ok(Arc::new(bytes))
    }).await.cloned()
}

/// Load Sapling output parameters
pub async fn load_sapling_output_params() -> Result<Arc<Vec<u8>>, RouterError> {
    SAPLING_OUTPUT_PARAMS.get_or_try_init(|| async {
        let params_dir = get_params_dir()?;
        let file_path = params_dir.join("sapling-output.params");
        
        // Download if necessary
        download_params(SAPLING_OUTPUT_PARAMS_URL, SAPLING_OUTPUT_PARAMS_SIZE, &file_path).await?;
        
        // Load from file
        let bytes = fs::read(&file_path)
            .map_err(|e| RouterError::VerificationError(format!("Failed to read params file: {}", e)))?;
        
        Ok(Arc::new(bytes))
    }).await.cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[ignore] // Ignore by default as it downloads large files
    async fn test_load_sapling_params() {
        let spend_params = load_sapling_spend_params().await;
        assert!(spend_params.is_ok());
        
        let output_params = load_sapling_output_params().await;
        assert!(output_params.is_ok());
    }
}
