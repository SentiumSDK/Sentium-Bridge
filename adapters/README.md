# Chain Adapters

This directory contains chain-specific adapters for the Sentium Bridge Protocol.

## Structure

```
adapters/
├── ethereum/       # Ethereum adapter (Rust + Go)
├── polkadot/       # Polkadot adapter (Rust)
├── bitcoin/        # Bitcoin adapter (Rust)
├── cosmos/         # Cosmos adapter (Go + Rust)
└── sentium/        # Sentium native adapter (Rust)
```

## Adapter Interface

Each adapter must implement the `ChainAdapter` trait:

```rust
#[async_trait]
pub trait ChainAdapter: Send + Sync {
    fn chain_name(&self) -> &str;
    fn chain_id(&self) -> &str;
    
    async fn translate_intent(&self, intent: &Intent) -> Result<TranslatedIntent, RouterError>;
    async fn verify_state(&self, proof: &[u8]) -> Result<bool, RouterError>;
    async fn submit_transaction(&self, tx_data: &[u8]) -> Result<String, RouterError>;
    async fn query_balance(&self, address: &str, asset: &str) -> Result<u64, RouterError>;
}
```

## Adding a New Chain

1. Create a new directory for the chain
2. Implement the `ChainAdapter` trait
3. Add chain-specific RPC/API integration
4. Implement state verification logic
5. Add tests

## Language Usage

- **Rust**: Core adapter logic, cryptography, state verification
- **Go**: Networking, light clients (for chains with Go SDKs)
- **Python**: AI-powered optimizations (optional)
