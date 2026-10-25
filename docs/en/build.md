---
title: Build
description: Rust, native dependencies, xtask checks, Android, and macOS cross-compilation.
sidebar: { order: 3 }
---

# Build

## Toolchain and workspaces

`rust-toolchain.toml` selects Rust `1.98.0`, `rustfmt`, `clippy`, and a list of Rust targets. That list installs standard libraries; it is not a runtime support matrix. Install the native C/C++ compiler, linker, CMake, and tools required by the selected dependencies. Windows builds use MSVC.

The root workspace contains Engine, RoukakuVN, Player, CLI, and shared libraries. Editor has its own `Editor/Cargo.toml` and lockfile. Run `cargo xtask` from the repository root, selecting `engine`, `editor`, or `all`. Cargo subprocesses use `--locked`.

Engine rendering uses wgpu 30.0.x, matching ShinkiEMU and Slint 1.18.1's `unstable-wgpu-30` backend. Windows keeps the `static-dxc` feature. Presentation uses the submitting queue and the default `Auto` surface color space, preserving SDR output. Editor currently does not consume wgpu through Engine crates; its GPUI renderer and independent lockfile remain unchanged.

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

Tests that open real windows/GPU surfaces or use a physical audio output device are ignored by default, including on hosted CI runners. Their assertions and native backends remain unchanged. On a development machine with the required devices, opt in with `--hardware` on `test` or `check`:

```sh
cargo xtask test --workspace engine --hardware -p roukaku-platform-windows
cargo xtask check --workspace engine --hardware
```

The flag passes `--include-ignored` to the test harness, so it also runs previously ignored tests. Install any additional platform codecs and provide fixtures required by those tests. Tests gated by `platform-test-driver` still require `--features platform-test-driver`; input-driver tests can focus a real window and send input. CI omits `--hardware`.

To exercise the native GPU scene, WGSL filter, texture upload and readback without opening a window, run:

```sh
cargo test --locked -p roukaku-platform-common --test offscreen native_offscreen_gpu_renders_scene_filter_and_readback -- --ignored --nocapture
```

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

## Apple builds from Linux

Native macOS CI stays on `macos-15` / `macos-15-intel` and uses Cargo and Xcode. On Linux, Apple target builds use cross 0.2.5 with private images in `Cross.toml`. macOS requires 13.0 or later; iOS requires 16.0 or later. Targets are `x86_64-apple-darwin`, `aarch64-apple-darwin`, `aarch64-apple-ios` (device), `aarch64-apple-ios-sim` and `x86_64-apple-ios` (simulators).

### Obtain the SDKs

Obtain Xcode from the [Mac App Store](https://apps.apple.com/app/xcode/id497799835) or [Apple Developer Downloads](https://developer.apple.com/download/all/) under Apple's license. Expand an official `.xip` with macOS's `xip --expand`, select that Xcode installation, and accept its license on that Mac. Command Line Tools alone do not include the iOS SDKs. On the Mac, export SDK contents while preserving their internal symlinks:

```sh
for sdk in macosx iphoneos iphonesimulator; do
  tar -C "$(xcrun --sdk "$sdk" --show-sdk-path)" -cJf "$sdk.sdk.tar.xz" .
done
```

Transfer these archives privately to the Linux host and extract each into its own directory. `SHINKI_APPLE_SDK_DIR` points directly to the extracted `.sdk` contents, containing `SDKSettings.json`, `usr/` and `System/`. Match `macosx` to Darwin, `iphoneos` to the device and `iphonesimulator` to both simulator triples. Neither xtask nor the Dockerfile downloads an Apple SDK. Keep SDK archives and SDK-containing images out of public registries and Git.

### Prepare and build

Keep the checkout, caches and tools under a dedicated directory with sufficient space; image creation refuses less than 8 GiB free on `/`. Do not delete unrelated caches or Docker images. Example Linux setup:

Docker must provide Buildx/BuildKit (`docker buildx version`). For a task-local plugin, use `DOCKER_CONFIG=~/shinkirou-apple/docker` and its `cli-plugins/docker-buildx` path. The official [Buildx v0.38.1 Linux x86_64 asset](https://github.com/docker/buildx/releases/download/v0.38.1/buildx-v0.38.1.linux-amd64) has SHA256 `e25d5cd675092bb5d1d03632fdc46cdf3fede7b514dc5f023a105e00d3ce5ad4`; verify it before making it executable. No sudo or changes to an existing Docker configuration are needed.

```sh
export CARGO_HOME="$HOME/shinkirou-apple/cargo"
export RUSTUP_HOME="$HOME/shinkirou-apple/rustup"
export CARGO_TARGET_DIR="$HOME/shinkirou-apple/target"
export PATH="$HOME/shinkirou-apple/tools/bin:$PATH"
export DOCKER_CONFIG="$HOME/shinkirou-apple/docker"
rustup toolchain install 1.98.0 --profile minimal
rustup target add --toolchain 1.98.0 x86_64-apple-darwin aarch64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
cargo +1.98.0 install --locked --root "$HOME/shinkirou-apple/tools" cross --version 0.2.5
cargo +1.98.0 install --locked --root "$HOME/shinkirou-apple/tools" apple-codesign --version 0.29.0
export SHINKI_APPLE_SDK_DIR="$HOME/shinkirou-apple/sdks/macosx.sdk"
cargo xtask apple-image --target aarch64-apple-darwin
cargo xtask build --workspace engine --target aarch64-apple-darwin -p roukaku-player --release
cargo xtask cross --target aarch64-apple-darwin check -- -p roukaku-core
```

Build one image per target, switching SDK directories for iOS. Images use Rust 1.98.0, pinned [osxcross](https://github.com/tpoechtrager/osxcross) for macOS, LLVM/LLD for iOS, target-scoped C/C++/CMake tools and static libass from the pinned vcpkg input in `build/cross/`. Host tools keep their Linux compiler. Native licenses are exported to `.work/apple-native/<target>` and reused only for the same image identity. Build JSON includes Cargo's actual `compiler-artifact.executable` paths; packaging takes that path, not a guessed target directory.

### Player resources and test bundles

The shared Apple factory uses winit/Metal, AVFoundation decoding, CoreAudio playback with an active iOS audio session, AccessKit semantics, touch/IME input, atomic sandbox saves and the native Files picker with persistent authorization. UIKit owns the iOS main-thread event loop; the VM future runs on its worker, rather than pumping a desktop event loop. iOS does not use the desktop gilrs gamepad backend.

Cook a package containing an `ios-release` profile (`PlatformHostProfile::ios_release`) and create flat resources with `roukaku package bundle --platform ios --ios-player <built-player>`, supplying the usual package, target, profile and output arguments. Then package the actual Cargo executable and that resource directory:

```sh
cargo xtask package --target aarch64-apple-ios --binary <compiler-artifact-executable> --resources .work/ios-resources --version 0.1.0-beta.1 --out .work/ios-player-test
```

macOS output is an ad-hoc-signed `.app` and ZIP; simulator output is a clearly named test `.app` and ZIP; device output is an `adhoc-device-test.ipa` with `Payload/*.app`. Packaging validates Mach-O architecture, device/simulator platform and dependency closure before rewriting or signing copied files. Linux signs using `rcodesign` and verifies each Mach-O; native macOS uses `codesign`. Ad-hoc signing provides no provisioning profile, notarization or App Store identity. Device installation still needs valid Apple signing/provisioning; no production-installable IPA is claimed. Native device/simulator lifecycle and runtime conformance remain separate from compilation.

For a built artifact, retain `file`, `llvm-objdump --macho --dylibs-used --private-headers <executable>` and `rcodesign verify <executable>` output alongside its actual path and size. SDK-backed builds and signatures cannot be verified when the legal SDK input is absent.


Return to the [documentation index](index.md) or continue with [Quick start](quick-start.md).
