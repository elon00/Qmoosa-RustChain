# GitHub Copilot Instructions — Qmoosa-RustChain

Read `AGENTS.md` first.

For Polkadot Product/frontend work:
- Use current `@parity/product-sdk` patterns and Host-mediated capabilities.
- Do not open direct frontend RPC/WebSocket connections when Product SDK/Host access is required.
- Do not expose keys; signing stays with product accounts or user-approved wallets.
- Consult the official Polkadot AI resources listed in `agentics/polkadot-ai-resources.json`.

For Rust backend work:
- Use typed Rust/Subxt interfaces and runtime metadata rather than guessed SCALE bytes.
- AI tools produce structured transaction intents, not signatures.
- Never invent balances, deployment addresses, code hashes, explorer proofs, or test results.

Before declaring agentics integration complete, run `scripts/agentics-sync.sh` or `scripts/agentics-sync.ps1`.
