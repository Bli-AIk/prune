# prune

[![license](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE-APACHE) <img src="https://img.shields.io/github/repo-size/Bli-AIk/prune.svg"/> <img src="https://img.shields.io/github/last-commit/Bli-AIk/prune.svg"/> <br>
<img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" />

> Current Status: 🚧 Early Development

**prune** — A SoupRune mod manager and Android build assistant.

| English | Simplified Chinese |
|---------|--------------------|
| English | [简体中文](./readme_zh-hans.md) |

## Introduction

`prune` is the management and build-assistant tool for [SoupRune](https://github.com/Bli-AIk/souprune).

Its first priority is a Linux-first Android workflow: plan, build, install, and sync the SoupRune runtime, builtin WASM,
active mod, and dependency mods to an Android device with one command path.

The active implementation lives on the [`feat/init`](https://github.com/Bli-AIk/prune/tree/feat/init) branch. The `main`
branch intentionally contains only repository metadata, README files, and licenses.

## Features

* **Android build planning** — Generate ordered build/install/sync stages before running them.
* **Android deploy helper** — Execute the same stages for local Android deployment.
* **Dependency-aware mod sync** — Build and sync the active mod together with dependency mods.
* **CLI-first workflow** — Provide commands such as `doctor`, `android plan`, `android deploy`, and `mod list`.
* **Dioxus GUI shell** — Keep a feature-gated desktop GUI entry point for future cross-platform tooling.

## How to Use

Use the active development branch:

```bash
git clone https://github.com/Bli-AIk/prune.git
cd prune
git switch feat/init
```

Inside the SoupRune workspace, the tool is intended to be used as the `crates/prune` submodule:

```bash
cargo run -p prune -- android plan --mod mad_dummy_example --device DEVICE123
cargo run -p prune -- android deploy --mod mad_dummy_example --device DEVICE123
```

## How to Build

### Prerequisites

* Rust 1.85 or later
* For Android workflows: Android SDK, Android NDK, JDK, Gradle wrapper, `adb`
* For the GUI feature on Linux: GTK/WebKit desktop development packages

### Build Steps

```bash
# 1. Clone the SoupRune workspace with submodules
git clone --recurse-submodules https://github.com/Bli-AIk/souprune.git
cd souprune

# 2. Use the prune development branch
git -C crates/prune switch feat/init

# 3. Run tests
cargo test -p prune

# 4. Check the optional GUI feature
cargo check -p prune --features gui
```

## Dependencies

Core dependencies include:

| Crate | Version | Description |
|-------|---------|-------------|
| [clap](https://crates.io/crates/clap) | 4 | Handles command-line arguments for the CLI. |
| [anyhow](https://crates.io/crates/anyhow) | 1 | Simple and flexible error handling. |
| [serde](https://crates.io/crates/serde) | 1 | Parses project and mod metadata. |
| [toml](https://crates.io/crates/toml) | 0.9 | Reads SoupRune project configuration files. |
| [dioxus](https://crates.io/crates/dioxus) | 0.7 | Optional desktop GUI shell. |

## Contributing

Issues and Pull Requests are always welcome! Whether you want to fix a bug, add a feature, or just correct a typo.

## License

This project is dual-licensed, you can choose either:

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
* MIT license ([LICENSE-MIT](LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
