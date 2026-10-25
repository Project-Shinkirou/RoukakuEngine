---
title: RoukakuEngine documentation
description: The roles of RoukakuEngine, RoukakuVN, Player, Editor, and the CLI.
sidebar: { order: 1 }
---

# RoukakuEngine

RoukakuEngine supports native 2D and visual-novel authoring and playback. Its shared runtime owns scenes, tasks, clocks, input, text, media, and resources. RoukakuVN adds story execution, choices, presentation, system pages, and saves.

- **RoukakuPlayer** hosts packaged works and owns windows, devices, and platform events.
- **RoukakuVN** is the visual-novel product module. Authors write `.roukaku` sources; Cook compiles them for the same `VnSession` used by Player and preview.
- **RoukakuEditor** provides text, story graph, timeline, Details, and a separate Player preview. Panel edits modify the same source documents.
- **`roukaku`** is the CLI for script and UI checks, Cook, package creation, and platform bundles.

ShinkiEMU is a separate product for running native legacy engines. It consumes shared RoukakuEngine libraries at a pinned Git revision; RoukakuEngine does not depend on ShinkiEMU.

## Start here

1. [Quick start](quick-start.md): build tools, package the public `Examples/NativeVN` sample, and launch Player.
2. [Build](build.md): Rust and native prerequisites, workspace checks, Android, and macOS cross-compilation.
3. [Architecture and interfaces](architecture.md): crate ownership, budgets, cancellation, and data integrity.
4. [VN authoring](vn-authoring.md): projects, story sources, UI, Luau, localization, and assets.
5. [Editor](editor.md): editing, saving, importing, and product preview.

A platform declared in configuration is not proof of device-tested windows, input, audio, or saves. Use the [build guide](build.md) and current manifests for build entry points.

[中文](../zh-cn/index.md) · [日本語](../ja/index.md)
