# CRITICAL: Cargo Path Fix

## Issue
Direct `cargo` commands fail with "cargo: command not found" in this environment.

## Solution  
**Always use `./scripts/cargo.sh` instead of direct cargo commands.**

## For AI Assistants (Kilocode)
When you need to run cargo commands, automatically use the wrapper script:

```bash
# Instead of: cargo --version
./scripts/cargo.sh --version

# Instead of: cargo build
./scripts/cargo.sh build

# Instead of: cargo test
./scripts/cargo.sh test

# Instead of: cargo check
./scripts/cargo.sh check
```

## Auto-Detection
If you encounter `cargo: command not found`, immediately try the wrapper script:
1. Check if `./scripts/cargo.sh` exists
2. Use it instead of direct cargo commands
3. All cargo functionality is available through this script

## Why This Exists
The system PATH doesn't include cargo by default. The wrapper script at `./scripts/cargo.sh` automatically configures the correct PATH and passes through all arguments.

**Location**: `/Users/michaelhobbs/RustroverProjects/untitled/scripts/cargo.sh`
