# prune

[![license](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE-APACHE) <img src="https://img.shields.io/github/repo-size/Bli-AIk/prune.svg"/> <img src="https://img.shields.io/github/last-commit/Bli-AIk/prune.svg"/> <br>
<img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" />

> 当前状态：🚧 早期开发阶段

**prune** — SoupRune 的 mod 管理器，提供 CLI 与 GUI 双前端。

| English | Simplified Chinese |
|---------|--------------------|
| [English](./readme.md) | 简体中文 |

## 简介

`prune` 是 [SoupRune](https://github.com/Bli-AIk/souprune) 的 mod 管理器。

提供两个前端：

| 前端 | 技术栈 | 角色 |
|------|--------|------|
| **prune CLI** | Rust | 开发者工具 — 规划、构建、部署、服务 |
| **prune GUI** | Android (Java) | 面向普通用户的 mod 管理 |

GUI 直接操作设备上的本地工作区（`/storage/emulated/0/SoupRune/projects/`）：发现已安装的 mod、切换
当前 mod、修改配置——全部在本地完成，无需服务器。仅在需要远程触发构建或下载产物时，才连接 prune server。

Android GUI 当前为原生实现。未来计划用 Dioxus 重写为跨平台应用，覆盖更多设备和桌面端。

## 架构

```
┌──────────────┐                ┌────────────────┐
│  prune CLI   │                │   prune GUI    │
│  (Rust)      │   HTTP API     │  (Android)     │
│              │◄──────────────►│                │
│  开发端      │  prune server  │  用户端        │
│  构建/部署   │  (高级功能)    │  本地 mod 管理 │
└──────────────┘                └────────────────┘
```

- **GUI** 运行在 Android 设备上，直接读写本地文件系统中的 mod。
- **CLI** 运行在开发者机器上，用于工作区检查、构建规划、设备部署。
- **Server**（`prune server`）仅在 GUI 需要远程构建或下载产物时使用。

## 特性

### CLI

- `doctor` — 检查本地 SoupRune 工作区布局。
- `mod list` — 列出已发现的本地 mod。
- `android plan` — 打印有序的构建/安装/同步阶段，不执行。
- `android deploy` — 执行全部阶段，完成端到端 Android 部署。
- `server` — 启动 HTTP API 服务器（供 GUI 高级功能使用）。
- `--gui` — 启动实验性 Dioxus 桌面外壳（`--features gui`）。

### GUI (Android)

- 浏览设备上已安装的 mod。
- 切换当前 mod，配置语言和分辨率。
- 查看 mod 依赖关系，检测缺失的依赖链。
- 远程触发构建并流式查看进度（需要 prune server）。
- 国际化支持（英文、简体中文）。

## 如何使用

```bash
git clone https://github.com/Bli-AIk/prune.git
cd prune
```

在 SoupRune 工作区中使用 CLI：

```bash
# 查看可用 mod
cargo run -p prune -- mod list

# 规划和执行 Android 构建
cargo run -p prune -- android plan --mod mad_dummy_example --device DEVICE123
cargo run -p prune -- android deploy --mod mad_dummy_example --device DEVICE123
```

## 构建方法

### 前置条件

- Rust 1.85 或更高版本
- Linux 上启用 Dioxus GUI feature 需要：GTK/WebKit 桌面开发包

### 构建步骤

```bash
git clone --recurse-submodules https://github.com/Bli-AIk/souprune.git
cd souprune

cargo test -p prune
cargo check -p prune --features gui
```

## 依赖项

### Rust (CLI & Server)

| Crate | 版本 | 作用 |
|-------|------|------|
| [clap](https://crates.io/crates/clap) | 4 | 命令行参数解析。 |
| [axum](https://crates.io/crates/axum) | 0.8 | GUI 后端的 HTTP 服务器。 |
| [tokio](https://crates.io/crates/tokio) | 1 | 异步运行时。 |
| [serde](https://crates.io/crates/serde) + [serde_json](https://crates.io/crates/serde_json) | 1 | 配置与 API 序列化。 |
| [toml](https://crates.io/crates/toml) | 0.9 | SoupRune 项目配置解析。 |
| [zip](https://crates.io/crates/zip) | 8 | Mod 打包。 |
| [dioxus](https://crates.io/crates/dioxus) | 0.7 | 可选桌面 GUI（feature-gated）。 |

### Android GUI

- 依赖项参见 `android/build.gradle`。

## 贡献指南

欢迎提交 Issue 和 Pull Request。

## 许可证

双协议开源，可选择以下任一：

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) 或 http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) 或 http://opensource.org/licenses/MIT)
