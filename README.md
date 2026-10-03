# 🦀 Qmoosa-RustChain

A native, high-performance **Rust-first** Web4 platform designed for **Polkadot Hub / PolkaVM / Substrate**, featuring authentic NIST FIPS 204 Post-Quantum Cryptography (ML-DSA-65), on-chain verifiable x402 Bazaar Protocol, Substrate `AccountId32` primitives, and Conway Automaton Agentics.

---

## 🏗️ Architectural Philosophy & Security Posture

### 1. Cryptographic Authenticity (NIST FIPS 204 ML-DSA-65)
Unlike pseudo-PQC wrappers, **Qmoosa-RustChain** implements authentic, lattice-based digital signatures compliant with **NIST FIPS 204 (ML-DSA-65)** via `fips204`:
- **Key Sizes**: 1,952-byte Public Key, 4,032-byte Secret Key.
- **Signature Size**: 3,309-byte lattice signature.
- **Attestation Pipeline**: Envelopes bind SHA3-256 payload digest, timestamps, expiration windows, and unique nonces with full replay attack protection.

### 2. Polkadot Native Primitives (`AccountId32` & SS58)
All accounts across crates and contracts use Substrate-native 32-byte public keys (`AccountId32`) with:
- **SS58 Base-58 Checksum Encoding/Decoding**: Validated via Blake2b-512 checksums against Substrate (`prefix 42`) and Polkadot (`prefix 0`) formats.
- **Parity SCALE Codec**: Complete serialization roundtrip for all smart contract message selectors and event emissions.

### 3. Smart Contract Architecture (PolkaVM Ready)
Contracts in `contracts/` are structured for Polkadot's next-generation **PolkaVM (`pallet-revive`)** execution engine:
- Dedicated SCALE-encoded ABI messages (`QdotMessage`, `LaunchpadMessage`, `X402SettlementMessage`).
- Strongly typed SCALE-encoded on-chain events emitted to the host runtime.
- Invariant state guards for double-claim prevention, vesting lockups, and fee routing.

> [!NOTE]
> **Security Clarification**: Rust guarantees memory safety, absence of null-pointer dereferences, and data-race freedom at compile time. However, smart contract logical risks (such as cross-contract reentrancy flows, economic exploits, and authorization flaws) are defended through explicit state-ordering checks, checks-effects-interactions patterns, and anti-replay registries.

### 4. x402 Bazaar Protocol (On-Chain Subxt Settlement Verification)
The HTTP 402 gateway enforces an end-to-end on-chain verification pipeline using `subxt v0.51`:
```text
HTTP Request
  └─► 402 Payment Required (Challenge ID + SS58 Merchant + Amount)
        └─► On-Chain Polkadot Transaction
              └─► Subxt Live WebSocket RPC Inspection
                    └─► Finalized Block Check -> SCALE Event Decoders
                          ├── Balances::Transfer (Native DOT)
                          └── pallet_assets::Transferred (Asset Hub Fungible Assets)
                                └─► Recipient & Amount Matching
                                      └─► Anti-Replay Cache Verification
                                            └─► Resource Unlocked
```

### 5. Polkadot Agent Kit & Model Context Protocol (MCP)
`crates/agent-core` implements an autonomous **Polkadot Agent Kit** action registry exposed directly over the standard **Model Context Protocol (MCP)**:
- **Zero Private Key Exposure**: Agents only formulate structured proposals and action descriptions; signing authority remains strictly with the user's host wallet.
- **Agent Kit Tool Suite**:
  - `polkadot_transfer_native`: Native DOT token transfer proposals.
  - `polkadot_transfer_asset`: Asset Hub fungible asset (`pallet_assets`) transfers.
  - `polkadot_xcm_transfer`: Cross-consensus messaging (XCM) transfers across parachains.
  - `polkadot_query_balance`: Balance lookup for Substrate `AccountId32` accounts.
  - `polkadot_sign_pqc_attestation`: NIST FIPS 204 ML-DSA-65 post-quantum signing envelope.
- **Standard MCP Protocol**: Full JSON-RPC 2.0 interface supporting `initialize`, `tools/list`, and `tools/call` for direct integration with AI assistants (Claude, Cursor, Antigravity).

---

## 📦 Workspace Layout

```text
Qmoosa-RustChain/
├── Cargo.toml                  # Workspace Manifest (Resolver 2)
├── AGENTS.md                   # AI Agent Protocol & Host-Mediated Security Guide
├── README.md                   # Technical Documentation & Architecture Blueprint
├── .github/workflows/rust.yml  # Automated CI/CD (fmt, clippy -D warnings, tests, release build)
│
├── crates/
│   ├── primitives/             # AccountId32, Substrate SS58 Blake2b Checksum, SCALE Codec
│   ├── pqc/                    # Genuine NIST FIPS 204 ML-DSA-65 Post-Quantum Attestation
│   ├── x402/                   # HTTP 402 Bazaar Protocol Gateway & Subxt v0.51 On-Chain Verifier
│   ├── conway/                 # B3/S23 Conway Automaton Event-Driven Agent Trigger Engine
│   └── agent-core/             # Polkadot Agent Kit & Model Context Protocol (MCP) Host Engine
│
├── contracts/
│   ├── qdot-token/             # Flexible / Uncapped Supply Token with Role-Governed Mint & PolkaVM ABI
│   ├── launchpad/              # Fair Launch Presale, Vesting Locks & 2.5% Treasury Fee Cut
│   └── x402-settlement/        # On-Chain HTTP 402 Micro-Settlement Registry (1% Network Split)
│
└── services/
    └── api/                    # Axum/Tokio Microservice with x402 Gateway & MCP Endpoint
```

---

## 🧪 Testing & Verification

Run the complete test suite across all 9 workspace crates and contracts:

```bash
# 1. Format check
cargo fmt --all -- --check

# 2. Strict static analysis (zero warnings allowed)
cargo clippy --workspace --all-targets -- -D warnings

# 3. Workspace unit and integration tests (45 tests)
cargo test --workspace
```

### Test Suite Summary

| Component | Module | Passed Tests | Description |
|---|---|:---:|---|
| `qmoosa-primitives` | SS58 & SCALE Codec | 4 | Substrate prefix, Blake2b checksum, user wallet decoding, SCALE roundtrip |
| `qmoosa-pqc` | NIST FIPS 204 ML-DSA-65 | 6 | 3309-byte lattice signatures, tampered payload, wrong pubkey, corrupted sig, replay, expiration |
| `qmoosa-qdot-token` | PolkaVM QDOT Token | 5 | Mint/Burn, Pause, Access Control, PolkaVM message dispatch & events |
| `qmoosa-launchpad` | Presale & Vesting | 5 | Token purchase, time-locked claim, fee routing, PolkaVM message dispatch |
| `qmoosa-x402` | Bazaar Gateway & Subxt Verifier | 11 | Subxt event decoders (Balances::Transfer, Assets::Transferred), finality, recipient/amount check, anti-replay |
| `qmoosa-x402-settlement` | On-Chain Settlement Registry | 4 | Micro-settlement, fee splits, replay protection, PolkaVM message dispatch |
| `qmoosa-agent-core` | Polkadot Agent Kit & MCP | 8 | Intent routing, Agent Kit tool registry, native/asset/XCM tool calls, MCP JSON-RPC protocol |
| `qmoosa-conway` | Cellular Automaton Triggers | 2 | Glider simulation, epoch milestone event dispatch |
| **Total** | **Full Workspace** | **45** | **100% Passed (0 Failures)** |

---

## 🚀 Running the API Service

```bash
cargo run -p qmoosa-api
# Service runs on http://0.0.0.0:8080
```

### Endpoints
- `GET /health`: Platform health, PQC status, Subxt connection, and network parameters.
- `GET /api/v1/alpha-model`: HTTP 402 protected resource (requires payment proof).
- `POST /api/pqc/sign`: Generates genuine NIST FIPS 204 ML-DSA-65 envelope.
- `POST /api/pqc/verify`: Verifies ML-DSA-65 envelope against payload.
- `GET /api/v1/agent/tools`: Lists all registered Polkadot Agent Kit tools.
- `POST /mcp`: Standard Model Context Protocol (MCP) JSON-RPC 2.0 endpoint (`initialize`, `tools/list`, `tools/call`).

---

## 🗺️ Engineering Roadmap

```text
Phase 1: Rust Foundation & Primitives (Completed)
  ├── Authentic NIST FIPS 204 ML-DSA-65 (fips204)
  ├── Substrate AccountId32 & Blake2b SS58 Checksum Codec
  ├── On-Chain x402 RPC Verifier Pipeline
  └── Strict CI: fmt, clippy (-D warnings), tests

Phase 2: PolkaVM Deployment Artifacts (Next)
  ├── Target RISC-V compilation via pallet-revive tooling
  ├── Deploy to Polkadot Hub Westend / Asset Hub TestNet
  └── Verify on-chain contract code & gas efficiency

Phase 3: Production Mainnet & Multi-Agent Swarm
  ├── SubWallet & Talisman Native Substrate Signing UI
  └── Conway Event-Driven Autonomous Execution Swarm
```
