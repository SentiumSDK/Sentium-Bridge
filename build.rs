use std::io::Result;

fn main() -> Result<()> {
    // Configure prost-build to generate Rust code from TRON .proto files
    let mut config = prost_build::Config::new();
    
    // Enable type attributes for better ergonomics
    config.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");
    
    // Compile TRON protocol buffer files
    config.compile_protos(
        &[
            "proto/tron/Tron.proto",
            "proto/tron/smart_contract.proto",
            "proto/tron/balance_contract.proto",
            "proto/tron/common.proto",
            "proto/tron/Discover.proto",
        ],
        &["proto/tron/"],
    )?;
    
    // Tell Cargo to rerun this build script if proto files change
    println!("cargo:rerun-if-changed=proto/tron/");
    
    Ok(())
}
