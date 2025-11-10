#!/bin/bash
# Script to measure code coverage for Sentium Bridge adapters
# Uses cargo-tarpaulin for coverage analysis

set -e

echo "==================================="
echo "Sentium Bridge Coverage Measurement"
echo "==================================="
echo ""

# Check if cargo-tarpaulin is installed
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo "cargo-tarpaulin not found. Installing..."
    cargo install cargo-tarpaulin
fi

echo "Running coverage analysis..."
echo ""

# Run tarpaulin with HTML output
cargo tarpaulin \
    --out Html \
    --out Xml \
    --output-dir target/coverage \
    --exclude-files "tests/*" \
    --exclude-files "benches/*" \
    --exclude-files "examples/*" \
    --timeout 300 \
    --verbose

echo ""
echo "==================================="
echo "Coverage Report Generated"
echo "==================================="
echo ""
echo "HTML Report: target/coverage/index.html"
echo "XML Report: target/coverage/cobertura.xml"
echo ""

# Parse coverage percentage from output
if [ -f "target/coverage/cobertura.xml" ]; then
    echo "Checking coverage thresholds..."
    
    # Extract line coverage percentage
    COVERAGE=$(grep -oP 'line-rate="\K[0-9.]+' target/coverage/cobertura.xml | head -1)
    COVERAGE_PERCENT=$(echo "$COVERAGE * 100" | bc)
    
    echo "Overall Coverage: ${COVERAGE_PERCENT}%"
    
    # Check if coverage meets 80% threshold
    if (( $(echo "$COVERAGE_PERCENT >= 80" | bc -l) )); then
        echo "✓ Coverage meets 80% threshold"
        exit 0
    else
        echo "✗ Coverage below 80% threshold"
        echo "  Current: ${COVERAGE_PERCENT}%"
        echo "  Required: 80%"
        exit 1
    fi
else
    echo "Warning: Could not find coverage report"
    exit 1
fi
