#!/usr/bin/env bash
# Wrapper script for MCP servers that need secrets from .env
# Sources .env from the project root, then execs the given command.
# Usage: mcp-env-wrapper.sh <command> [args...]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ENV_FILE="$PROJECT_ROOT/.env"

if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck source=/dev/null
  source "$ENV_FILE"
  set +a
fi

exec "$@"
