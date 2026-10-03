#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "Usage: $0 <path-to-pvm-contract>" >&2
  exit 2
fi

CONTRACT_DIR="$1"

if [ ! -f "$CONTRACT_DIR/Cargo.toml" ]; then
  echo "ERROR: no Cargo.toml found in $CONTRACT_DIR" >&2
  exit 1
fi

if ! cargo pvm-contract --help >/dev/null 2>&1; then
  echo "ERROR: cargo-pvm-contract is not installed." >&2
  echo "Run scripts/polkavm/setup-toolchain.sh first." >&2
  exit 1
fi

echo "Building PolkaVM contract in $CONTRACT_DIR"
(
  cd "$CONTRACT_DIR"
  cargo +nightly pvm-contract build
)

echo "Build complete. Expected outputs are under target/<profile>/*.polkavm and *.abi.json."
