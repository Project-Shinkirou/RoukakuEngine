<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/readme-banner-dark.png">
  <img src="docs/assets/readme-banner-light.png" alt="RoukakuEngine —— 原生 2D 视觉小说创作引擎、播放器、编辑器与共享运行时">
</picture>

<div align="center">

# RoukakuEngine

**基于 Rust 的原生 2D 视觉小说创作引擎、播放器、编辑器与共享运行时。**

[![License](https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-f59e0b)](LICENSE-MIT)
[![Status](https://img.shields.io/badge/status-源码公开%20%C2%B7%20开发暂停-64748b)](https://project-shinkirou.github.io/zh-cn/status/)

[English](README.md) · 简体中文 · [日本語](README.ja.md)

[官方门户](https://project-shinkirou.github.io/zh-cn/roukaku/) · [文档](https://project-shinkirou.github.io/zh-cn/roukaku/docs/) · [快速上手](https://project-shinkirou.github.io/zh-cn/roukaku/docs/quick-start/) · [Project-Shinkirou](https://github.com/Project-Shinkirou)

</div>

---

> [!NOTE]
> **项目状态：功能开发已暂停。** RoukakuEngine 不发布预编译安装包，但源码与文档持续公开并可从源码构建；其共享底层运行时库持续为 [ShinkiEMU](https://github.com/Project-Shinkirou/ShinkiEMU) 提供支持。详见[项目状态](https://project-shinkirou.github.io/zh-cn/status/)。

## 核心组件

| 组件 | 职责 |
|---|---|
| **共享运行时** | 场景、任务调度、时钟、输入、文字排版、音视频媒体与资源管理 |
| **RoukakuVN** | 视觉小说模块——将 `.roukaku` 剧本烘焙并交由统一 `VnSession` 执行 |
| **RoukakuPlayer** | 原生运行宿主，负责加载打包作品并管理窗口、设备与平台事件 |
| **RoukakuEditor** | 集成剧本编辑、剧情图、时间轴、属性面板与独立播放器实时预览 |
| **`roukaku` CLI** | 剧本/界面校验、资源烘焙、构包与平台打包命令行工具 |

## 从源码开始

安装 `rust-toolchain.toml` 指定的 Rust 工具链及原生 C/C++ 工具链，按[构建文档](docs/zh-cn/build.md)准备原生依赖后，在仓库根目录执行：

```sh
cargo build --locked -p roukaku-cli -p roukaku-player -p roukaku-crash-reporter
cargo run --locked -p roukaku-cli -- --help
cargo run --locked --manifest-path Editor/Cargo.toml -p roukaku-editor -- Examples/NativeVN/project.yaml
cargo xtask check --workspace engine
cargo xtask check --workspace editor
```

安装命令行工具到 Cargo bin 目录：

```sh
cargo install --locked --path Engine/Source/Programs/roukaku-cli --bin roukaku
roukaku --help
```

按照[快速上手](docs/zh-cn/quick-start.md)可烘焙、打包并运行公开示例工程。

## 文档导航

| 主题 | 简体中文 | English | 日本語 |
|---|---|---|---|
| 概览 | [概览](docs/zh-cn/index.md) | [index](docs/en/index.md) | [概要](docs/ja/index.md) |
| 快速上手 | [快速上手](docs/zh-cn/quick-start.md) | [quick-start](docs/en/quick-start.md) | [クイックスタート](docs/ja/quick-start.md) |
| 构建 | [构建](docs/zh-cn/build.md) | [build](docs/en/build.md) | [ビルド](docs/ja/build.md) |
| 架构 | [架构](docs/zh-cn/architecture.md) | [architecture](docs/en/architecture.md) | [アーキテクチャ](docs/ja/architecture.md) |
| 编辑器 | [编辑器](docs/zh-cn/editor.md) | [editor](docs/en/editor.md) | [エディター](docs/ja/editor.md) |
| VN 创作 | [VN 创作](docs/zh-cn/vn-authoring.md) | [vn-authoring](docs/en/vn-authoring.md) | [VN 制作](docs/ja/vn-authoring.md) |

[参与贡献](CONTRIBUTING.md) · [来源与署名](ORIGINS.md) · [Issues](https://github.com/Project-Shinkirou/RoukakuEngine/issues)

## 与 ShinkiEMU 的关系

[ShinkiEMU](https://github.com/Project-Shinkirou/ShinkiEMU) 是 Project-Shinkirou 旗下用于运行既有视觉小说引擎的姊妹项目，在固定 Git 提交上复用 RoukakuEngine 的共享底层库；RoukakuEngine 本身不依赖 ShinkiEMU。

## 许可

项目自有代码采用 [MIT](LICENSE-MIT) 或 [Apache-2.0](LICENSE-APACHE) 双许可（任选其一）。第三方代码与素材保留原有许可与署名。
