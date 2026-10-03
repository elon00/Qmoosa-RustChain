#!/usr/bin/env bash
set -euo pipefail

echo "== Qmoosa-RustChain PolkaVM toolchain setup =="

if ! command -v rustup >/dev/null 2>&1; then
  echo "ERROR: rustup is required. Install Rust from https://rustup.rs/" >&2
  exit 1
fi

rustup toolchain install nightly --component rust-src --profile minimal

if ! command -v cargo-pvm-contract >/dev/null 2>&1 && ! cargo pvm-contract --help >/dev/null 2>&1; then
  cargo install cargo-pvm-contract --locked
fi

echo
echo "Toolchain verification:"
rustc +nightly --version
cargo +nightly --version
cargo pvm-contract --help >/dev/null

echo "OK: cargo-pvm-contract is installed and nightly rust-src is available."
