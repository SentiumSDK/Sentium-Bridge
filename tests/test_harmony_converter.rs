// Standalone test for Harmony address converter
use bech32::{ToBase32, FromBase32, Variant};

#[derive(Debug)]
enum TestError {
    Bech32Error(String),
    HexError(String),
    ValidationError(String),
}

impl std::fmt::Display for TestError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            TestError::Bech32Error(s) => write!(f, "Bech32 error: {}", s),
            TestError::HexError(s) => write!(f, "Hex error: {}", s),
            TestError::ValidationError(s) => write!(f, "Validation error: {}", s),
        }
    }
}

impl std::error::Error for TestError {}

/// Convert a Harmony ONE address (bech32 format) to Ethereum hex format
fn one_to_eth(one_address: &str) -> Result<String, TestError> {
    // Decode bech32 address
    let (hrp, data, variant) = bech32::decode(one_address)
        .map_err(|e| TestError::Bech32Error(format!("Failed to decode: {}", e)))?;
    
    // Verify HRP is "one"
    if hrp != "one" {
        return Err(TestError::ValidationError(format!(
            "Invalid HRP: expected 'one', got '{}'", hrp
        )));
    }
    
    // Verify variant
    if variant != Variant::Bech32 {
        return Err(TestError::ValidationError(
            "Invalid variant".to_string()
        ));
    }
    
    // Convert from base32 to bytes
    let bytes = Vec::<u8>::from_base32(&data)
        .map_err(|e| TestError::Bech32Error(format!("Failed to decode base32: {}", e)))?;
    
    // Verify length
    if bytes.len() != 20 {
        return Err(TestError::ValidationError(format!(
            "Invalid length: expected 20, got {}", bytes.len()
        )));
    }
    
    // Format as hex
    Ok(format!("0x{}", hex::encode(bytes)))
}

/// Convert an Ethereum hex address to Harmony ONE address (bech32 format)
fn eth_to_one(eth_address: &str) -> Result<String, TestError> {
    // Remove 0x prefix
    let hex_str = eth_address.strip_prefix("0x")
        .unwrap_or(eth_address);
    
    // Validate length
    if hex_str.len() != 40 {
        return Err(TestError::ValidationError(format!(
            "Invalid length: expected 40, got {}", hex_str.len()
        )));
    }
    
    // Decode hex
    let bytes = hex::decode(hex_str)
        .map_err(|e| TestError::HexError(format!("Failed to decode: {}", e)))?;
    
    // Convert to base32
    let data = bytes.to_base32();
    
    // Encode as bech32
    bech32::encode("one", data, Variant::Bech32)
        .map_err(|e| TestError::Bech32Error(format!("Failed to encode: {}", e)))
}

fn main() {
    println!("Testing Harmony Address Converter\n");
    
    // Test 1: Convert ETH to ONE
    println!("Test 1: ETH to ONE conversion");
    let eth_addr = "0x0000000000000000000000000000000000000000";
    match eth_to_one(eth_addr) {
        Ok(one_addr) => {
            println!("  ETH: {}", eth_addr);
            println!("  ONE: {}", one_addr);
            println!("  ✓ Success\n");
        }
        Err(e) => {
            println!("  ✗ Failed: {}\n", e);
        }
    }
    
    // Test 2: Convert ONE to ETH
    println!("Test 2: ONE to ETH conversion");
    let one_addr = "one1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq9yrzh5";
    match one_to_eth(one_addr) {
        Ok(eth_addr) => {
            println!("  ONE: {}", one_addr);
            println!("  ETH: {}", eth_addr);
            println!("  ✓ Success\n");
        }
        Err(e) => {
            println!("  ✗ Failed: {}\n", e);
        }
    }
    
    // Test 3: Roundtrip conversion
    println!("Test 3: Roundtrip conversion (ETH -> ONE -> ETH)");
    let original = "0x1234567890123456789012345678901234567890";
    match eth_to_one(original) {
        Ok(one_addr) => {
            match one_to_eth(&one_addr) {
                Ok(final_eth) => {
                    println!("  Original: {}", original);
                    println!("  ONE:      {}", one_addr);
                    println!("  Final:    {}", final_eth);
                    if original.to_lowercase() == final_eth.to_lowercase() {
                        println!("  ✓ Roundtrip successful\n");
                    } else {
                        println!("  ✗ Roundtrip failed: addresses don't match\n");
                    }
                }
                Err(e) => println!("  ✗ ONE to ETH failed: {}\n", e),
            }
        }
        Err(e) => println!("  ✗ ETH to ONE failed: {}\n", e),
    }
    
    // Test 4: Invalid HRP
    println!("Test 4: Invalid HRP (should fail)");
    let invalid = "eth1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq9yrzh5";
    match one_to_eth(invalid) {
        Ok(_) => println!("  ✗ Should have failed but succeeded\n"),
        Err(e) => println!("  ✓ Correctly rejected: {}\n", e),
    }
    
    // Test 5: Invalid hex length
    println!("Test 5: Invalid hex length (should fail)");
    let invalid = "0x1234";
    match eth_to_one(invalid) {
        Ok(_) => println!("  ✗ Should have failed but succeeded\n"),
        Err(e) => println!("  ✓ Correctly rejected: {}\n", e),
    }
    
    println!("All tests completed!");
}
