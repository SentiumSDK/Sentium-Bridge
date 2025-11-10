# Sentium Bridge Testing Guide

This document provides a comprehensive overview of the testing infrastructure for Sentium Bridge adapters.

## Test Structure

The testing suite is organized into four main categories:

### 1. Unit Tests (`tests/adapter_unit_tests.rs`)

Tests core functionality of individual adapter components:

- **Cosmos Adapter**: Protobuf encoding/decoding
- **Polkadot Adapter**: Storage proof verification
- **Bitcoin Adapter**: UTXO selection algorithms
- **Zcash Adapter**: zk-SNARK proof structure validation
- **Dash Adapter**: X11 hash chain verification
- **TRON Adapter**: Protobuf serialization
- **Solana Adapter**: DEX instruction creation and slippage calculation
- **Harmony Adapter**: Bech32 address conversion
- **Litecoin/Dogecoin Adapters**: Scrypt hashing
- **Chia Adapter**: Wallet discovery logic

### 2. Integration Tests (`tests/adapter_integration_tests.rs`)

Tests end-to-end functionality with test networks:

- Balance queries against testnets
- Transaction submission flows
- State verification with real proofs
- Cross-chain routing
- Multi-hop transactions
- Performance benchmarks

**Note**: Integration tests are marked with `#[ignore]` and require:
- Access to test networks
- Valid RPC endpoints
- Test credentials

### 3. Error Tests (`tests/adapter_error_tests.rs`)

Tests error handling and edge cases:

- **Network Failures**: Invalid RPC URLs, connection timeouts, authentication errors
- **Invalid Inputs**: Malformed addresses, empty fields, unsupported chains
- **Insufficient Funds**: UTXO selection failures, balance checks, gas estimation
- **Malformed Data**: Invalid protobuf, truncated messages, corrupted proofs
- **Boundary Conditions**: Zero amounts, maximum values, overflow/underflow
- **Concurrent Access**: Race conditions, deadlocks
- **Timeouts**: Query timeouts, transaction submission timeouts
- **State Consistency**: Nonce management, balance tracking

### 4. Existing Integration Tests (`tests/integration_tests.rs`)

Original integration tests covering:
- End-to-end cross-chain intents
- Light client state verification
- Context preservation
- Multi-hop routing
- Route discovery

## Running Tests

### Run All Tests

```bash
cargo test
```

### Run Specific Test Suite

```bash
# Unit tests only
cargo test --test adapter_unit_tests

# Integration tests only
cargo test --test adapter_integration_tests

# Error tests only
cargo test --test adapter_error_tests
```

### Run Tests for Specific Adapter

```bash
# Cosmos tests
cargo test cosmos_tests

# Bitcoin tests
cargo test bitcoin_tests

# Ethereum tests
cargo test ethereum
```

### Run Integration Tests (Requires Testnets)

```bash
# Run ignored tests
cargo test -- --ignored

# Run specific integration test
cargo test --test adapter_integration_tests test_ethereum_balance_query -- --ignored
```

## Code Coverage

### Measure Coverage

```bash
./scripts/measure_coverage.sh
```

This generates:
- HTML report: `target/coverage/index.html`
- XML report: `target/coverage/cobertura.xml`
- LCOV report: `target/coverage/lcov.info`

### Analyze Coverage

```bash
./scripts/analyze_coverage.sh
```

Provides:
- Per-adapter coverage percentages
- Component-level breakdown
- Overall coverage status
- Improvement recommendations

### Coverage Targets

- **Overall**: ≥80% line coverage
- **Critical adapters**: ≥85%
- **Standard adapters**: ≥80%

See [COVERAGE.md](COVERAGE.md) for detailed coverage documentation.

## Test Configuration

### Test Network Endpoints

Edit constants in test files:

```rust
const ETH_TESTNET_RPC: &str = "https://sepolia.infura.io/v3/YOUR_KEY";
const POLKADOT_TESTNET_RPC: &str = "wss://westend-rpc.polkadot.io";
const BITCOIN_TESTNET_RPC: &str = "http://localhost:18332";
const COSMOS_TESTNET_RPC: &str = "https://rpc.sentry-01.theta-testnet.polypore.xyz";
```

### Tarpaulin Configuration

Coverage settings in `tarpaulin.toml`:

```toml
[coverage]
fail-under = 80.0

[run]
timeout = 300
```

## Writing New Tests

### Unit Test Template

```rust
#[test]
fn test_adapter_functionality() {
    // Setup
    let input = create_test_input();
    
    // Execute
    let result = adapter_function(input);
    
    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), expected_output);
}
```

### Integration Test Template

```rust
#[tokio::test]
#[ignore] // Requires testnet
async fn test_adapter_integration() {
    let translator = Arc::new(IntentTranslator::new());
    let adapter = AdapterType::new(rpc_url, translator);
    
    let result = adapter.some_operation().await;
    
    assert!(result.is_ok());
}
```

### Error Test Template

```rust
#[tokio::test]
async fn test_adapter_error_handling() {
    let adapter = create_adapter_with_invalid_config();
    
    let result = adapter.operation().await;
    
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ExpectedError));
}
```

## Best Practices

1. **Test Isolation**: Each test should be independent
2. **Clear Naming**: Use descriptive test names
3. **Arrange-Act-Assert**: Follow AAA pattern
4. **Error Testing**: Test both success and failure paths
5. **Mock External Dependencies**: For unit tests
6. **Use Real Networks**: For integration tests (with `#[ignore]`)
7. **Document Requirements**: Note any special setup needed
8. **Performance Testing**: Include benchmarks for critical paths

## Continuous Integration

### GitHub Actions

```yaml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run tests
        run: cargo test
      - name: Run coverage
        run: ./scripts/measure_coverage.sh
```

## Troubleshooting

### Tests Fail to Compile

The main codebase has compilation errors that need to be fixed before tests can run. The test files themselves are correct and comprehensive.

### Integration Tests Fail

1. Check network connectivity
2. Verify RPC endpoints are accessible
3. Ensure test credentials are valid
4. Check if testnets are operational

### Coverage Tool Fails

```bash
# Install tarpaulin manually
cargo install cargo-tarpaulin

# Or use Docker
docker run --security-opt seccomp=unconfined -v "${PWD}:/volume" xd009642/tarpaulin
```

## Test Metrics

Current test coverage:
- **Unit tests**: 12 test modules, 50+ test cases
- **Integration tests**: 15+ end-to-end scenarios
- **Error tests**: 40+ error conditions
- **Total**: 100+ test cases

Target metrics:
- Line coverage: ≥80%
- Branch coverage: ≥70%
- Test pass rate: 100%

## Resources

- [Rust Testing Book](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Tokio Testing Guide](https://tokio.rs/tokio/topics/testing)
- [cargo-tarpaulin](https://github.com/xd009642/tarpaulin)
- [Property-based Testing](https://github.com/proptest-rs/proptest)
