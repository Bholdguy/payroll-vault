# payrollvault

on-chain earnings ledger for ai agents, on casper.

one contract records payments per agent id. anyone can read any agent's lifetime total, free. writes are restricted on-chain to authorized accounts — not by a ui.

**live on casper testnet:**
- contract package: `a9b847d4b2688ec1bdfd430f0a1a6488cb9395634aa680657a6ce6e5fd7579ee`
- deployment tx: [`2afc6072…f892301`](https://testnet.cspr.live/transaction/2afc60728b260eb117cc537335346761afba8189ae50bfcfdd1df96acf892301)
- site: see `index.html` · docs: see `docs.html`

## what's in the contract

| entry point | access | what it does |
|---|---|---|
| `record_payment(agent_id, amount)` | authorized only | adds `amount` to the agent's lifetime total, emits `PaymentRecorded` (agent id, amount, block time) |
| `get_earnings(agent_id)` | anyone, free | returns the agent's lifetime total (0 if unknown) |
| `authorized_caller()` | anyone, free | returns the account allowed to record |

overflow-checked arithmetic; events follow the casper event standard (ces). v2 (in `src/vault.rs` on the `v2` branch / this repo) adds operators and two-step ownership transfer — 5/5 tests passing, not yet deployed.

## build & test

requires rust nightly-2026-01-01 (pinned in `rust-toolchain`), the `wasm32-unknown-unknown` target, `cargo-odra`, `wasm-strip` (wabt), `wasm-opt` (binaryen).

```bash
cargo odra test          # run the test suite on odra's local vm
cargo odra build         # produce wasm/PayrollVault.wasm
```

## deploy (casper testnet)

```bash
export ODRA_CASPER_LIVENET_SECRET_KEY_PATH=/path/to/secret_key.pem
export ODRA_CASPER_LIVENET_NODE_ADDRESS=https://node.testnet.casper.network
export ODRA_CASPER_LIVENET_CHAIN_NAME=casper-test
export ODRA_CASPER_LIVENET_EVENTS_URL=https://node.testnet.casper.network/events
export ODRA_BACKEND=livenet
cargo run --release --bin payroll_vault_cli -- deploy
```

then interact:

```bash
cargo run --release --bin payroll_vault_cli -- contract PayrollVault record_payment --agent_id "agent-001" --amount 1000 --gas "5 cspr"
cargo run --release --bin payroll_vault_cli -- contract PayrollVault get_earnings --agent_id "agent-001"
```

## stack

rust · [odra 2.8.2](https://github.com/odradev/odra) · casper testnet · ces events

built by ciph.
