---
name: verify-commit
description: Run the full verification pipeline and fix any issues found. Keeps running until everything passes.
user-invocable: true
allowed-tools: "Bash(cargo *) Bash(./scripts/*) Bash(git status*) Bash(git diff*) Bash(git log*) Read Edit Grep Glob"
---

# Verify and Fix Until Clean

You are the verification agent. Your job is to run the full verification pipeline and fix every issue until it passes cleanly. You do NOT stop until the pipeline is green.

## Process

1. Run `./scripts/verify_agent_work.sh` from the project root
2. If it passes, report success and stop
3. If it fails, analyze the output to identify each failure:
   - **Format failures**: Run `cargo fmt` to fix
   - **Compilation errors**: Read the error, fix the source code
   - **Clippy warnings**: Fix the lint issue in the source
   - **Test failures**: Read the failing test, understand what it expects, fix the code or test
   - **Architecture violations**: Check the module's CLAUDE.md for allowed imports, restructure the code
   - **Test quality issues**: Replace trivial assertions with meaningful ones
4. After fixing, run the pipeline again from step 1
5. Repeat until ALL checks pass

## Rules
- Do NOT skip or disable any check
- Do NOT add `#[allow(...)]` attributes to suppress warnings
- Do NOT modify `verify_agent_work.sh` or other verification scripts
- Do NOT add exceptions to `.governance/architecture_exceptions.txt` without explaining why
- Fix the root cause, not the symptom
- Maximum 5 iterations. If still failing after 5, report what remains broken and stop
