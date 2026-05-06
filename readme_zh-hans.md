# prune

[![license](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE-APACHE) <img src="https://img.shields.io/github/repo-size/Bli-AIk/prune.svg"/> <img src="https://img.shields.io/github/last-commit/Bli-AIk/prune.svg"/> <br>
<img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" />

> 当前状态：🚧 早期开发阶段

**prune** — SoupRune 的 mod 管理器与 Android 构建辅助工具。

| English | Simplified Chinese |
|---------|--------------------|
| [English](./readme.md) | 简体中文 |

## 简介

`prune` 是 [SoupRune](https://github.com/Bli-AIk/souprune) 的管理与构建辅助工具。

它当前的首要目标是 Linux 优先的 Android 工作流：通过一条命令链路规划、构建、安装，并把 SoupRune 运行时、
builtin WASM、当前 mod 与依赖 mod 同步到 Android 设备。

当前实现位于 [`feat/init`](https://github.com/Bli-AIk/prune/tree/feat/init) 分支。`main` 分支刻意只保留仓库元数据、
README 文件和许可证。

## 特性

* **Android 构建规划** — 在执行前生成有序的构建、安装和同步阶段。
* **Android 部署辅助** — 执行同一套阶段，用于本地 Android 部署。
* **依赖感知的 mod 同步** — 构建并同步当前 mod 及其依赖 mod。
* **CLI 优先工作流** — 提供 `doctor`、`android plan`、`android deploy`、`mod list` 等命令。
* **Dioxus GUI 外壳** — 保留 feature-gated 的桌面 GUI 入口，为后续跨平台工具做准备。

## 如何使用

使用当前开发分支：

```bash
git clone https://github.com/Bli-AIk/prune.git
cd prune
git switch feat/init
```

在 SoupRune 工作区中，工具预期作为 `crates/prune` 子模块使用：

```bash
cargo run -p prune -- android plan --mod mad_dummy_example --device DEVICE123
cargo run -p prune -- android deploy --mod mad_dummy_example --device DEVICE123
```

## 构建方法

### 前置条件

* Rust 1.85 或更高版本
* Android 工作流需要：Android SDK、Android NDK、JDK、Gradle wrapper、`adb`
* Linux 上的 GUI feature 需要：GTK/WebKit 桌面开发包

### 构建步骤

```bash
# 1. 克隆 SoupRune 工作区及子模块
git clone --recurse-submodules https://github.com/Bli-AIk/souprune.git
cd souprune

# 2. 使用 prune 开发分支
git -C crates/prune switch feat/init

# 3. 运行测试
cargo test -p prune

# 4. 检查可选 GUI feature
cargo check -p prune --features gui
```

## 依赖项

核心依赖如下：

| Crate | 版本 | 作用 |
|-------|---------|-------------|
| [clap](https://crates.io/crates/clap) | 4 | 处理命令行参数 |
| [anyhow](https://crates.io/crates/anyhow) | 1 | 方便地处理报错信息 |
| [serde](https://crates.io/crates/serde) | 1 | 解析项目与 mod 元数据 |
| [toml](https://crates.io/crates/toml) | 0.9 | 读取 SoupRune 项目配置文件 |
| [dioxus](https://crates.io/crates/dioxus) | 0.7 | 可选桌面 GUI 外壳 |

## 贡献指南

随时欢迎提交 Issue 或 Pull Request！无论你是想修个 Bug，加个功能，还是只是想改改错别字。

## 许可证

本项目采用双协议开源，你可以自由选择：

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) 或 [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
* MIT license ([LICENSE-MIT](LICENSE-MIT) 或 [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
