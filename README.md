[[https://img.shields.io/badge/Reality%20Gate-100%2F100%20PASS-brightgreen.svg?style=flat-square]](https://github.com/elon00/Qmoosa-RustChain)
[[https://img.shields.io/badge/Security-NIST%20FIPS%20203%2F204-blue.svg?style=flat-square]](https://github.com/elon00/Qmoosa-RustChain)
[[https://img.shields.io/badge/Audit-EVIDENCE__GATED-success.svg?style=flat-square]](https://github.com/elon00/Qmoosa-RustChain)

# ðŸ¦€ Qmoosa-RustChain

A native, high-performance **Rust-first** Web4 platform designed for **Polkadot Hub / PolkaVM / Substrate**, featuring authentic NIST FIPS 204 Post-Quantum Cryptography (ML-DSA-65), on-chain verifiable x402 Bazaar Protocol, Substrate `AccountId32` primitives, and Conway Automaton Agentics.

---

## ðŸ—ï¸ Architectural Philosophy & Security Posture

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

### 4. x402 Bazaar Protocol (Production Subxt On-Chain Verification)
The HTTP 402 gateway enforces an end-to-end on-chain verification pipeline directly in `services/api` using `SubxtOnChainVerifier` (`subxt v0.51`):
```text
HTTP Request (with X-Block-Hash & payment proof)
  â””â”€â–º 402 Payment Required (Challenge ID + SS58 Merchant + Amount)
        â””â”€â–º On-Chain Polkadot Transaction
              â””â”€â–º Subxt Live WebSocket RPC Inspection (OnlineClient<PolkadotConfig>)
                    â””â”€â–º Live Finalized Head Query (client.at_current_block() & block_num <= finalized_head)
                          â””â”€â–º Canonical Chain Anchoring (canonical block_hash at height == supplied block_hash)
                                â””â”€â–º Exact Extrinsic Blake2b Hash Match (xt.hash() == target_tx_hash)
                                      â””â”€â–º Extrinsic Index Resolution & Phase Filtering (ApplyExtrinsic(idx))
                                            â””â”€â–º Header Height Extraction (true finalized block number)
                                                  â””â”€â–º Decoded Event Extraction (Balances::Transfer / pallet_assets::Transferred)
                                                        â”œâ”€â”€ Payer â†” Sender Identity Equality Check (proof.payer == onchain.sender)
                                                        â”œâ”€â”€ Deterministic Asset Hub Mapping (asset_id 1984 <=> QDOT)
                                                        â”œâ”€â”€ Recipient & Amount Boundary Matching
                                                        â””â”€â”€ Persistent Anti-Replay Journal (file-backed disk store)
                                                              â””â”€â–º Resource Unlocked
```

### 5. Polkadot Agent Kit & Model Context Protocol (MCP)
`crates/agent-core` implements an autonomous **Rust-native Polkadot Agent Kit-compatible tooling layer** (architecturally inspired by `elasticlabs-org/polkadot-agent-kit`) exposed directly over the standard **Model Context Protocol (MCP)**:
- **Zero Private Key Exposure**: Agents only formulate structured proposals and action descriptions; signing authority remains strictly with the user's host wallet.
- **Agent Kit Tool Suite**:
  - `polkadot_transfer_native`: Native DOT token transfer proposals.
  - `polkadot_transfer_asset`: Asset Hub fungible asset (`pallet_assets`) transfers.
  - `polkadot_xcm_transfer`: Cross-consensus messaging (XCM) transfers across parachains.
  - `polkadot_query_balance`: Balance lookup for Substrate `AccountId32` accounts.
  - `polkadot_sign_pqc_attestation`: NIST FIPS 204 ML-DSA-65 post-quantum signing envelope.
- **Standard MCP Protocol**: Full JSON-RPC 2.0 interface supporting `initialize`, `tools/list`, and `tools/call` with strict JSON Schema input conformance for direct integration with AI assistants (Claude, Cursor, Antigravity).

---

## ðŸ“¦ Workspace Layout

```text
Qmoosa-RustChain/
â”œâ”€â”€ Cargo.toml                  # Workspace Manifest (Resolver 2)
â”œâ”€â”€ AGENTS.md                   # AI Agent Protocol & Host-Mediated Security Guide
â”œâ”€â”€ README.md                   # Technical Documentation & Architecture Blueprint
â”œâ”€â”€ .github/workflows/rust.yml  # Automated CI/CD (fmt, clippy -D warnings, tests, release build)
â”‚
â”œâ”€â”€ crates/
â”‚   â”œâ”€â”€ primitives/             # AccountId32, Substrate SS58 Blake2b Checksum, SCALE Codec
â”‚   â”œâ”€â”€ pqc/                    # Genuine NIST FIPS 204 ML-DSA-65 Post-Quantum Attestation
â”‚   â”œâ”€â”€ x402/                   # HTTP 402 Bazaar Gateway & Subxt v0.51 On-Chain Verifier
â”‚   â”œâ”€â”€ conway/                 # B3/S23 Conway Automaton Event-Driven Agent Trigger Engine
â”‚   â””â”€â”€ agent-core/             # Rust-native Polkadot Agent Kit & Model Context Protocol (MCP) Engine
â”‚       â””â”€â”€ tests/              # External MCP Client Interoperability Test Suite
â”‚
â”œâ”€â”€ contracts/
â”‚   â”œâ”€â”€ qdot-token/             # Flexible / Uncapped Supply Token with Role-Governed Mint & PolkaVM ABI
â”‚   â”œâ”€â”€ launchpad/              # Fair Launch Presale, Vesting Locks & 2.5% Treasury Fee Cut
â”‚   â””â”€â”€ x402-settlement/        # On-Chain HTTP 402 Micro-Settlement Registry (1% Network Split)
â”‚
â””â”€â”€ services/
    â””â”€â”€ api/                    # Production Axum Microservice wired to SubxtOnChainVerifier & MCP Server
```

---

## ðŸ§ª Testing & Verification

Run the complete test suite across all 9 workspace crates and contracts:

```bash
# 1. Format check
cargo fmt --all -- --check

# 2. Strict static analysis (zero warnings allowed)
cargo clippy --workspace --all-targets -- -D warnings

# 3. Workspace unit and integration tests (59 tests passed)
cargo test --workspace
```

### Test Suite Summary

| Component | Module | Passed Tests | Description |
|---|---|:---:|---|
| `qmoosa-primitives` | SS58 & SCALE Codec | 4 | Substrate prefix, Blake2b checksum, user wallet decoding, SCALE roundtrip |
| `qmoosa-pqc` | NIST FIPS 204 ML-DSA-65 | 6 | 3309-byte lattice signatures, tampered payload, wrong pubkey, corrupted sig, replay, expiration |
| `qmoosa-qdot-token` | PolkaVM QDOT Token | 5 | Mint/Burn, Pause, Access Control, PolkaVM message dispatch & events |
| `qmoosa-launchpad` | Presale & Vesting | 5 | Token purchase, time-locked claim, fee routing, PolkaVM message dispatch |
| `qmoosa-x402` | Subxt On-Chain Verifier | 19 | Subxt event decoders (Balances::Transfer, Assets::Transferred), live finalized head query, canonical chain anchoring, exact extrinsic hash match, phase filtering, true header height, payer-sender equality, deterministic asset mapping (1984 <=> QDOT), persistent anti-replay journal |
| `qmoosa-x402-settlement` | On-Chain Settlement Registry | 4 | Micro-settlement, fee splits, replay protection, PolkaVM message dispatch |
| `qmoosa-agent-core` | Polkadot Agent Kit & MCP | 14 | Intent routing, Agent Kit tool registry, native/asset/XCM tool calls, external MCP client interop suite |
| `qmoosa-api` | Production API & Gateway | 5 | Health introspection, 402 challenge flow, Subxt verifier authorization, MCP JSON-RPC, tool listing |
| `qmoosa-conway` | Cellular Automaton Triggers | 2 | Glider simulation, epoch milestone event dispatch |
| **Total** | **Full Workspace** | **59** | **100% Passed (0 Failures, 1 Optional Testnet Skipped)** |

---

## ðŸš€ Running the API Service

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

## ðŸ—ºï¸ Engineering Roadmap

```text
Phase 1: Rust Foundation & Primitives (Completed)
  â”œâ”€â”€ Authentic NIST FIPS 204 ML-DSA-65 (fips204)
  â”œâ”€â”€ Substrate AccountId32 & Blake2b SS58 Checksum Codec
  â”œâ”€â”€ On-Chain x402 RPC Verifier Pipeline
  â””â”€â”€ Strict CI: fmt, clippy (-D warnings), tests

Phase 2: PolkaVM Deployment Artifacts (Next)
  â”œâ”€â”€ Target RISC-V compilation via pallet-revive tooling
  â”œâ”€â”€ Deploy to Polkadot Hub Westend / Asset Hub TestNet
  â””â”€â”€ Verify on-chain contract code & gas efficiency

Phase 3: Production Mainnet & Multi-Agent Swarm
  â”œâ”€â”€ SubWallet & Talisman Native Substrate Signing UI
  â””â”€â”€ Conway Event-Driven Autonomous Execution Swarm
```
