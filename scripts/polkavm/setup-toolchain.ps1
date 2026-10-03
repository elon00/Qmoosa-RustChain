$ErrorActionPreference = "Stop"

Write-Host "== Qmoosa-RustChain PolkaVM toolchain setup =="

if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    throw "rustup is required. Install Rust from https://rustup.rs/"
}

rustup toolchain install nightly --component rust-src --profile minimal

$cargoPvm = Get-Command cargo-pvm-contract -ErrorAction SilentlyContinue
if (-not $cargoPvm) {
    try {
        cargo pvm-contract --help *> $null
    } catch {
        cargo install cargo-pvm-contract --locked
    }
}

Write-Host ""
Write-Host "Toolchain verification:"
rustc +nightly --version
cargo +nightly --version
cargo pvm-contract --help *> $null

Write-Host "OK: cargo-pvm-contract is installed and nightly rust-src is available."
