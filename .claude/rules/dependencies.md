---
paths:
  - "**/Cargo.toml"
---

# Dependency Management Rules

1. **Do NOT add new dependencies** without explicit justification.
2. **Pin exact versions** for direct dependencies (e.g., `serde = "1.0.210"` not `serde = "1"`).
3. **Use feature flags** to minimize compiled code (e.g., `tokio = { version = "...", features = ["rt-multi-thread", "macros"] }`).
4. **Run `cargo audit`** after adding any dependency.
5. **Prefer well-maintained crates** with >1000 downloads/month.
6. **Check existing deps first** before adding new ones — avoid duplicate functionality.
7. **`dev-dependencies`** can use less strict versioning.
8. **Document non-obvious dependencies** with a comment above the entry in Cargo.toml.
9. **MCP server crates** (`mcp-servers/`) are standalone — they have separate Cargo.toml files and do NOT share the workspace.
