# Rust Coding Agent Guidelines (AGENTS.md)

This file instructs AI coding assistants (Cursor, Claude Code, Cline, Windsurf, Antigravity) working on **Qmoosa-RustChain**.

## 1. Architectural Philosophy
- **Rust-First Polkadot Native**: We target Polkadot Hub (`pallet-revive` / PolkaVM RISC-V) and Substrate/FRAME modular architecture, rather than Ethereum virtual machine emulation.
- **Zero Private Keys in Code or AI Context**: Private keys and seed phrases MUST NEVER be logged, passed into AI prompts, or stored in code. All transactions use **Host-mediated signing**.
- **Deterministic Memory Safety**: Leverage Rust's borrow checker to ensure exclusive state transitions, compile-time reentrancy immunity, and strict overflow guards.
- **NIST ML-DSA-65 PQC Attestation**: Lattice-based post-quantum cryptographic envelopes verify all off-chain agent intents and artifacts.
- **x402 Bazaar Protocol**: Machine-to-machine HTTP 402 micro-settlements for autonomous agents.

## 2. Workspace Layout
- `crates/pqc`: NIST FIPS 204 ML-DSA-65 post-quantum signing & verification
- `crates/x402`: HTTP 402 Bazaar Protocol payment challenge & verification engine
- `crates/conway`: Game of Life B3/S23 cellular automaton event-driven task generator
- `crates/agent-core`: Multi-model AI agent intent router and transaction proposer
- `contracts/qdot-token`: Role-governed uncapped/flexible supply token
- `contracts/launchpad`: Presale campaigns, vesting locks, and treasury fee routing
- `contracts/x402-settlement`: On-chain payment settlement verification
- `services/api`: Axum/Tokio web service with protected endpoints

## 3. Testing Standard
Every crate must maintain comprehensive unit tests (`cargo test --workspace`) verifying positive and negative security invariants.
