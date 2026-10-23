---
title: Quick start
description: Build the CLI, cook the public NativeVN sample, package it, and launch Player.
sidebar: { order: 2 }
---

# Quick start

Run these commands from the repository root. Prepare Rust, a C/C++ toolchain, and native dependencies using [Build](build.md), and fetch the repository's Git LFS assets. The sample uses committed authoring sources directly; no generation script is required.

## Build the tools

```sh
cargo build --locked -p roukaku-cli -p roukaku-player -p roukaku-crash-reporter
cargo run --locked -p roukaku-cli -- --help
```

The Cargo package is `roukaku-cli`; its executable is `roukaku`. These examples use `cargo run`, so installation into PATH is unnecessary.

## Cook and package

The public sample is `Examples/NativeVN`, with `project.yaml` as its entry point. Use the target and profile declared there:

```sh
cargo run --locked -p roukaku-cli -- cook Examples/NativeVN/project.yaml --profile advanced-vn --target nativevn-flagship-game --out .work/nativevn-cooked
cargo run --locked -p roukaku-cli -- package build .work/nativevn-cooked --target nativevn-flagship-game --out .work/nativevn.roukakupkg
```

Cook compiles story and UI sources and collects localization, asset sidecars, and project sections. `package build` verifies cooked artifact hashes before writing the container. Choose fresh output paths for each run to preserve existing artifacts.

## Windows Player preview

In PowerShell, set `$env:VC_CRT_REDIST` to the redistributable Microsoft VC x64 CRT directory matching the build toolchain. It must contain `msvcp140.dll`, `vcruntime140.dll`, and `vcruntime140_1.dll`. Use redistributable release DLLs, not debug DLLs.

```powershell
cargo run --locked -p roukaku-cli -- package bundle .work/nativevn.roukakupkg --out .work/nativevn-preview --target nativevn-flagship-game --profile advanced-vn --platform windows --windows-player target/debug/roukaku-player.exe --windows-runtime "$env:VC_CRT_REDIST" --crash-reporter target/debug/RoukakuCrashReporter.exe
.work/nativevn-preview/RoukakuPlayer.exe
```

Launch the bundled Player. It reads the adjacent `RoukakuPlayer.config.json` and opens a separate window. The raw `target/debug/roukaku-player.exe` needs bundle configuration; it does not take a project source path.

Use physical input and the sample UI. During text reveal, the first advance reveals the complete line; the next continues the story. Check rendered output, audible audio, choices, and saves. Successful Cook or compilation does not establish runtime correctness.

On Linux, use the same Cook and package commands, then bundle with `--platform linux --linux-player target/debug/roukaku-player`, without Windows CRT or crash-reporter arguments. Run `roukaku-player` in the output directory. On macOS, use `--platform macos --macos-player target/debug/roukaku-player`; the bundled entry point is `Contents/MacOS/roukaku-player`. Each platform needs the native providers selected by the project.

## Preview from Editor

```sh
cargo run --locked --manifest-path Editor/Cargo.toml -p roukaku-editor -- Examples/NativeVN/project.yaml
```

Load a local tool configuration using Preview setup, then select Save & Preview. This runs the same Cook, package, bundle, and Player pipeline. Pause, resume, and stop operate on the real Player after its handshake. See [Editor](editor.md) for configuration fields and editing capabilities.

Continue with [VN authoring](vn-authoring.md) or [Architecture](architecture.md).
