$ErrorActionPreference = "Stop"

$Root = (Get-Item $PSScriptRoot).Parent.FullName
Set-Location $Root

Write-Host "== Qmoosa-RustChain AI Agentics Single-Click Sync ==" -ForegroundColor Cyan

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw "cargo is required" }
if (-not (Get-Command git -ErrorAction SilentlyContinue)) { throw "git is required" }

$CacheRoot = Join-Path $Root ".agent-cache"
$Cache = Join-Path $CacheRoot "product-sdk"
New-Item -ItemType Directory -Force -Path $CacheRoot | Out-Null

if (Test-Path (Join-Path $Cache ".git")) {
    Write-Host "[1/6] Refreshing official paritytech/product-sdk skills..."
    git -C $Cache fetch --depth 1 origin
    git -C $Cache reset --hard origin/HEAD
} else {
    Write-Host "[1/6] Cloning official paritytech/product-sdk skills..."
    git clone --depth 1 https://github.com/paritytech/product-sdk.git $Cache
}
if (-not (Test-Path (Join-Path $Cache "skills"))) { throw "Product SDK skills directory not found" }

Write-Host "[2/6] Verifying required AI resource manifest..."
$Manifest = Get-Content "agentics/polkadot-ai-resources.json" -Raw
if ($Manifest -notmatch "docs.polkadot.com/llms.txt") { throw "Missing Polkadot llms.txt resource" }
if ($Manifest -notmatch "paritytech/product-sdk") { throw "Missing Product SDK resource" }

Write-Host "[3/6] Verifying runtime agent-tool registry..."
$AgentCore = Get-Content "crates/agent-core/src/lib.rs" -Raw
$Tools = @(
    "polkadot_get_balance",
    "polkadot_transfer_native",
    "polkadot_transfer_asset",
    "polkadot_xcm_transfer",
    "polkadot_ai_resources",
    "polkadot_host_rules"
)
foreach ($Tool in $Tools) {
    if ($AgentCore -notmatch [regex]::Escape('"' + $Tool + '"')) {
        throw "Missing required agent tool: $Tool"
    }
}
if ($AgentCore -match "free_balance_plancks.*10_000_000_000") {
    throw "Fabricated balance placeholder detected"
}

Write-Host "[4/6] Running formatting + strict lint..."
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

Write-Host "[5/6] Running agent/MCP + workspace tests..."
cargo test -p qmoosa-agent-core
cargo test --workspace

Write-Host "[6/6] Verifying API exposes MCP/tool endpoints..."
$Api = Get-Content "services/api/src/main.rs" -Raw
if ($Api -notmatch 'route\("/mcp"') { throw "Missing /mcp route" }
if ($Api -notmatch 'route\("/api/v1/agent/tools"') { throw "Missing agent tools route" }

Write-Host ""
Write-Host "AI AGENTICS INTEGRATION MISSION: SUCCESS" -ForegroundColor Green
Write-Host "Official resources synchronized, toolkit registry verified, MCP/API wiring verified, and quality gates passed."
