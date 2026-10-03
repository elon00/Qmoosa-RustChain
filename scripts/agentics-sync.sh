#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== Qmoosa-RustChain AI Agentics Single-Click Sync =="

command -v cargo >/dev/null
command -v curl >/dev/null
command -v tar >/dev/null

CACHE_ROOT="$ROOT/.agent-cache"
CACHE="$CACHE_ROOT/product-sdk"
ARCHIVE="$CACHE_ROOT/product-sdk-main.tar.gz"
mkdir -p "$CACHE_ROOT"

echo "[1/6] Synchronizing official paritytech/product-sdk skills from GitHub tarball..."
rm -rf "$CACHE" "$ARCHIVE"
mkdir -p "$CACHE"
curl --fail --silent --show-error --location --retry 3 \
  https://github.com/paritytech/product-sdk/archive/refs/heads/main.tar.gz \
  --output "$ARCHIVE"
tar -xzf "$ARCHIVE" --strip-components=1 -C "$CACHE"
rm -f "$ARCHIVE"

test -d "$CACHE/skills"
echo "      Product SDK skills synchronized."

echo "[2/6] Verifying required AI resource manifest..."
test -f agentics/polkadot-ai-resources.json
grep -q "docs.polkadot.com/llms.txt" agentics/polkadot-ai-resources.json
grep -q "paritytech/product-sdk" agentics/polkadot-ai-resources.json

echo "[3/6] Verifying runtime agent-tool registry..."
for tool in   polkadot_get_balance   polkadot_transfer_native   polkadot_transfer_asset   polkadot_xcm_transfer   polkadot_ai_resources   polkadot_host_rules
do
  grep -q ""$tool"" crates/agent-core/src/lib.rs || {
    echo "ERROR: missing required agent tool: $tool" >&2
    exit 1
  }
done

if grep -q 'free_balance_plancks.*10_000_000_000' crates/agent-core/src/lib.rs; then
  echo "ERROR: fabricated balance placeholder detected." >&2
  exit 1
fi

echo "[4/6] Running formatting + strict lint..."
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

echo "[5/6] Running agent/MCP + workspace tests..."
cargo test -p qmoosa-agent-core
cargo test --workspace

echo "[6/6] Verifying API exposes MCP/tool endpoints..."
grep -q 'route("/mcp"' services/api/src/main.rs
grep -q 'route("/api/v1/agent/tools"' services/api/src/main.rs

echo
echo "AI AGENTICS INTEGRATION MISSION: SUCCESS"
echo "Official resources synchronized, toolkit registry verified, MCP/API wiring verified, and quality gates passed."
