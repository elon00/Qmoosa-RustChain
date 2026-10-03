# Qmoosa-RustChain AI Agent Instructions

These instructions apply to Codex, Claude Code, Cursor, Cline, Windsurf, GitHub Copilot, Antigravity, and custom MCP clients working on this repository.

## Architecture boundary

- **Rust backend/services:** Rust, Subxt, Axum/Tokio, x402 verification, ML-DSA-65, agent orchestration.
- **Polkadot Product frontend:** use `@parity/product-sdk` through a Polkadot Host for chain access, signing, and Product storage.
- **Smart contracts:** Rust -> PolkaVM through the current `cargo-pvm-contract` / `pallet-revive` toolchain.
- Never substitute synthetic files or derived hashes for compiler artifacts or on-chain deployment proof.

## Official AI sources

Always prefer these current Polkadot sources before inventing APIs:

- Agent setup: https://docs.polkadot.com/apps/get-started/set-up-your-ai-agent/
- Product skills: https://docs.polkadot.com/reference/apps/skills/
- LLM index: https://docs.polkadot.com/llms.txt
- Full machine-readable docs: https://docs.polkadot.com/ai/llms-full.jsonl
- Product SDK: https://github.com/paritytech/product-sdk
- Playground CLI: https://github.com/paritytech/playground-cli
- Contract Dependency Manager: https://github.com/paritytech/contract-dependency-manager

Refresh the Product SDK skills after major releases. The one-click sync scripts in `scripts/agentics-sync.*` clone/update the official skills into the local ignored cache.

## Signing and security

- Never request, log, commit, or expose private keys or seed phrases.
- AI agents propose actions; users/Hosts/wallets sign them.
- Product frontend signing must use the Host/product-account or explicit wallet approval.
- Do not hand-craft SCALE call bytes when runtime metadata encoding is required.
- Do not fabricate balances or live chain state.
- Rust provides memory-safety guarantees; it does **not** automatically prevent authorization, economic, replay, state-machine, or reentrancy-like logic bugs.
- PQC claims must be backed by actual ML-DSA-65 key generation/sign/verify code and tests.

## Agent toolkit

The MCP/tool registry in `crates/agent-core` exposes structured proposal/context tools including:

- `polkadot_get_balance` — returns an execution plan and requires an approved live chain client; it never invents a balance.
- `polkadot_transfer_native`
- `polkadot_transfer_asset`
- `polkadot_xcm_transfer`
- `polkadot_ai_resources`
- `polkadot_host_rules`

Transaction tools are structured intents. Runtime call bytes must be metadata-encoded at execution time and signed outside the AI agent.

## Quality gates

Before declaring the **agentics integration mission** complete, all of these must pass:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p qmoosa-agent-core
```

The agentics mission is separate from contract deployment. Never claim PolkaVM deployment unless real compiler artifacts and finalized on-chain evidence exist.
