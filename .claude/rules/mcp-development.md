---
paths:
  - "mcp-servers/**/*"
---

# MCP Server Development Rules

1. **Security first**: All MCP servers MUST validate and sanitize inputs.
2. **Path sandboxing**: Path-accepting tools MUST validate against allowed directories — no directory traversal.
3. **SQL read-only by default**: SQL-accepting tools MUST enforce read-only with explicit opt-in for writes.
4. **`#![forbid(unsafe_code)]`** required for all Rust MCP server implementations.
5. **JSON-RPC 2.0 over stdio** — this is the MCP transport protocol.
6. **Standalone projects**: Each server has its own `Cargo.toml` or `package.json`, separate from the workspace.
7. **README.md required**: Include tool descriptions, input schemas, and security model.
8. **All tools must have tests** — at minimum basic input/output validation.
9. **Error handling**: Use MCP `isError: true` format for errors — never crash the server process.
10. **No build artifacts committed**: Ensure `.gitignore` covers `target/`, `dist/`, `node_modules/`.
11. **Naming convention**: `{category}-{number}-{name}` (e.g., `dev-01-filesystem`).
