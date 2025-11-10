#!/bin/bash
# Detailed coverage analysis script
# Analyzes coverage per adapter and identifies uncovered code

set -e

echo "==================================="
echo "Detailed Coverage Analysis"
echo "==================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if coverage report exists
if [ ! -f "target/coverage/cobertura.xml" ]; then
    echo "Error: Coverage report not found. Run measure_coverage.sh first."
    exit 1
fi

echo "Analyzing coverage by adapter..."
echo ""

# Function to extract coverage for a specific file pattern
get_file_coverage() {
    local pattern=$1
    local name=$2
    
    # Extract coverage for files matching pattern
    local coverage=$(grep -A 5 "filename=\".*${pattern}.*\"" target/coverage/cobertura.xml | \
                    grep -oP 'line-rate="\K[0-9.]+' | \
                    head -1)
    
    if [ -n "$coverage" ]; then
        local percent=$(echo "$coverage * 100" | bc)
        
        if (( $(echo "$percent >= 80" | bc -l) )); then
            echo -e "${GREEN}✓${NC} $name: ${percent}%"
        elif (( $(echo "$percent >= 60" | bc -l) )); then
            echo -e "${YELLOW}⚠${NC} $name: ${percent}%"
        else
            echo -e "${RED}✗${NC} $name: ${percent}%"
        fi
    else
        echo -e "${YELLOW}?${NC} $name: No data"
    fi
}

# Analyze each adapter
echo "Adapter Coverage:"
echo "-----------------"
get_file_coverage "ethereum" "Ethereum Adapter"
get_file_coverage "polkadot" "Polkadot Adapter"
get_file_coverage "bitcoin" "Bitcoin Adapter"
get_file_coverage "cosmos" "Cosmos Adapter"
get_file_coverage "solana" "Solana Adapter"
get_file_coverage "zcash" "Zcash Adapter"
get_file_coverage "harmony" "Harmony Adapter"
get_file_coverage "dash" "Dash Adapter"
get_file_coverage "tron" "TRON Adapter"
get_file_coverage "litecoin" "Litecoin Adapter"
get_file_coverage "dogecoin" "Dogecoin Adapter"
get_file_coverage "chia" "Chia Adapter"

echo ""
echo "Component Coverage:"
echo "-------------------"
get_file_coverage "intent_translator" "Intent Translator"
get_file_coverage "light-clients" "Light Clients"
get_file_coverage "router" "Router"

echo ""
echo "==================================="
echo "Coverage Summary"
echo "==================================="

# Extract overall coverage
OVERALL=$(grep -oP 'line-rate="\K[0-9.]+' target/coverage/cobertura.xml | head -1)
OVERALL_PERCENT=$(echo "$OVERALL * 100" | bc)

echo ""
echo "Overall Coverage: ${OVERALL_PERCENT}%"
echo "Target: 80%"

if (( $(echo "$OVERALL_PERCENT >= 80" | bc -l) )); then
    echo -e "${GREEN}Status: PASS${NC}"
    echo ""
    echo "Coverage meets the 80% threshold!"
else
    echo -e "${RED}Status: FAIL${NC}"
    echo ""
    echo "Coverage is below the 80% threshold."
    echo "Gap: $(echo "80 - $OVERALL_PERCENT" | bc)%"
    echo ""
    echo "Recommendations:"
    echo "1. Add tests for uncovered adapters"
    echo "2. Focus on adapters with <60% coverage"
    echo "3. Review uncovered lines in HTML report"
fi

echo ""
echo "Detailed report: target/coverage/index.html"
echo ""
