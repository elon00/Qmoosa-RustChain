# Phase 2 — Rust → PolkaVM Contract Build Pipeline

This phase converts the existing Rust domain logic into actual PolkaVM smart-contract projects for `pallet-revive`.

## Ground-truth tooling

The primary Rust contract toolchain is Parity's `cargo-pvm-contract`.

Requirements:

- Rust nightly
- `rust-src`
- `cargo-pvm-contract`

Install on Linux/macOS:

```bash
bash scripts/polkavm/setup-toolchain.sh
```

Install on Windows PowerShell:

```powershell
./scripts/polkavm/setup-toolchain.ps1
```

A PVM-ready contract is built with:

```bash
bash scripts/polkavm/build-contract.sh contracts/<pvm-contract-dir>
```

or directly:

```bash
cd contracts/<pvm-contract-dir>
cargo +nightly pvm-contract build
```

Expected outputs:

```text
target/<profile>/<binary-name>.polkavm
target/<profile>/<binary-name>.abi.json
```

## Important repository boundary

The current crates under:

- `contracts/qdot-token`
- `contracts/launchpad`
- `contracts/x402-settlement`

contain tested Rust domain/state-machine logic and SCALE-oriented interfaces, but they are **not yet cargo-pvm-contract projects**. They must be migrated one by one to real PVM entrypoints and host storage/event APIs before the build command above can produce deployable `.polkavm` artifacts.

Do not report a contract as PolkaVM-deployed until all of these exist:

1. a successful `cargo pvm-contract build`,
2. a `.polkavm` artifact,
3. an ABI JSON artifact,
4. a deterministic code hash,
5. a real `pallet-revive` deployment/instantiation extrinsic,
6. an on-chain contract address,
7. explorer/RPC proof that the deployed code exists.

## Migration order

1. `qdot-token`
2. `x402-settlement`
3. `launchpad`

The token is first because it gives the smallest useful stateful surface for validating storage, caller authorization, events, ABI encoding, and deployment before the more complex launchpad is migrated.

## CI policy

The existing workspace CI remains on stable Rust.

PolkaVM contract builds use nightly separately. This avoids forcing the entire Rust workspace onto nightly before contract migration is complete.
