# AGENTS.md

## Purpose
Practical contributor guide for `bitcoin-rpc-web`. Keep this file short and update it as team conventions evolve.

## Project Snapshot
- Stack: Rust (`edition = 2024`) desktop app using `iced` for UI.
- Core function: Bitcoin Core JSON-RPC dashboard/client with optional ZMQ feed.
- Note: `README.md` still mentions `wry`; current implementation is `iced` (`src/app/mod.rs`).

## Repo Map
- `src/app`: app state, messages, update loop, subscriptions.
- `src/core`: RPC client, config persistence, schema/dashboard logic.
- `src/ui`: UI views/components.
- `src/zmq.rs`: ZMQ subscriber integration.
- `assets/openrpc.json`: baked RPC method schema.
- `tunes/`: optional tracker music assets (feature-gated audio).

## Run And Build
- Nix run: `nix run`
- Cargo run: `cargo run --release`
- Run without audio: `cargo run --release --no-default-features`
- Debug logs: `RUST_LOG=bitcoin_rpc_web=debug cargo run --release`

## Testing
- Default: `cargo test`
- If you change only one module, prefer targeted tests first:
  - `cargo test <module_or_test_name>`

## Safety Rules (Do Not Weaken)
- Do not relax RPC host safety checks unless explicitly requested.
  - Public/untrusted RPC endpoints are blocked by default.
  - Bypass exists only via `DANGER_INSECURE_RPC=1`.
- Do not weaken config file write security in `src/core/config_store.rs`.
  - Keep atomic writes.
  - Keep owner-only permissions on Unix (`0600`).

## Editing Expectations
- Keep changes scoped and minimal.
- Prefer code as source of truth over stale docs; update docs when behavior changes.
- Avoid unrelated refactors in functional changes.

## When Finishing A Change
- Run formatting/tests relevant to touched code.
- Summarize user-visible behavior changes and any new env vars/flags.
