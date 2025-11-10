# Code Coverage Guide

This document explains how to measure and analyze code coverage for the Sentium Bridge adapters.

## Requirements

- Rust toolchain (stable)
- cargo-tarpaulin (installed automatically by scripts)

## Quick Start

### Measure Coverage

Run the coverage measurement script:

```bash
./scripts/measure_coverage.sh
```

This will:
1. Install cargo-tarpaulin if needed
2. Run all tests with coverage tracking
3. Generate HTML and XML reports
4. Check if coverage meets the 80% threshold

### Analyze Coverage

For detailed per-adapter analysis:

```bash
./scripts/analyze_coverage.sh
```

This provides:
- Coverage percentage for each adapter
- Component-level coverage breakdown
- Overall coverage status
- Recommendations for improvement

## Coverage Reports

After running the measurement script, reports are available at:

- **HTML Report**: `target/coverage/index.html` (open in browser)
- **XML Report**: `target/coverage/cobertura.xml` (for CI/CD)
- **LCOV Report**: `target/coverage/lcov.info` (for IDE integration)

## Coverage Thresholds

### Overall Target
- **Minimum**: 80% line coverage
- **Goal**: 90%+ line coverage

### Per-Adapter Targets
Each adapter should maintain:
- **Critical adapters** (Ethereum, Bitcoin, Polkadot, Cosmos): ≥85%
- **Standard adapters**: ≥80%
- **Experimental adapters**: ≥70%

## Understanding Coverage

### What is Measured
- **Line Coverage**: Percentage of code lines executed during tests
- **Branch Coverage**: Percentage of conditional branches tested
- **Function Coverage**: Percentage of functions called

### What is Excluded
- Test files (`tests/*`)
- Benchmark files (`benches/*`)
- Example files (`examples/*`)
- Generated code

## Improving Coverage

### 1. Identify Uncovered Code

Open the HTML report and look for:
- Red lines (not executed)
- Yellow lines (partially executed)
- Uncovered branches

### 2. Add Tests

Focus on:
- Error handling paths
- Edge cases
- Conditional logic
- Integration points

### 3. Run Specific Tests

Test a specific adapter:

```bash
cargo test --test adapter_unit_tests cosmos_tests
```

Test with coverage:

```bash
cargo tarpaulin --test adapter_unit_tests
```

## CI/CD Integration

### GitHub Actions

```yaml
- name: Run Coverage
  run: ./scripts/measure_coverage.sh

- name: Upload Coverage
  uses: codecov/codecov-action@v3
  with:
    files: target/coverage/cobertura.xml
```

### GitLab CI

```yaml
coverage:
  script:
    - ./scripts/measure_coverage.sh
  coverage: '/Overall Coverage: (\d+\.\d+)%/'
  artifacts:
    reports:
      coverage_report:
        coverage_format: cobertura
        path: target/coverage/cobertura.xml
```

## Troubleshooting

### Tarpaulin Installation Fails

Install manually:
```bash
cargo install cargo-tarpaulin
```

### Tests Timeout

Increase timeout in `tarpaulin.toml`:
```toml
[run]
timeout = 600  # 10 minutes
```

### Low Coverage

1. Check which files are uncovered:
   ```bash
   ./scripts/analyze_coverage.sh
   ```

2. Add unit tests for uncovered adapters

3. Add integration tests for end-to-end flows

4. Test error cases and edge conditions

## Best Practices

1. **Write tests first**: Follow TDD for new features
2. **Test error paths**: Don't just test happy paths
3. **Use property-based testing**: For complex logic
4. **Mock external dependencies**: For unit tests
5. **Use real networks**: For integration tests (with `#[ignore]`)

## Coverage Goals by Phase

### Phase 1: Foundation (Current)
- Overall: ≥80%
- Critical adapters: ≥85%

### Phase 2: Production Ready
- Overall: ≥85%
- All adapters: ≥80%

### Phase 3: Mature
- Overall: ≥90%
- All adapters: ≥85%
- Branch coverage: ≥80%

## Resources

- [cargo-tarpaulin documentation](https://github.com/xd009642/tarpaulin)
- [Rust testing guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Code coverage best practices](https://martinfowler.com/bliki/TestCoverage.html)
