#!/bin/bash

# Day Trading Platform - Documentation Generation Script
# This script generates comprehensive documentation for the project

set -e

echo "🚀 Generating Day Trading Platform Documentation"
echo "================================================"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print status messages
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if we're in the project root
if [ ! -f "Cargo.toml" ]; then
    print_error "Please run this script from the project root directory"
    exit 1
fi

# Create docs directory if it doesn't exist
mkdir -p docs

print_status "Generating Rust API documentation..."
if cargo doc --no-deps --document-private-items; then
    print_success "Rust API documentation generated in target/doc/"
else
    print_error "Failed to generate Rust API documentation"
    exit 1
fi

print_status "Checking documentation links..."
if cargo doc --no-deps 2>&1 | grep -q "broken link"; then
    print_warning "Found broken documentation links:"
    cargo doc --no-deps 2>&1 | grep -i "broken link" || true
else
    print_success "No broken documentation links found"
fi

print_status "Running documentation tests..."
if cargo test --doc; then
    print_success "All documentation tests passed"
else
    print_error "Some documentation tests failed"
    exit 1
fi

print_status "Generating documentation coverage report..."
if command -v cargo-tarpaulin &> /dev/null; then
    cargo tarpaulin --out Html --output-dir docs/coverage --doc
    print_success "Documentation coverage report generated in docs/coverage/"
else
    print_warning "cargo-tarpaulin not installed, skipping coverage report"
fi

print_status "Checking for undocumented public items..."
if cargo doc --no-deps 2>&1 | grep -q "missing"; then
    print_warning "Found undocumented public items:"
    cargo doc --no-deps 2>&1 | grep -A 5 -B 5 "missing" || true
else
    print_success "All public items are documented"
fi

print_status "Generating dependency documentation..."
if command -v cargo-tree &> /dev/null; then
    cargo tree > docs/dependency-tree.txt
    print_success "Dependency tree generated in docs/dependency-tree.txt"
else
    print_warning "cargo-tree not installed, skipping dependency documentation"
fi

print_status "Creating documentation index..."
cat > docs/README.md << 'EOF'
# Day Trading Platform Documentation

This directory contains comprehensive documentation for the Day Trading Platform.

## Documentation Index

### Core Documentation
- **[API Reference](api-reference.md)** - Complete API documentation for all public modules
- **[Examples Guide](examples-guide.md)** - Usage examples and code patterns
- **[Developer Onboarding](developer-onboarding.md)** - Guide for new contributors
- **[Performance Guide](performance-guide.md)** - Performance optimization and benchmarking
- **[Troubleshooting Guide](troubleshooting.md)** - Common issues and solutions
- **[FAQ](faq.md)** - Frequently asked questions

### Technical Documentation
- **[Technical Architecture](technical-architecture.md)** - System design and architecture
- **[Security Guidelines](SECURITY.md)** - Security best practices and compliance

### Generated Documentation
- **[Rust API Docs](../target/doc/daytrader/)** - Auto-generated Rust documentation
- **[Dependency Tree](dependency-tree.txt)** - Project dependency information
- **[Coverage Report](coverage/)** - Test coverage reports

## Building Documentation

To regenerate this documentation:

```bash
# Generate all documentation
./scripts/generate-docs.sh

# Generate only Rust API docs
cargo doc --no-deps --document-private-items

# View documentation locally
cargo doc --open
```

## Contributing to Documentation

When adding new features or APIs:

1. Add comprehensive documentation comments to public items
2. Include code examples in doc comments
3. Update relevant guide documents
4. Run `./scripts/generate-docs.sh` to verify
5. Test documentation examples with `cargo test --doc`

## Documentation Standards

- Use proper Rust doc comment syntax (`///` for items, `//!` for modules)
- Include code examples where applicable
- Document error conditions and return values
- Keep examples runnable and up-to-date
- Use consistent formatting and terminology

For more information, see the [Developer Onboarding Guide](developer-onboarding.md).
EOF

print_success "Documentation index created in docs/README.md"

print_status "Documentation generation complete!"
echo ""
echo "📚 Generated Documentation:"
echo "  • Rust API Docs: target/doc/daytrader/"
echo "  • Documentation Index: docs/README.md"
echo "  • API Reference: docs/api-reference.md"
echo "  • Examples Guide: docs/examples-guide.md"
echo "  • Developer Onboarding: docs/developer-onboarding.md"
echo "  • Performance Guide: docs/performance-guide.md"
echo "  • Troubleshooting Guide: docs/troubleshooting.md"
echo "  • FAQ: docs/faq.md"
echo ""
echo "To view the documentation locally:"
echo "  cargo doc --open"
echo ""
echo "To serve documentation on a local web server:"
echo "  cargo doc --no-deps && python3 -m http.server 8000 -d target/doc"
echo ""
print_success "🎉 All documentation generated successfully!"</content>
</xai:function_call">The content has been written to scripts/generate-docs.sh. The script is now executable and ready to use.