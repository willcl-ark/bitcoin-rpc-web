# Iced Migration Plan (Clean Break)

## Progress (as of 2026-02-20)

- Completed: Phase 0 (groundwork/cleanup)
- Completed: Phase 1 (core services extraction)
- Completed: Phase 2 (Config UI) with connect/save/reload flow wired and runtime ZMQ updates applied.
- Completed: Phase 3 (RPC UI) with method search, single call, batch mode, and response rendering.
- Completed: Phase 4 (Dashboard UI) with periodic refresh + peer list/detail rendering.
- In progress: Phase 5 (ZMQ-driven refreshes) with polling, topic mapping, debounce, and feed indicator wiring.
- Current runtime blocker on NixOS devshell: `NoWaylandLib` when creating the `iced`/`winit` event loop.
- Next focus: Phase 5 polish/completion and Flake migration updates for Wayland/X11 graphics libs.

## Scope

This is a full replacement of the current WebView/HTML frontend with a native `iced` desktop UI.
No backward compatibility is required.

### Must support at launch
- Config settings (RPC URL/auth/wallet/poll/ZMQ settings, persistence)
- RPC calling (method discovery + execution, including batch calls)
- Dashboard view (chain/network/mempool/peers/traffic)

### Explicitly out of scope for initial launch
- Visual parity with the current WebView UI
- Feature flags or runtime switch between old/new UI
- Keeping `wry`, `web/*`, or protocol bridge code paths

---

## Target Architecture

## 1) High-level runtime model

- Single native app using `iced` event loop.
- UI drives all actions through typed `Message` events.
- RPC and ZMQ work run asynchronously; results feed back into UI state.
- No local `app://` protocol, no browser fetch, no JS.

## 2) Module structure

Proposed layout:

- `src/main.rs`
  - app bootstrap, logging init, iced run entrypoint
- `src/app/`
  - `mod.rs`
  - `state.rs` (top-level app state)
  - `message.rs` (global message enum)
  - `update.rs` (state transitions + task spawning)
  - `view.rs` (top-level layout + tab routing)
  - `subscription.rs` (tick/subscription wiring)
- `src/ui/`
  - `dashboard.rs`
  - `rpc.rs`
  - `config.rs`
  - `components.rs` (small reusable rows/cards/editors)
- `src/core/`
  - `rpc_client.rs` (typed client wrapping JSON-RPC transport)
  - `dashboard_service.rs` (batch dashboard fetch + mapping)
  - `config_store.rs` (load/save config file)
  - `schema.rs` (openrpc load + indexed method metadata)
- Reused modules with minimal adaptation:
  - `src/rpc.rs` (transport/safety checks)
  - `src/zmq.rs`
  - `src/logging.rs`
  - `src/music.rs` (only if retained)

## 3) UI information architecture

- Left navigation (default iced widgets):
  - `Dashboard`
  - `RPC`
  - `Config`
- Main panel switches by active tab.
- Default iced styling and spacing; no custom theme work in initial launch.

---

## Product Behavior Specification

## 1) Config screen

Fields:
- RPC URL
- RPC user
- RPC password
- Wallet
- Poll interval seconds
- ZMQ address
- ZMQ buffer limit

Actions:
- `Connect` (validate + apply in-memory config + warm test call)
- `Save` (persist to config file)
- Optional `Reload` from disk

Behavior:
- Use existing safety logic (`is_safe_rpc_host` + `DANGER_INSECURE_RPC` override).
- Display inline validation and transport errors.
- Password persistence is explicit and always saved as configured by user.

Persistence:
- Store config as JSON in OS config dir:
  - Linux: `$XDG_CONFIG_HOME/bitcoin-rpc-web/config.json` fallback `~/.config/...`
  - macOS: `~/Library/Application Support/bitcoin-rpc-web/config.json`

## 2) RPC screen

Required launch behavior:
- Load and parse `assets/openrpc.json`.
- Search/filter method list by name/category.
- Select method and execute with user-provided params.

MVP params UX:
- Raw JSON params editor (`[]` default).
- Validate JSON before send.
- Execute button disabled while in-flight.
- Response viewer shows full JSON (`result` or `error`).

Batch support:
- Add optional batch editor mode:
  - text area for array of JSON-RPC requests
  - execute as a single batch request
- Keep single-call mode default.

## 3) Dashboard screen

Data sources per refresh:
- `getblockchaininfo`
- `getnetworkinfo`
- `getmempoolinfo`
- `getpeerinfo`
- `uptime`
- `getnettotals`

Requirements:
- Use one batched RPC request per refresh cycle.
- Configurable refresh interval from settings.
- Render simple cards/lists with defaults:
  - Chain summary
  - Mempool summary
  - Network summary
  - Traffic totals
  - Peers list (compact table-like rows)

ZMQ integration:
- Reuse current subscriber logic.
- Use ZMQ events to trigger debounced partial refreshes:
  - `hashblock` => refresh chain + mempool
  - `hashtx` => refresh mempool
- Fallback periodic full refresh always remains active.

---

## Delivery Plan (Execution Order)

## Phase 0: Groundwork and cleanup [DONE]

Tasks:
- Remove web frontend artifacts:
  - delete `web/index.html`, `web/app.js`, `web/style.css`
  - delete custom protocol handling in `src/protocol.rs`
- Remove WebView dependencies from Cargo and code:
  - remove `wry`, `winit`, Linux `gtk` dependency usage
- Establish new module tree (`src/app`, `src/ui`, `src/core`)

Acceptance criteria:
- Project compiles with a minimal iced window and placeholder tabs.
- No references to `wry`, custom protocol, or web assets remain.
Status:
- Done.

## Phase 1: Core services extraction [DONE]

Tasks:
- Implement `core::rpc_client`:
  - `call(method, params)` and `batch(calls)`
  - preserve existing auth/url/wallet safety behavior
- Implement `core::schema`:
  - load openrpc JSON at startup
  - provide searchable method index
- Implement `core::config_store`:
  - load/save config file and defaults
- Implement `core::dashboard_service`:
  - batched fetch and typed dashboard snapshot mapping

Acceptance criteria:
- Unit tests pass for client payload normalization and error mapping.
- Config load/save roundtrip tested.
- Dashboard snapshot builder tested against representative JSON.
Status:
- Done.

## Phase 2: Config UI [NEXT]

Tasks:
- Build full Config tab with editable fields + `Connect` + `Save`.
- Wire validation and transport checks into UI errors.
- Update in-memory runtime config and trigger downstream updates.
- Hook ZMQ start/stop on ZMQ address change.

Acceptance criteria:
- User can configure node and connect from fresh app start.
- Settings persist across restarts.
- Invalid/public URL blocked unless `DANGER_INSECURE_RPC=1`.

## Phase 3: RPC UI

Tasks:
- Build RPC method list + search + selection panel.
- Build params JSON input + execute action.
- Render response JSON with scroll container.
- Add optional batch mode input and execution path.

Acceptance criteria:
- Single RPC calls work against connected node.
- Batch calls execute and show per-item results/errors.
- In-flight state and errors are clear in UI.

## Phase 4: Dashboard UI

Tasks:
- Build simple card-based dashboard using default iced containers.
- Implement periodic refresh subscription from configured interval.
- Wire initial full batch refresh.
- Add peer list rendering and lightweight detail view pane.

Acceptance criteria:
- Dashboard updates correctly at configured interval.
- All required dashboard sections render valid data.
- UI remains responsive under normal refresh rates.

## Phase 5: ZMQ-driven refreshes

Tasks:
- Integrate ZMQ state snapshots into app update loop.
- Add debounced refresh triggers by topic mapping.
- Track ZMQ connection indicator and basic feed summary.

Acceptance criteria:
- New block/tx notifications reduce stale dashboard lag.
- Debounce prevents excessive refresh storms.
- Disconnect/reconnect handled gracefully.

## Phase 6: Docs, polish, hardening

Tasks:
- Rewrite README for iced architecture + usage.
- Add operational docs for config path and environment vars.
- Add smoke/integration checklist and performance notes.
- Remove dead code and legacy comments.

Acceptance criteria:
- README matches runtime behavior.
- No dead WebView codepaths remain.
- Build/dev shell instructions work on Linux + macOS.

---

## Flake Migration Plan

## 1) Cargo changes

Remove dependencies:
- `wry`
- `winit`
- `gtk` target dependency

Add dependency:
- `iced` (wgpu backend + tokio runtime feature set)

Keep:
- `ureq`, `serde_json`, `tracing`, `tracing-subscriber`, `zmq2`, `sha2`
- optional audio deps unchanged unless intentionally removed

## 2) `flake.nix` package/build inputs

Remove Linux WebView deps:
- `webkitgtk_4_1`
- `gtk3`
- `glib`
- `libsoup_3`
- `wrapGAppsHook3`
- `glib-networking`

Add Linux native graphics/window deps for iced/wgpu stack:
- `wayland`
- `libxkbcommon`
- `xorg.libX11`
- `xorg.libXcursor`
- `xorg.libXi`
- `xorg.libXrandr`
- `vulkan-loader`
- OpenGL/Mesa libs (`mesa` or equivalent in nixpkgs)

Keep shared deps:
- `openssl`
- `zeromq`
- `pkg-config`
- `alsa-lib` (if audio feature enabled)

macOS:
- keep security/system frameworks
- validate additional framework requirements from iced docs if needed

## 3) Dev shell updates

- Remove WebKit/GTK-specific env exports.
- Include same graphics libs as build inputs for local runs.
- Keep rust tooling (`rust-analyzer`, `cargo-watch`, etc.).

## 4) Validation gates for flake

- `nix build` succeeds on Linux and macOS targets listed in flake.
- `nix run` opens iced window and can execute at least one RPC call.
- `nix develop` has all libs needed for `cargo run`.

---

## Data Model Plan

## 1) App state (initial shape)

- `AppState`
  - `active_tab: Tab`
  - `config: RuntimeConfig`
  - `connection: ConnectionState`
  - `schema: SchemaState`
  - `rpc_view: RpcViewState`
  - `dashboard: DashboardState`
  - `zmq: ZmqUiState`

## 2) Message taxonomy

- Navigation:
  - `TabSelected(Tab)`
- Config:
  - `ConfigEdited(Field, String)`
  - `ConfigConnectPressed`
  - `ConfigSavePressed`
  - `ConfigLoaded(Result<...>)`
  - `ConfigConnected(Result<...>)`
- RPC:
  - `RpcSearchChanged(String)`
  - `RpcMethodSelected(String)`
  - `RpcParamsChanged(String)`
  - `RpcExecutePressed`
  - `RpcExecuted(Result<...>)`
  - `RpcBatchModeToggled(bool)`
  - `RpcBatchChanged(String)`
- Dashboard:
  - `DashboardTick`
  - `DashboardLoaded(Result<DashboardSnapshot, String>)`
  - `DashboardPartialRefreshRequested(PartialSet)`
- ZMQ:
  - `ZmqPollTick`
  - `ZmqStateObserved(ZmqSnapshot)`
  - `ZmqEventHint(Topic)`

## 3) Async strategy

- `Task::perform` for RPC actions.
- `Subscription::run`/time-based subscription for periodic ticks.
- No blocking work in `view` or `update`.

---

## Testing and Verification Plan

## 1) Unit tests

- `rpc_client`:
  - single payload normalization
  - batch payload normalization
  - id mapping and response ordering assumptions
- `config_store`:
  - defaults when file missing
  - roundtrip serialization
- `dashboard_service`:
  - parse and map each required RPC result
  - missing field handling
- `schema`:
  - openrpc load and method index/search

## 2) Integration/smoke checks

- Connect with valid local node config.
- Execute representative RPC from RPC tab (`getblockchaininfo`).
- Dashboard refreshes on interval and updates values.
- ZMQ event triggers partial refresh without UI freeze.
- Restart app and confirm persisted settings load.

## 3) Performance/robustness checks

- Verify no unbounded queue growth in dashboard refresh logic.
- Verify debounce caps ZMQ-triggered refresh rate.
- Verify app remains interactive under repeated RPC failures.

---

## Risks and Mitigations

- Risk: UI async flow complexity causes stale/in-flight races.
  - Mitigation: centralize in-flight flags and generation counters in state.

- Risk: cross-platform runtime libs missing for wgpu/iced in Nix.
  - Mitigation: explicit flake dependency matrix + Linux/macOS validation gates.

- Risk: RPC error surfaces become inconsistent after moving from JS.
  - Mitigation: typed error model in `rpc_client` and unified rendering helper.

- Risk: ZMQ thread + UI polling create lock contention.
  - Mitigation: short critical sections; snapshot-copy before UI processing.

---

## Definition of Done

The migration is complete when all of the following are true:

- App has no WebView/HTML/JS code paths.
- `iced` UI provides Config, RPC, and Dashboard tabs with required functionality.
- Dashboard refresh uses batched RPC requests.
- Settings persist on disk and apply correctly at runtime.
- Flake builds and runs the iced app on supported systems.
- README and developer docs reflect the new architecture.
