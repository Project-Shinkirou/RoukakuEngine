# RoukakuEngine

An engine for making and running native 2D works and visual novels.

English · [简体中文](README.zh-CN.md) · [日本語](README.ja.md)

> **Status: development is paused.** RoukakuEngine has no binary releases. The source and documentation stay public, and you can build it from source with the steps below. See the [project status](https://project-shinkirou.github.io/en/status/) page.

**Portal:** [RoukakuEngine](https://project-shinkirou.github.io/en/roukaku/) · [Documentation](https://project-shinkirou.github.io/en/roukaku/docs/) · [Quick start](https://project-shinkirou.github.io/en/roukaku/docs/quick-start/)

## Parts

- **Shared runtime**: scenes, tasks, clocks, input, text, media and resource management.
- **RoukakuVN**: the visual novel module. Authors write `.roukaku` sources, which are cooked and then executed by the same `VnSession`.
- **RoukakuPlayer**: the runtime host. It loads packaged data and manages windows, devices and platform events.
- **RoukakuEditor**: text, story graph, timeline, Details and a standalone Player preview; panel edits are written back to the same source.
- **`roukaku` CLI**: script and UI checks, cooking, package builds and platform bundles.

A platform declared in the configuration has not necessarily had its windowing, input, audio and save/load verified on a device.

## Get started

Install Rust (the toolchain is selected by `rust-toolchain.toml`) and a native C/C++ toolchain, then prepare native dependencies with the [build guide](docs/en/build.md). From the repository root:

```sh
cargo build --locked -p roukaku-cli -p roukaku-player -p roukaku-crash-reporter
cargo run --locked -p roukaku-cli -- --help
cargo run --locked --manifest-path Editor/Cargo.toml -p roukaku-editor -- Examples/NativeVN/project.yaml
cargo xtask check --workspace engine
cargo xtask check --workspace editor
```

Install the CLI into Cargo's bin directory:

```sh
cargo install --locked --path Engine/Source/Programs/roukaku-cli --bin roukaku
roukaku --help
```

See [Quick start](docs/en/quick-start.md) to cook, package and play the public sample.

## Documentation

| | English | 简体中文 | 日本語 |
|---|---|---|---|
| Overview | [index](docs/en/index.md) | [index](docs/zh-cn/index.md) | [index](docs/ja/index.md) |
| Quick start | [quick-start](docs/en/quick-start.md) | [quick-start](docs/zh-cn/quick-start.md) | [quick-start](docs/ja/quick-start.md) |
| Build | [build](docs/en/build.md) | [build](docs/zh-cn/build.md) | [build](docs/ja/build.md) |
| Architecture | [architecture](docs/en/architecture.md) | [architecture](docs/zh-cn/architecture.md) | [architecture](docs/ja/architecture.md) |
| Editor | [editor](docs/en/editor.md) | [editor](docs/zh-cn/editor.md) | [editor](docs/ja/editor.md) |
| VN authoring | [vn-authoring](docs/en/vn-authoring.md) | [vn-authoring](docs/zh-cn/vn-authoring.md) | [vn-authoring](docs/ja/vn-authoring.md) |

[Contributing](CONTRIBUTING.md) · [Origins and attribution](ORIGINS.md) · [Issues](https://github.com/Project-Shinkirou/RoukakuEngine/issues)

## Relation to ShinkiEMU

[ShinkiEMU](https://github.com/Project-Shinkirou/ShinkiEMU) is a separate product for running original visual novel engines. It uses RoukakuEngine's shared libraries at a pinned Git revision; RoukakuEngine does not depend on ShinkiEMU.

## License

Project-owned code is available under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. Third-party code and assets keep their own licenses and attribution.
