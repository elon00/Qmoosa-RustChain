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

### 4. x402 Bazaar Protocol (On-Chain Settlement Verification)
The HTTP 402 gateway enforces an end-to-end on-chain verification pipeline:
```text
HTTP Request
  └─► 402 Payment Required (Challenge ID + SS58 Merchant + Amount)
        └─► On-Chain Polkadot Transaction
              └─► RPC Verification (Tx Hash -> Block Inclusion -> Recipient Match -> Asset Match -> Finality)
                    └─► Anti-Replay Check (Challenge + Tx Hash)
                          └─► Resource Unlocked
```

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
│   ├── x402/                   # HTTP 402 Bazaar Protocol Gateway & On-Chain RPC Verifier
│   ├── conway/                 # B3/S23 Conway Automaton Event-Driven Agent Trigger Engine
│   └── agent-core/             # Host-Mediated Intent Router (Zero Private Key Exposure)
│
├── contracts/
│   ├── qdot-token/             # Flexible / Uncapped Supply Token with Role-Governed Mint & PolkaVM ABI
│   ├── launchpad/              # Fair Launch Presale, Vesting Locks & 2.5% Treasury Fee Cut
│   └── x402-settlement/        # On-Chain HTTP 402 Micro-Settlement Registry (1% Network Split)
│
└── services/
    └── api/                    # High-throughput Axum/Tokio Microservice with x402 Gateway
```

---

## 🧪 Testing & Verification

Run the complete test suite across all 9 workspace crates and contracts:

```bash
# 1. Format check
cargo fmt --all -- --check

# 2. Strict static analysis (zero warnings allowed)
cargo clippy --workspace --all-targets -- -D warnings

# 3. Workspace unit and integration tests (34 tests)
cargo test --workspace
```

### Test Suite Summary

| Component | Module | Passed Tests | Description |
|---|---|:---:|---|
| `qmoosa-primitives` | SS58 & SCALE Codec | 4 | Substrate prefix, Blake2b checksum, user wallet decoding, SCALE roundtrip |
| `qmoosa-pqc` | NIST FIPS 204 ML-DSA-65 | 6 | 3309-byte lattice signatures, tampered payload, wrong pubkey, corrupted sig, replay, expiration |
| `qmoosa-qdot-token` | PolkaVM QDOT Token | 5 | Mint/Burn, Pause, Access Control, PolkaVM message dispatch & events |
| `qmoosa-launchpad` | Presale & Vesting | 5 | Token purchase, time-locked claim, fee routing, PolkaVM message dispatch |
| `qmoosa-x402` | Bazaar Gateway & Verification | 5 | On-chain lookup, recipient check, amount check, finality check, anti-replay |
| `qmoosa-x402-settlement` | On-Chain Settlement Registry | 4 | Micro-settlement, fee splits, replay protection, PolkaVM message dispatch |
| `qmoosa-agent-core` | Intent & Proposal Safety | 3 | Intent classification, proposal construction, zero key exposure |
| `qmoosa-conway` | Cellular Automaton Triggers | 2 | Glider simulation, epoch milestone event dispatch |
| **Total** | **Full Workspace** | **34** | **100% Passed (0 Failures)** |

---

## 🚀 Running the API Service

```bash
cargo run -p qmoosa-api
# Service runs on http://0.0.0.0:8080
```

### Endpoints
- `GET /health`: Platform health, PQC status, and network parameters.
- `GET /api/v1/alpha-model`: HTTP 402 protected resource (requires payment proof).
- `POST /api/pqc/sign`: Generates genuine NIST FIPS 204 ML-DSA-65 envelope.
- `POST /api/pqc/verify`: Verifies ML-DSA-65 envelope against payload.

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
