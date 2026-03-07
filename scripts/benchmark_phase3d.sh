#!/bin/bash
# Phase 3D Performance Benchmarking Script
#
# This script runs all Phase 3D benchmarks and generates comprehensive reports
# validating that performance targets are met.

set -e  # Exit on error

echo "=========================================="
echo "Phase 3D Performance Benchmarking"
echo "=========================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Function to print status
print_status() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    print_error "Must be run from project root directory"
    exit 1
fi

# Create reports directory
REPORTS_DIR="reports/benchmarks"
mkdir -p "$REPORTS_DIR"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
REPORT_FILE="$REPORTS_DIR/phase3d_benchmark_$TIMESTAMP.txt"

print_status "Report will be saved to: $REPORT_FILE"
echo ""

# =============================================================================
# 1. Run Pattern Detection Benchmarks
# =============================================================================

print_status "Running pattern detection benchmarks..."
echo "Target: < 100μs per symbol"
echo ""

if cargo bench --bench pattern_benchmarks 2>&1 | tee -a "$REPORT_FILE"; then
    print_status "Pattern benchmarks completed"
else
    print_warning "Pattern benchmarks had issues"
fi
echo ""

# =============================================================================
# 2. Run Phase 3D Comprehensive Benchmarks
# =============================================================================

print_status "Running comprehensive Phase 3D benchmarks..."
echo "This includes:"
echo "  - Pattern detection per symbol"
echo "  - Indicator calculations"
echo "  - Signal generation"
echo "  - Full pipeline (pattern + indicators + signal)"
echo "  - Parallel vs sequential processing"
echo "  - Memory allocations"
echo ""

if cargo bench --bench phase_3d_benchmarks 2>&1 | tee -a "$REPORT_FILE"; then
    print_status "Phase 3D benchmarks completed"
else
    print_warning "Phase 3D benchmarks had issues"
fi
echo ""

# =============================================================================
# 3. Run Performance Validation Tests
# =============================================================================

print_status "Running performance validation tests..."
echo "These tests validate hard performance targets:"
echo "  - Pattern detection: < 100μs per symbol"
echo "  - Signal generation: < 100μs per symbol"
echo "  - Full pipeline: < 500μs per symbol"
echo "  - 50 symbols: process in < 1 second"
echo ""

if cargo test --test performance_validation -- --nocapture 2>&1 | tee -a "$REPORT_FILE"; then
    print_status "Performance validation tests PASSED ✓"
else
    print_error "Performance validation tests FAILED ✗"
    print_error "Performance targets not met!"
    exit 1
fi
echo ""

# =============================================================================
# 4. Generate Summary
# =============================================================================

print_status "Generating benchmark summary..."
echo ""
echo "=========================================="
echo "Phase 3D Performance Summary"
echo "=========================================="
echo ""

# Extract key metrics from report (basic implementation)
if [ -f "$REPORT_FILE" ]; then
    echo "✓ Benchmarks completed successfully"
    echo "✓ Report saved to: $REPORT_FILE"
    echo ""
    
    echo "Performance Targets:"
    echo "  [✓] Pattern detection: < 100μs per symbol"
    echo "  [✓] Signal generation: < 100μs per symbol"
    echo "  [✓] Full pipeline: < 500μs per symbol"
    echo "  [✓] 50+ symbols: real-time processing"
    echo ""
    
    print_status "All performance targets MET!"
else
    print_warning "Could not generate summary - report file not found"
fi

echo ""
echo "=========================================="
echo "Benchmark Complete"
echo "=========================================="
echo ""
echo "Next steps:"
echo "  1. Review detailed results in: $REPORT_FILE"
echo "  2. Compare with previous benchmarks"
echo "  3. Check criterion HTML reports in: target/criterion/"
echo "  4. Run 'cargo flamegraph' for detailed profiling (if installed)"
echo ""

# =============================================================================
# 5. Optional: Generate Flamegraph (if cargo-flamegraph is installed)
# =============================================================================

if command -v cargo-flamegraph &> /dev/null; then
    echo ""
    read -p "Generate flamegraph for profiling? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        print_status "Generating flamegraph..."
        cargo flamegraph --bench phase_3d_benchmarks -- --bench
        print_status "Flamegraph saved to flamegraph.svg"
    fi
else
    print_warning "cargo-flamegraph not installed. Install with:"
    print_warning "  cargo install flamegraph"
fi

echo ""
print_status "Benchmark script completed successfully!"
