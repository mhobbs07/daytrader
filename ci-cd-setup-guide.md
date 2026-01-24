# CI/CD Setup Guide for Documentation Generation

## Overview

This guide provides instructions for setting up automated documentation generation and deployment for the Day Trading Platform.

## GitHub Actions Workflow Setup

Create the following file: `.github/workflows/docs.yml`

```yaml
name: Generate Documentation

on:
  push:
    branches: [ main, master ]
  pull_request:
    branches: [ main, master ]

jobs:
  docs:
    runs-on: ubuntu-latest

    steps:
    - name: Checkout code
      uses: actions/checkout@v4

    - name: Install Rust
      uses: dtolnay/rust-toolchain@stable
      with:
        components: rust-docs

    - name: Cache dependencies
      uses: Swatinem/rust-cache@v2

    - name: Build documentation
      run: cargo doc --document-private-items --no-deps

    - name: Deploy to GitHub Pages
      if: github.ref == 'refs/heads/main' || github.ref == 'refs/heads/master'
      uses: peaceiris/actions-gh-pages@v3
      with:
        github_token: ${{ secrets.GITHUB_TOKEN }}
        publish_dir: ./target/doc
        cname: docs.daytrader.dev  # Optional: add custom domain
```

## Local Documentation Generation

To generate documentation locally:

```bash
# Generate docs for public APIs only
cargo doc

# Generate docs including private items
cargo doc --document-private-items

# Open docs in browser
cargo doc --open
```

## Documentation Standards

### Module Documentation Requirements

Every module must include:

```rust
//! # Module Name
//!
//! Brief description of the module's purpose and functionality.
//!
//! ## Overview
//!
//! Detailed explanation of the module's role in the system.
//!
//! ## Architecture
//!
//! How this module fits into the overall system architecture.
//!
//! ## Examples
//!
//! ```rust
//! // Usage examples
//! ```
//!
//! ## Design Decisions
//!
//! Rationale for key architectural choices made in this module.
```

### Function Documentation Requirements

All public functions must include:

```rust
/// Brief description of what the function does.
///
/// # Arguments
///
/// * `param1` - Description of parameter 1
/// * `param2` - Description of parameter 2
///
/// # Returns
///
/// Description of return value and possible error conditions.
///
/// # Examples
///
/// ```rust
/// // Usage example with expected output
/// ```
///
/// # Performance
///
/// Any performance characteristics or constraints.
///
/// # Panics
///
/// Conditions under which the function will panic (if any).
```

## Documentation Testing

### Adding Doc Tests

Include testable examples in documentation:

```rust
/// Calculates the total value of a position.
///
/// # Examples
///
/// ```rust
/// use daytrader::risk::types::DollarAmount;
/// use rust_decimal::Decimal;
///
/// let price = DollarAmount::new(Decimal::new(15000, 2)).unwrap();
/// let quantity = 100;
/// let total = price.value() * Decimal::new(quantity, 0);
/// assert_eq!(total, Decimal::new(1500000, 2)); // $15,000.00
/// ```
pub fn calculate_position_value(price: DollarAmount, quantity: u32) -> DollarAmount {
    // Implementation
}
```

### Running Doc Tests

```bash
# Run all doc tests
cargo test --doc

# Run doc tests for specific package
cargo test --doc --package daytrader
```

## Documentation Deployment

### GitHub Pages Setup

1. Enable GitHub Pages in repository settings
2. Set source to "GitHub Actions"
3. The workflow above will automatically deploy docs on main branch pushes

### Custom Domain (Optional)

1. Add `docs.daytrader.dev` to repository settings
2. Create CNAME record pointing to `username.github.io`
3. Update the workflow's `cname` field

## Quality Gates

### Documentation Coverage

- All public APIs must be documented
- Examples must compile and pass
- Cross-references must be valid
- No broken links in documentation

### CI/CD Checks

The following must pass in CI:

```yaml
- name: Check documentation
  run: |
    cargo doc --no-deps
    cargo test --doc
```

## Maintenance

### Regular Tasks

- Review documentation for accuracy after API changes
- Update examples when implementation changes
- Ensure cross-references remain valid
- Test documentation examples regularly

### Versioning

- Documentation is versioned with code
- Breaking changes require documentation updates
- Deprecation notices must be documented

## Troubleshooting

### Common Issues

1. **Missing dependencies in docs**
   - Use `--no-deps` flag to avoid building dependencies
   - Ensure all required features are enabled

2. **Broken doc tests**
   - Check that examples use correct imports
   - Ensure test environment has required setup
   - Verify examples match current API

3. **GitHub Pages deployment failures**
   - Check that workflow has proper permissions
   - Verify publish directory exists
   - Ensure GITHUB_TOKEN secret is available

## Integration with Development Workflow

### Pull Request Requirements

- Documentation must be updated for API changes
- New public APIs require documentation
- Doc tests must pass
- Cross-references must be valid

### Code Review Checklist

- [ ] All public functions have documentation
- [ ] Examples are provided and testable
- [ ] Cross-references are accurate
- [ ] Documentation builds without warnings
- [ ] Doc tests pass