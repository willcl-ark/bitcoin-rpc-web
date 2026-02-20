# bitcoin-rpc-web Deep Review

Date: 2026-02-20  
Scope: repository structure, Rust architecture, GUI/update loop behavior, RPC/ZMQ security, and testability.

## Executive Summary

The project is cleanly organized at a module level (`app`, `core`, `ui`, `zmq`) and compiles/tests successfully, but there are several high-impact issues:

1. Security checks for RPC target safety are inconsistent across config paths.
2. Credentials are written to disk without explicit restrictive permissions.
3. Async task results can apply stale data after runtime config changes.
4. ZMQ UI polling does avoidable repeated work and can degrade responsiveness at scale.

This document lists each finding with exact references and concrete fix guidance.

## Findings

### 1) High: Config file may be world-readable (credential disclosure)

- Location: `src/core/config_store.rs:429`
- Relevant code: `fs::write(&self.path, bytes)`
- Why this matters:
  - `RpcConfig` includes `user` and `password` (`src/core/rpc_client.rs:20`).
  - On many systems, `fs::write` results in mode derived from umask (often `0644`), which can expose credentials to other local users.
- Impact:
  - Local credential leak for Bitcoin Core RPC access.
  - Higher risk if user reuses credentials or exposes wallet RPC permissions.

#### Fix guidance

1. Use explicit secure file creation on Unix:
   - `std::os::unix::fs::OpenOptionsExt::mode(0o600)`.
2. Write atomically:
   - Write to temporary file in same directory.
   - `sync_all` temp file.
   - Rename temp file to target path.
3. For existing files:
   - On load/save, detect overly permissive mode and warn or chmod to `0600` (Unix).
4. Keep parent directory creation as-is (`create_dir_all`), but consider documenting expected directory perms.

#### Suggested patch shape

- Update `ConfigStore::save` in `src/core/config_store.rs`:
  - replace `fs::write` with secure `OpenOptions` + `write_all`.
  - add atomic rename flow.
- Add tests:
  - `#[cfg(unix)]` test asserting mode `0o600`.

---

### 2) High: Startup path bypasses unsafe-host RPC guard

- Locations:
  - `src/app/state.rs:233` (loads config and constructs client)
  - `src/app/state.rs:295` (creates `RpcClient` from loaded config)
  - `src/app/mod.rs:15` (immediate `DashboardTick` on startup)
- Existing guard lives in:
  - `src/app/update.rs:138`
  - `src/app/update.rs:215`
- Why this matters:
  - `is_safe_rpc_host` is enforced for connect/save UI actions, but not for persisted startup config.
  - A saved public RPC URL can be used at startup without `DANGER_INSECURE_RPC=1`.

#### Fix guidance

1. Introduce a single validation entry point for runtime config application:
   - e.g. `fn validate_runtime_config(config: &RpcConfig) -> Result<(), String>`.
2. Call it in all paths before `RpcClient::new(...)` or `apply_runtime_config(...)`:
   - startup init in `State::new`
   - connect
   - save
   - reload
3. On startup invalid config:
   - fallback to default safe config;
   - surface clear error in `state.config.store_error` / `state.config.error`.

#### Suggested patch shape

- `src/app/update.rs`:
  - extract host safety check into shared helper.
- `src/app/state.rs`:
  - validate loaded config before assigning runtime/client.
  - store explanatory error when rejected.

---

### 3) High: Reload path bypasses unsafe-host RPC guard

- Locations:
  - `src/app/update.rs:181` (`ConfigReloadFinished`)
  - `src/app/update.rs:183` (direct `apply_runtime_config(state, config)`)
- Why this matters:
  - Reloading config from disk allows same bypass even after app is running.
  - Behavior is inconsistent and surprising versus Connect/Save.

#### Fix guidance

1. In `ConfigReloadFinished(Ok(config))`, run the same shared validation helper used by connect/save/startup.
2. If invalid:
   - keep prior runtime config;
   - set `state.config.error` to actionable message;
   - do not call `apply_runtime_config`.
3. Keep one canonical error text to reduce UI confusion and testing complexity.

---

### 4) Medium: Stale async results can overwrite newer runtime state

- Locations:
  - Dispatch points:
    - `src/app/update.rs:557` (`start_dashboard_refresh`)
    - `src/app/update.rs:567` (`start_partial_dashboard_refresh`)
  - Apply points:
    - `src/app/update.rs:425` (`DashboardLoaded`)
    - `src/app/update.rs:472` (`DashboardPartialLoaded`)
- Why this matters:
  - Requests capture a cloned `RpcClient`.
  - If user changes config while request is in flight, old response may still apply and replace state with stale data.

#### Fix guidance

1. Add request generation/versioning:
   - `dashboard_request_gen: u64` in `DashboardState`.
   - increment whenever runtime config changes (`apply_runtime_config`).
2. Include generation with each task result:
   - message variants carry `(generation, result)`.
3. Ignore completion if generation != current generation.
4. Apply same pattern for RPC execute path if you want consistent behavior (`RpcExecuteFinished`).

#### Suggested patch shape

- `src/app/message.rs`: adjust message payloads.
- `src/app/update.rs`: tag tasks and gate apply logic.
- `src/app/state.rs`: add generation field.

---

### 5) Medium: Blocking HTTP calls run in async tasks

- Locations:
  - `src/app/update.rs:688` (`test_rpc_config`)
  - `src/app/update.rs:700` (`run_single_rpc`)
  - `src/app/update.rs:710` (`run_batch_rpc`)
  - `src/app/update.rs:716` (`load_dashboard`)
  - `src/core/rpc_client.rs` uses synchronous `ureq`.
- Why this matters:
  - `Task::perform(async ...)` with blocking I/O can consume async worker threads and reduce UI responsiveness under slow network/RPC conditions.

#### Fix guidance

Option A (lower change risk):
1. Wrap blocking RPC work in a dedicated blocking executor boundary (e.g. `tokio::task::spawn_blocking` where available in iced runtime setup).

Option B (cleaner long-term):
1. Replace `ureq` with async client (`reqwest` async / `hyper`) and propagate async RPC methods.

If keeping `ureq`, at minimum:
1. Configure timeouts in `ureq::Agent` builder.
2. Surface timeout-specific user errors.

---

### 6) Medium: ZMQ polling does full scan/rebuild every 300ms

- Locations:
  - Poll frequency: `src/app/subscription.rs:13`
  - Scan path: `src/app/update.rs:592`
  - Rebuild path: `src/app/update.rs:608`
  - Buffer bounds: `src/core/rpc_client.rs:8`..`src/core/rpc_client.rs:9`
- Why this matters:
  - Every tick iterates the full queue and rebuilds `recent_events` vector even when only 0-1 new messages arrived.
  - At large `zmq_buffer_limit`, this becomes repeated O(n) work on UI thread.

#### Fix guidance

1. Process incrementally:
   - Track last processed cursor.
   - Only iterate newly appended messages.
2. Maintain UI ring buffer directly in `state.zmq.recent_events`:
   - append new events;
   - truncate to fixed cap (e.g. 80).
3. Keep shared ZMQ queue for transport history if needed, but avoid full-copy on each tick.

#### Suggested patch shape

- `src/app/update.rs`:
  - rewrite `poll_zmq_feed` to avoid rebuilding from entire queue.

---

### 7) Medium: `State` is too coupled (orchestration + domain + transport handles + UI)

- Primary location: `src/app/state.rs:210` onward
- Symptoms:
  - `State` owns UI concerns, network clients, shared mutex state, thread handles, and domain snapshots.
  - `ui` reads domain structs directly, e.g. `src/ui/dashboard.rs:11`.
  - `update.rs` central reducer is large and cross-cutting.
- Why this matters:
  - Harder to unit-test business rules independently of UI.
  - Higher chance of regressions when adding features (tight coupling across tabs/subsystems).

#### Refactor guidance

1. Split state into subdomains with clear boundaries:
   - `ConfigModel`
   - `RpcConsoleModel`
   - `DashboardModel`
   - `ZmqModel`
2. Introduce application services/use-cases in `core` or `app/service` layer.
3. Map domain snapshot -> view model in UI boundary functions.
4. Keep `State` as composition root, but avoid direct transport handles in view-facing model.

---

### 8) Low: Dashboard parser silently drops/masks data issues

- Locations:
  - Silent peer skip: `src/core/dashboard_service.rs:210`..`src/core/dashboard_service.rs:215`
  - Broad defaults with `unwrap_or`: e.g. `src/core/dashboard_service.rs:195`, `:231`..`:260`
- Why this matters:
  - RPC schema/version drift can be hidden.
  - UI may show misleading defaults instead of signaling parse/data mismatch.

#### Fix guidance

1. Track parse warnings:
   - `Vec<String>` or structured warning enum in `DashboardSnapshot`.
2. Log warnings with peer IDs/field names.
3. Show small warning indicator in dashboard when partial parsing occurred.
4. Keep fallback behavior for robustness, but make degradation visible.

---

### 9) Low: Clippy `-D warnings` currently fails

- Errors:
  - `src/ui/components.rs:213`
  - `src/ui/music_bar.rs:100`
- Rule: `clippy::needless_update`
- Why this matters:
  - Reduces CI strictness/confidence if lint gate is used.

#### Fix guidance

1. Remove `..container::Style::default()` in fully-specified struct literals.
2. Add lint check in CI pipeline to prevent regressions:
   - `cargo clippy --all-targets -- -D warnings`

---

## Test Coverage Gaps (what to add next)

Current suite passes (`cargo test -q`, 13 tests), but misses critical behavioral guarantees.

### A) Security invariants

1. Startup rejects unsafe host unless `DANGER_INSECURE_RPC=1`.
2. Reload rejects unsafe host and keeps previous runtime config.
3. Config file save uses secure permissions (Unix).

### B) Async consistency

1. Dashboard result with stale generation is ignored.
2. Partial refresh stale result is ignored.

### C) ZMQ performance behavior

1. Polling with large queue processes only new events.
2. `recent_events` remains bounded and ordered.

### D) Reducer correctness

1. `handle_config` state flags (`connect_in_flight`, `save_in_flight`) transitions.
2. `apply_runtime_config` side effects:
   - client replaced,
   - form synchronized,
   - ZMQ runtime reapplied correctly.

---

## Suggested Implementation Order

1. Security hardening first:
   - Findings #1, #2, #3.
2. Data-race/staleness prevention:
   - Finding #4.
3. Responsiveness/perf:
   - Findings #5, #6.
4. Maintainability:
   - Findings #7, #8.
5. CI hygiene:
   - Finding #9.

---

## Validation Checklist After Fixes

1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test`
4. Manual:
   - start with unsafe saved URL and verify rejection,
   - reload unsafe config and verify rejection + old config retained,
   - high-volume ZMQ stream and verify UI remains responsive.

