# bitcoin-rpc-web

A lightweight desktop GUI for interacting with Bitcoin Core's JSON-RPC interface. Built with [wry](https://github.com/nicoreeves/nicoreeves.github.io) (WebView) and a vanilla HTML/JS frontend.

## Features

- Browse all RPC methods from Bitcoin Core's OpenRPC schema, grouped by category
  - This is currently baked in for ~ v30.99 functionality, based on [this branch](https://github.com/bitcoin/bitcoin/compare/master...willcl-ark:bitcoin:json-rpc-schema)
- Fill in parameters with type-aware form fields and execute calls
- Multi-wallet support with a wallet selector dropdown
- Collapsible config panel with optional password persistence
- Built-in tracker music player for extra fun while crafting transactions

## Usage

### With Nix

```
nix run
```

### With Cargo

Requires system dependencies for WebView:

- **Linux:** GTK 3, WebKitGTK 4.1, libsoup 3, ALSA
- **macOS:** No extra dependencies

```
cargo run --release
```

Configure the RPC connection (URL, user, password) via the gear icon in the sidebar. The app connects to `http://127.0.0.1:8332` by default.

Enable debug logging with `RUST_LOG=1`.

## Music

Tracker tunes sourced from [The Mod Archive](https://modarchive.org). Playback uses [xmrs](https://crates.io/crates/xmrs) and [rodio](https://crates.io/crates/rodio).
