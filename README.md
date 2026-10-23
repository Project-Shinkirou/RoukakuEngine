---
title: RoukakuEngine
description: Native 2D and visual-novel authoring, playback, and tools.
---

# RoukakuEngine

RoukakuEngine 提供共享 2D 运行层、RoukakuVN 视觉小说模块、RoukakuPlayer、RoukakuEditor 和命令行工具 `roukaku`。ShinkiEMU 是独立产品，通过固定 Git revision 使用共享底层库。

RoukakuEngine provides a shared 2D runtime, the RoukakuVN visual-novel module, RoukakuPlayer, RoukakuEditor, and the `roukaku` CLI. ShinkiEMU is a separate product consuming shared libraries at a pinned Git revision.

## 开始使用 / Get started

安装 Rust（工具链由 `rust-toolchain.toml` 选择）与本机 C/C++ 工具链，先按[构建说明](docs/zh-cn/build.md)准备原生依赖。Install Rust and a native C/C++ toolchain, then prepare native dependencies using the [build guide](docs/en/build.md).

在仓库根目录运行 / From the repository root:

```sh
cargo build --locked -p roukaku-cli -p roukaku-player -p roukaku-crash-reporter
cargo run --locked -p roukaku-cli -- --help
cargo run --locked --manifest-path Editor/Cargo.toml -p roukaku-editor -- Examples/NativeVN/project.yaml
cargo xtask check --workspace engine
cargo xtask check --workspace editor
```

将 CLI 安装到 Cargo bin 目录 / Install the CLI into Cargo's bin directory:

```sh
cargo install --locked --path Engine/Source/Programs/roukaku-cli --bin roukaku
roukaku --help
```

公开示例的 Cook、打包和 Player 启动步骤见[快速开始](docs/zh-cn/quick-start.md) / See [Quick start](docs/en/quick-start.md) to cook, package, and play the public sample.

## 文档 / Documentation

- [中文文档](docs/zh-cn/index.md) · [English documentation](docs/en/index.md)
- [参与贡献 / Contributing](CONTRIBUTING.md)
- [来源与归属 / Origins and attribution](ORIGINS.md)

## 许可证 / License

自有代码采用 **MIT OR Apache-2.0**，可任选其一，见 [LICENSE-MIT](LICENSE-MIT) 和 [LICENSE-APACHE](LICENSE-APACHE)。第三方代码与素材保留各自许可证和归属。

Project-owned code is available under **MIT OR Apache-2.0**, at your option. Third-party code and assets retain their own licenses and attribution.
