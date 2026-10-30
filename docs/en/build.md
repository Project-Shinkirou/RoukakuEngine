---
title: Build
description: Rust, native dependencies, xtask checks, Android, and macOS cross-compilation.
sidebar: { order: 3 }
---

# Build

## Toolchain and workspaces

`rust-toolchain.toml` selects Rust `1.99.1`, `rustfmt`, `clippy`, and a list of Rust targets. Engine crates use Rust edition 2024 and require Rust 1.99 or newer. That target list installs standard libraries; it is not a runtime support matrix. Install the native C/C++ compiler, linker, CMake, and tools required by the selected dependencies. Windows builds use MSVC.

The root workspace contains Engine, RoukakuVN, Player, CLI, and shared libraries. Editor has its own `Editor/Cargo.toml` and lockfile. Run `cargo xtask` from the repository root, selecting `engine`, `editor`, or `all`. Cargo subprocesses use `--locked`.

The dependency baseline uses vcpkg `0c2a9ace1964b818b15f7ae50ba1c42404918a7b`, libass 0.17.5, and optional FFmpeg 9.0.2 port revision 2. The latest released `mlua` 0.12.2 is retained. Its released `mlua-sys` 0.13.0 loader rejects the bytecode emitted by Luau 744 during VM initialization, so the workspace pins the [upstream loader fix](https://github.com/mlua-rs/mlua/blob/ff96ed786295cae96f43ef1372bd22be0fbf7866/mlua-sys/src/luau/compat.rs) instead of downgrading Luau or skipping runtime tests.

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

## Compiled work extensions

Work extensions implement `roukaku_plugin::WorkExtension` and register their Rust
types with `WorkExtensionRegistry::register` in a project's `player` crate.
Install that registry before calling `roukaku_player::run()`. Each session gets
its own instances. Player preflight checks provider, command and schema identity;
an unregistered provider reports `ROUKAKU_EXTENSION_PROVIDER_UNREGISTERED`.
Commands receive typed `ExtensionInvocation` data and return real `SceneCommand`
output, retained per provider across frames. Scalar arguments map to JSON values;
fixed-point arguments use integer millionths.

`Examples/ExtensionPlayer/extension` is a public progress-meter implementation.
Its companion `player` crate links it into the ordinary Player host; its tests
launch a packaged story command and inspect the resulting presentation.

```sh
cargo xtask project-player Examples/ExtensionPlayer --output ./project-player.exe
# Optional --target selects a Cargo target; the required native SDK must be installed.
cargo xtask project-player ./ContentProject --prebuilt ./roukaku-player.exe --output ./content-player.exe
```

`project-player` builds `player/Cargo.toml` with `--locked --release`, reads Cargo's
artifact messages and stages its single executable. A pure-content project has
no `player/Cargo.toml` and reuses `--prebuilt`. Pass the staged executable to the
existing package bundle command's platform Player option. Extension projects
cannot substitute a prebuilt binary.

`WorkExtensionDeclaration` and `ExtensionCommandDeclaration` contain only data
(provider IDs, command schemas and argument schemas). Compiled Editor panels
can share those types without pulling Editor UI dependencies into Player.
There is no runtime library loader, binary descriptor or plugin ABI gate.
The `roukaku-engine` and `roukaku-vn` Rust facades build as ordinary `rlib`
dependencies; redundant Rust `dylib` outputs are removed. Android and Web host
`cdylib` entrypoints remain platform artifacts.
Extensions explicitly return `Transient` or `Restorable` from `restoration()`;
`ExtensionState` supplies capture, candidate validation and restore hooks for
business state. The checkpoint/restore owner integrates those hooks separately.

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
$env:ROUKAKU_TEXT_VCPKG_ROOT = $env:ROUKAKU_SUBTITLE_VCPKG_ROOT
$env:PATH = "$env:ROUKAKU_SUBTITLE_VCPKG_ROOT/installed/x64-windows-static-md-dynbidi/bin;$env:ROUKAKU_SUBTITLE_VCPKG_ROOT/installed/x64-windows-static-md-dynbidi/debug/bin;$env:PATH"
```

After preparing FFmpeg, also apply the printed `VCPKG_ROOT`, `VCPKG_DEFAULT_TRIPLET`, and runtime DLL PATH. For non-Windows targets, apply `PKG_CONFIG_PATH` and `LD_LIBRARY_PATH` or `DYLD_LIBRARY_PATH` as printed. Account for native dependency licenses and dynamic libraries when distributing the product.

Text and subtitles share the pinned native stack. `cargo xtask deps --target <triple> --text` installs Pango 1.58 plus its HarfBuzz/FriBidi/FreeType/Fontconfig dependencies alongside libass 0.17.5 in `.work/vcpkg-subtitle`. Set `ROUKAKU_TEXT_VCPKG_ROOT` to that root, in addition to `ROUKAKU_SUBTITLE_VCPKG_ROOT`; keep its `bin` directory on PATH for dynamic dependencies. A system `pangoft2 >= 1.50` installation can instead be discovered through pkg-config.

Prepared target → vcpkg triplets: Windows MSVC → `x64-windows-static-md-dynbidi`; Linux x64 → `x64-linux`; Android arm64/x64 → `arm64-android-dynbidi` / `x64-android-dynbidi`; macOS arm64/x64 → `arm64-osx` / `x64-osx`; iOS device arm64 → `arm64-ios`; iOS simulator arm64/x64 → `arm64-ios-simulator` / `x64-ios`. Android uses the NDK target compiler and target pkg-config metadata; Apple preparation requires the matching SDK. OHOS and emscripten add their toolchains to this same FT2 bridge.


## Movie playback and synchronization

Platform decoders remain primary: WMF on Windows, AVFoundation on Apple platforms,
GStreamer on Linux, MediaCodec on Android, and WebCodecs on the experimental Web target.
FFmpeg is optional. AVFoundation video uses a pull-driven `AVAssetReader` with PTS,
one look-ahead sample, and one Player-owned pending frame; it never batches a movie
into a `CpuBuffer`. Closing the stream cancels its reader; seeking recreates the
reader and invalidates old presentation work.

Player sends a movie's same-container audio through the selected platform audio
decoder and the existing Kira mixer. Its public media scheduler uses the PCM voice
cursor corrected by rendered-but-unconsumed samples and endpoint output latency.
CPAL endpoints report callback/playback timestamp latency; Android reports Oboe
latency. Silent movies use host time. Pause freezes transport; seek discards the
old output queue and rebuilds both tracks at the target. Device recovery rebuilds
PCM voices at their audible positions. Decoded movie audio obeys the existing PCM
budget. After audio EOS, device-consumed mixer silence clocks any remaining video;
completion follows actual track EOS, never a container duration estimate.

The Windows regression decodes both tracks with WMF, renders real Kira PCM into the
bounded native output queue, and consumes it with a controlled software callback:

```sh
cargo test --locked -p roukaku-audio-kira --test native_movie_sync -- --nocapture
cargo test --locked -p roukaku-media --test container_tracks
```

It asserts video PTS versus audio progress across pause, seek, and output rebuild
against the scheduler's 100 ms lag / 20 ms lead limits. It does not open a physical
audio device. Apple's reader/PTS/seek/cancel regression requires Apple SDKs; CI
`products` runs it on `macos-15` and `macos-15-intel`.

`av-clock.mp4`, `silent.mp4`, and `silent.webm` in `roukaku-media/tests/fixtures`
contain generated patterns/tones and are dedicated to CC0, with no downloaded media.
Regenerate the A/V sample with:

```sh
ffmpeg -f lavfi -i testsrc2=size=64x64:rate=25:duration=3 -f lavfi -i sine=frequency=440:sample_rate=48000:duration=1.8 -c:v libx264 -pix_fmt yuv420p -bf 0 -g 25 -c:a aac -ac 2 -b:a 32k -movflags +faststart av-clock.mp4
```


## Android Player

Use `cargo xtask android`. Install JDK 27, `cargo-ndk`, and the Rust `aarch64-linux-android` target. Set `ANDROID_SDK_ROOT` or `ANDROID_HOME`. Install SDK platform `platforms;android-37.2`, Build Tools `37.0.0`, and stable [NDK r30](https://developer.android.com/ndk/downloads) `30.0.16248370`. The project uses AGP 9.4.1 and Gradle 9.8.2. JNI libraries retain 16 KiB ELF page alignment; uncompressed APK native libraries use modern zip alignment.

The build JVM stays on JDK 27: [Gradle 9.8.2 explicitly supports Java 27](https://docs.gradle.org/9.8.2/userguide/compatibility.html), and [Android's JDK guidance](https://developer.android.com/build/jdks) requires a build JDK at least as new as AGP's minimum (JDK 17 for AGP 9.4). This selects the JVM running the build, not the Android app's Java API or bytecode level.

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
rustup toolchain install 1.99.1 --profile minimal
rustup target add --toolchain 1.99.1 x86_64-apple-darwin aarch64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
cargo +1.99.1 install --locked --root "$HOME/shinkirou-apple/tools" cross --version 0.2.5
cargo +1.99.1 install --locked --root "$HOME/shinkirou-apple/tools" apple-codesign --version 0.29.0
export SHINKI_APPLE_SDK_DIR="$HOME/shinkirou-apple/sdks/macosx.sdk"
cargo xtask apple-image --target aarch64-apple-darwin
cargo xtask build --workspace engine --target aarch64-apple-darwin -p roukaku-player --release
cargo xtask cross --target aarch64-apple-darwin check -- -p roukaku-core
```

Build one image per target, switching SDK directories for iOS. Images use Rust 1.99.1, pinned [osxcross](https://github.com/tpoechtrager/osxcross) for macOS, LLVM/LLD for iOS, target-scoped C/C++/CMake tools and static libass from the pinned vcpkg input in `build/cross/`. Host tools keep their Linux compiler. Native licenses are exported to `.work/apple-native/<target>` and reused only for the same image identity. Build JSON includes Cargo's actual `compiler-artifact.executable` paths; packaging takes that path, not a guessed target directory.

### Player resources and test bundles

The shared Apple factory uses winit/Metal, AVFoundation decoding, CoreAudio playback with an active iOS audio session, AccessKit semantics, touch/IME input, atomic sandbox saves and the native Files picker with persistent authorization. UIKit owns the iOS main-thread event loop; the VM future runs on its worker, rather than pumping a desktop event loop. iOS does not use the desktop gilrs gamepad backend.

Cook a package containing an `ios-release` profile (`PlatformHostProfile::ios_release`) and create flat resources with `roukaku package bundle --platform ios --ios-player <built-player>`, supplying the usual package, target, profile and output arguments. Then package the actual Cargo executable and that resource directory:

```sh
cargo xtask package --target aarch64-apple-ios --binary <compiler-artifact-executable> --resources .work/ios-resources --version 0.1.0-beta.1 --out .work/ios-player-test
```

macOS output is an ad-hoc-signed `.app` and ZIP; simulator output is a clearly named test `.app` and ZIP; device output is an `adhoc-device-test.ipa` with `Payload/*.app`. Packaging validates Mach-O architecture, device/simulator platform and dependency closure before rewriting or signing copied files. Linux signs using `rcodesign` and verifies each Mach-O; native macOS uses `codesign`. Ad-hoc signing provides no provisioning profile, notarization or App Store identity. Device installation still needs valid Apple signing/provisioning; no production-installable IPA is claimed. Native device/simulator lifecycle and runtime conformance remain separate from compilation.

For a built artifact, retain `file`, `llvm-objdump --macho --dylibs-used --private-headers <executable>` and `rcodesign verify <executable>` output alongside its actual path and size. SDK-backed builds and signatures cannot be verified when the legal SDK input is absent.


Return to the [documentation index](index.md) or continue with [Quick start](quick-start.md).
