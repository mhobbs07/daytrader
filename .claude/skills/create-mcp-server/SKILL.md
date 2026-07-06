---
model: default
description: Scaffold a new MCP server following project conventions
---

# /create-mcp-server

Creates a new MCP server under `mcp-servers/` following project standards.

## Steps

1. Determine category and number from naming convention:
   - `dev-{NN}` — Development tools (filesystem, git, database)
   - `mkt-{NN}` — Market data (realtime, historical)
   - `com-{NN}` — Communications (alerting, notifications)
   Check existing servers: `!ls mcp-servers/`

2. Choose implementation language:
   - **Rust**: For performance-critical servers or those needing main project crate access
   - **TypeScript**: For I/O-heavy servers, rapid prototyping, or npm ecosystem needs

3. Create the server directory structure:
   - For Rust: `mcp-servers/{name}/Cargo.toml`, `src/main.rs`, `README.md`
   - For TypeScript: `mcp-servers/{name}/package.json`, `tsconfig.json`, `src/index.ts`, `README.md`

4. Apply mandatory security patterns:
   - `#![forbid(unsafe_code)]` for Rust
   - Path sandboxing for file-access tools (see `mcp-servers/dev-01-filesystem/src/index.ts`)
   - Read-only by default for database tools (see `mcp-servers/dev-05-database/src/main.rs`)
   - Input validation on all tool parameters

5. Implement JSON-RPC 2.0 over stdio with:
   - `initialize` → server info + capabilities
   - `tools/list` → tool definitions with inputSchema
   - `tools/call` → tool dispatch with error handling (never crash)

6. Add to `.claude/settings.json` mcpServers section (disabled by default)

7. Verify `.gitignore` covers `target/` and `node_modules/` for the new server

## Reference
- Rules: `.claude/rules/mcp-development.md`
- Example Rust server: `mcp-servers/dev-05-database/`
- Example TypeScript server: `mcp-servers/dev-01-filesystem/`
