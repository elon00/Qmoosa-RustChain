# 🦀 Qmoosa-RustChain

A native, high-performance **Rust-first** Web4 platform designed for **Polkadot Hub / PolkaVM / Substrate**, featuring Post-Quantum Cryptography (PQC), x402 Bazaar Protocol, and Conway Automaton Agentics.

---

## 🏗️ Architectural Philosophy

Instead of relying on Ethereum Virtual Machine (EVM) emulation, **Qmoosa-RustChain** leverages Rust's native alignment with the Polkadot SDK and PolkaVM:
- **Zero-Cost Memory Safety**: Rust's ownership and borrow checker guarantee exclusive state mutation, preventing entire classes of vulnerabilities like reentrancy and memory corruption at compile time.
- **PolkaVM Native Execution**: Targets Polkadot's next-generation RISC-V execution environment (`pallet-revive` / PolkaVM) for near-native execution speed with lower gas fees.
- **Native Substrate Identity**: Works with native Substrate SS58 addresses (`5CwB8yg...`) without forcing Ethereum `0x...` key translation.
- **Host-Mediated Security**: AI agents formulate and propose transactions; user wallets (Talisman, SubWallet, Polkadot.js) sign; zero private keys ever touch LLM prompts or backend code.

---

## 📦 Workspace Layout

```text
Qmoosa-RustChain/
├── Cargo.toml                  # Cargo Workspace Definition
├── AGENTS.md                   # AI Coding Agent Architecture Context
├── crates/
│   ├── pqc/                    # NIST FIPS 204 ML-DSA-65 Post-Quantum Attestation
│   ├── x402/                   # HTTP 402 Bazaar Protocol Payment & Replay Protection
│   ├── conway/                 # B3/S23 Conway Game of Life Event-Driven Agent Triggers
│   └── agent-core/             # AI Multi-Model Intent Router & Safe Transaction Proposer
├── contracts/
│   ├── qdot-token/             # Flexible / Uncapped Supply Tokenomics with Role-Governed Mint
│   ├── launchpad/              # Fair Launch Presale, Vesting Locks, and 2.5% Treasury Routing
│   └── x402-settlement/        # On-Chain HTTP 402 Micro-Settlement Registry with 1% Fee Cut
└── services/
    └── api/                    # High-throughput Axum/Tokio Microservice with x402 Gateway
```

---

## 🧪 Testing & Verification

Run the complete test suite across all workspace crates and contracts:

```bash
cargo test --workspace
```

---

## 🚀 Running the API Service

```bash
cargo run -p qmoosa-api
# Service runs on http://0.0.0.0:8080
```
