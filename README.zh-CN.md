# RoukakuEngine

用来创作和运行原生 2D 作品与视觉小说的引擎。

[English](README.md) · 简体中文 · [日本語](README.ja.md)

> **状态：暂停开发。** RoukakuEngine 没有发布二进制版本。源码和文档保持公开，可以按下面的步骤从源码构建。详见[项目状态](https://project-shinkirou.github.io/zh-cn/status/)页面。

**门户：**[RoukakuEngine](https://project-shinkirou.github.io/zh-cn/roukaku/) · [文档](https://project-shinkirou.github.io/zh-cn/roukaku/docs/) · [快速开始](https://project-shinkirou.github.io/zh-cn/roukaku/docs/quick-start/)

## 组成

- **共享运行层**：场景、任务、时钟、输入、文字、媒体和资源管理。
- **RoukakuVN**：视觉小说模块。作者编写 `.roukaku` 源码，经 Cook 编译后由同一套 `VnSession` 执行。
- **RoukakuPlayer**：作品运行宿主，加载打包数据，管理窗口、设备和平台事件。
- **RoukakuEditor**：文本、剧情图、时间线、Details 和独立 Player 预览，面板里的编辑会回写同一份源码。
- **`roukaku` 命令行工具**：脚本与 UI 检查、Cook、包构建和平台 bundle。

配置中声明了某个平台，不等于该平台的窗口、输入、声音和存读档已经通过设备验证。

## 开始使用

安装 Rust（工具链由 `rust-toolchain.toml` 选择）和本机 C/C++ 工具链，按[构建说明](docs/zh-cn/build.md)准备原生依赖，然后在仓库根目录运行：

```sh
cargo build --locked -p roukaku-cli -p roukaku-player -p roukaku-crash-reporter
cargo run --locked -p roukaku-cli -- --help
cargo run --locked --manifest-path Editor/Cargo.toml -p roukaku-editor -- Examples/NativeVN/project.yaml
cargo xtask check --workspace engine
cargo xtask check --workspace editor
```

把 CLI 安装到 Cargo 的 bin 目录：

```sh
cargo install --locked --path Engine/Source/Programs/roukaku-cli --bin roukaku
roukaku --help
```

公开示例的 Cook、打包和 Player 启动步骤见[快速开始](docs/zh-cn/quick-start.md)。

## 文档

| | 简体中文 | English | 日本語 |
|---|---|---|---|
| 概览 | [index](docs/zh-cn/index.md) | [index](docs/en/index.md) | [index](docs/ja/index.md) |
| 快速开始 | [quick-start](docs/zh-cn/quick-start.md) | [quick-start](docs/en/quick-start.md) | [quick-start](docs/ja/quick-start.md) |
| 构建 | [build](docs/zh-cn/build.md) | [build](docs/en/build.md) | [build](docs/ja/build.md) |
| 架构与接口 | [architecture](docs/zh-cn/architecture.md) | [architecture](docs/en/architecture.md) | [architecture](docs/ja/architecture.md) |
| Editor | [editor](docs/zh-cn/editor.md) | [editor](docs/en/editor.md) | [editor](docs/ja/editor.md) |
| VN 创作 | [vn-authoring](docs/zh-cn/vn-authoring.md) | [vn-authoring](docs/en/vn-authoring.md) | [vn-authoring](docs/ja/vn-authoring.md) |

[参与贡献](CONTRIBUTING.md) · [来源与归属](ORIGINS.md) · [问题反馈](https://github.com/Project-Shinkirou/RoukakuEngine/issues)

## 与 ShinkiEMU 的关系

[ShinkiEMU](https://github.com/Project-Shinkirou/ShinkiEMU) 是运行原生旧视觉小说引擎的独立产品，按固定 Git revision 使用 RoukakuEngine 的共享库；RoukakuEngine 不依赖 ShinkiEMU。

## 许可证

自有代码采用 [MIT](LICENSE-MIT) 或 [Apache-2.0](LICENSE-APACHE)，可任选其一。第三方代码与素材保留各自的许可证和归属。
