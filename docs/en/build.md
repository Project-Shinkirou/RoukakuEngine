---
title: Build
description: Rust, native dependencies, xtask checks, Android, and macOS cross-compilation.
sidebar: { order: 3 }
---

# Build

## Toolchain and workspaces

`rust-toolchain.toml` selects Rust `stable`, `rustfmt`, `clippy`, and a list of Rust targets. That list installs standard libraries; it is not a runtime support matrix. Install the native C/C++ compiler, linker, CMake, and tools required by the selected dependencies. Windows builds use MSVC.

The root workspace contains Engine, RoukakuVN, Player, CLI, and shared libraries. Editor has its own `Editor/Cargo.toml` and lockfile. Run `cargo xtask` from the repository root, selecting `engine`, `editor`, or `all`. Cargo subprocesses use `--locked`.

```sh
cargo xtask docs
cargo xtask check --workspace engine
cargo xtask check --workspace editor
cargo xtask build --workspace engine -p roukaku-cli -p roukaku-player
cargo xtask test --workspace engine -p roukaku-runtime
cargo xtask fmt --workspace all
cargo xtask lint --workspace editor
cargo xtask ui-check
```

`check` runs documentation, formatting, Clippy, build, and tests. `fmt` checks formatting without rewriting files; `lint` rejects Clippy warnings. An unfiltered `test` builds product executables first for real CLI/host tests; a targeted `-p` test does not perform that prebuild. Package selection requires an explicit workspace.

`check-fast` checks documentation links and hygiene only; it does not compile or test code. `docs` runs the same documentation check. `ui-check` explicitly downloads a pinned Luau analyzer and checks NativeVN controllers, requiring network access.

## Native dependencies

Dependency preparation is explicit. Ordinary builds do not install system packages. These commands use a pinned vcpkg revision to prepare FFmpeg and libass subtitle dependencies as selected:

```sh
cargo xtask deps --target x86_64-pc-windows-msvc --subtitle
cargo xtask deps --target x86_64-pc-windows-msvc --ffmpeg --subtitle
```

Without either selection flag, nothing is downloaded. Whether FFmpeg is needed depends on the crates/features and decode provider being built. Explicit WMF, GStreamer, or AVFoundation bindings still require their native environments; preparing FFmpeg does not replace those bindings.

The tool uses `.work/vcpkg` and a separate `.work/vcpkg-subtitle` installation. It prints environment settings and writes GitHub Actions environment files when available. Apply the printed settings yourself for subsequent local commands. Windows PowerShell subtitle setup:

```powershell
$env:ROUKAKU_SUBTITLE_VCPKG_ROOT = (Resolve-Path .work/vcpkg-subtitle).Path
$env:PATH = "$env:ROUKAKU_SUBTITLE_VCPKG_ROOT/installed/x64-windows-static-md-dynbidi/bin;$env:ROUKAKU_SUBTITLE_VCPKG_ROOT/installed/x64-windows-static-md-dynbidi/debug/bin;$env:PATH"
```

After preparing FFmpeg, also apply the printed `VCPKG_ROOT`, `VCPKG_DEFAULT_TRIPLET`, and runtime DLL PATH. For non-Windows targets, apply `PKG_CONFIG_PATH` and `LD_LIBRARY_PATH` or `DYLD_LIBRARY_PATH` as printed. Account for native dependency licenses and dynamic libraries when distributing the product.

## Android Player

Use `cargo xtask android`. Install JDK 17, `cargo-ndk`, and the Rust `aarch64-linux-android` target. Set `ANDROID_SDK_ROOT` or `ANDROID_HOME`. The SDK must contain Android 36, Build Tools 36.0.0, and NDK 30.0.15729638.

First create a `.roukakupkg` with an Android host profile using [Quick start](quick-start.md), then run:

```sh
cargo xtask android --package .work/nativevn.roukakupkg --target nativevn-flagship-game --application-id org.shinkirou.roukakuplayer --output .work/nativevn-android
```

Here `--target` is the product target ID, not a Rust triple. The current command builds arm64-v8a JNI, invokes the project's Gradle wrapper, and collects real APK/AAB files, JNI libraries, and a hash manifest. The output directory must be new. `--profile` selects the Cargo profile and defaults to `release`. Supply local signing configuration with `--signing-properties`. Without it, the debug APK is debug-signed and release output is marked unsigned, not a production installable release. Never commit signing files.

Android requires its matching platform profile; renaming a desktop package does not add Android support. Check touch, Back, foreground/background transitions, audio, and saves on a device.

## macOS cross-checks

Use an existing osxcross installation and a legally obtained macOS SDK. Set `ROUKAKU_OSXCROSS_ROOT`, set `SDKROOT` explicitly when SDK selection is ambiguous, and install the relevant Rust target first:

```sh
cargo xtask cross --target aarch64-apple-darwin check -- -p roukaku-core
cargo xtask cross --target x86_64-apple-darwin clippy -- -p roukaku-core -- -D warnings
```

Only those two macOS targets and `check|clippy` are accepted. The command validates the compiler, archiver, and SDK, isolates output under `.work/cross/<target>`, and sets the deployment target to macOS 13.0. It does not download SDKs, sign applications, or replace native macOS runtime checks.

Return to the [documentation index](index.md) or continue with [Quick start](quick-start.md).
