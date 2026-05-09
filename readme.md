# prune

[![license](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE-APACHE) <img src="https://img.shields.io/github/repo-size/Bli-AIk/prune.svg"/> <img src="https://img.shields.io/github/last-commit/Bli-AIk/prune.svg"/> <br>
<img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" />

> Current Status: 🚧 Early Development

**prune** — A SoupRune mod manager with CLI and GUI frontends.

| English | Simplified Chinese |
|---------|--------------------|
| English | [简体中文](./readme_zh-hans.md) |

## Introduction

`prune` is the mod manager for [SoupRune](https://github.com/Bli-AIk/souprune).

It provides two frontends:

| Frontend | Tech | Role |
|----------|------|------|
| **prune CLI** | Rust | Developer tooling — plan, build, deploy, serve |
| **prune GUI** | Android (Java) | Mod management for regular users |

The GUI works directly with the local workspace on device (`/storage/emulated/0/SoupRune/projects/`):
it discovers installed mods, switches the active mod, and manages configurations — all without a server.
Only advanced operations (remote build triggers, artifact downloads) rely on the prune server.

The Android GUI is currently native. A future Dioxus-based cross-platform rewrite is planned for
broader device and desktop reach.

## Architecture

```
┌──────────────┐                ┌────────────────┐
│  prune CLI   │                │   prune GUI    │
│  (Rust)      │   HTTP API     │  (Android)     │
│              │◄──────────────►│                │
│  开发端      │  prune server  │  用户端        │
│  构建/部署   │  (高级功能)    │  本地 mod 管理 │
└──────────────┘                └────────────────┘
```

- **GUI** runs on the Android device. It reads and writes mods directly on the local filesystem.
- **CLI** runs on the developer's machine for workspace inspection, build planning, and deploying to devices.
- **Server** (`prune server`) is only needed when the GUI triggers remote builds or fetches artifacts.

## Features

### CLI

- `doctor` — Check the local SoupRune workspace layout.
- `mod list` — List discovered local mods.
- `android plan` — Print ordered build/install/sync stages without executing them.
- `android deploy` — Run the stages for end-to-end Android deployment.
- `server` — Start the HTTP API server (used by the GUI for advanced operations).
- `--gui` — Launch the experimental Dioxus desktop shell (`--features gui`).

### GUI (Android)

- Browse mods installed on the device.
- Switch the active mod and configure language / resolution.
- View mod dependencies and detect missing dependency chains.
- Trigger remote builds and stream progress (requires prune server).
- i18n support (English, Simplified Chinese).

## How to Use

```bash
git clone https://github.com/Bli-AIk/prune.git
cd prune
```

Inside a SoupRune workspace, use the CLI:

```bash
# List available mods
cargo run -p prune -- mod list

# Plan and execute an Android build
cargo run -p prune -- android plan --mod mad_dummy_example --device DEVICE123
cargo run -p prune -- android deploy --mod mad_dummy_example --device DEVICE123
```

## How to Build

### Prerequisites

- Rust 1.85 or later
- For the Dioxus GUI feature on Linux: GTK/WebKit development packages

### Build Steps

```bash
git clone --recurse-submodules https://github.com/Bli-AIk/souprune.git
cd souprune

cargo test -p prune
cargo check -p prune --features gui
```

## Dependencies

### Rust (CLI & Server)

| Crate | Version | Description |
|-------|---------|-------------|
| [clap](https://crates.io/crates/clap) | 4 | CLI argument parsing. |
| [axum](https://crates.io/crates/axum) | 0.8 | HTTP server for the GUI backend. |
| [tokio](https://crates.io/crates/tokio) | 1 | Async runtime. |
| [serde](https://crates.io/crates/serde) + [serde_json](https://crates.io/crates/serde_json) | 1 | Config and API serialization. |
| [toml](https://crates.io/crates/toml) | 0.9 | SoupRune project config parsing. |
| [zip](https://crates.io/crates/zip) | 8 | Mod bundle packaging. |
| [dioxus](https://crates.io/crates/dioxus) | 0.7 | Optional desktop GUI (feature-gated). |

### Android GUI

- See `android/build.gradle` for dependencies.

## Contributing

Issues and Pull Requests are welcome.

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
