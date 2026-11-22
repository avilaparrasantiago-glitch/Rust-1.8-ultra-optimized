#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
[[ "$(uname -s)" == Linux ]]
for tool in rustup cargo cc c++ pkg-config git uv uvx codex; do
  command -v "$tool" >/dev/null || { echo "Falta $tool en PATH" >&2; exit 1; }
done
rustup component add rust-analyzer clippy rustfmt
cargo install --locked cargo-nextest
cargo install --locked flamegraph
mkdir -p .codex
if [[ -e .codex/config.toml ]]; then
  cmp "FONTS EXTRAS/tools/codex-linux.toml" .codex/config.toml || { echo "Revisa la configuración MCP existente" >&2; exit 1; }
else
  cp "FONTS EXTRAS/tools/codex-linux.toml" .codex/config.toml
fi
uv run --script "FONTS EXTRAS/tools/rust_mcp.py" </dev/null
uvx mcp-server-git --help >/dev/null
codex mcp list
cargo nextest --version
if ! command -v perf >/dev/null; then echo "Instala perf para el kernel de este host antes de perfilar." >&2; fi
