---
title: 构建
description: Rust 工具链、原生依赖、xtask 检查及 Android 和 macOS 构建入口。
sidebar: { order: 3 }
---

# 构建

## 工具链与 workspace

`rust-toolchain.toml` 选择 Rust `1.98.0`，并声明 `rustfmt`、`clippy` 和目标列表。这个列表用于安装 Rust 标准库，不是平台运行支持矩阵。安装本机 C/C++ 编译器、链接器、CMake，以及所选原生依赖需要的工具；Windows 使用 MSVC 工具链。

根 workspace 包含 Engine、RoukakuVN、Player、CLI 和共享库。Editor 使用 `Editor/Cargo.toml` 及自己的 lockfile。根目录的 `cargo xtask` 统一选择 `engine`、`editor` 或 `all`，底层 Cargo 命令使用 `--locked`。

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

`check` 依次检查文档、格式、Clippy、构建和测试。`fmt` 是格式检查，不改写文件；`lint` 执行 Clippy 并拒绝 warning。`test` 未指定 package 时先构建产品，供真实 CLI/宿主测试使用；定向 `-p` 不执行这项预构建。`-p` 必须配合明确的 workspace。

需要真实窗口、GPU surface 或音频输出设备的测试默认忽略，托管 CI 也不运行这些测试；测试断言和原生后端保持不变。在具备相应设备的开发机上，给 `test` 或 `check` 加上 `--hardware`：

```sh
cargo xtask test --workspace engine --hardware -p roukaku-platform-windows
cargo xtask check --workspace engine --hardware
```

此参数向测试 harness 传入 `--include-ignored`，因此也会运行此前已忽略的测试。按各测试要求准备平台 codec 和 fixture。由 `platform-test-driver` 控制的测试仍需 `--features platform-test-driver`；输入驱动测试会聚焦真实窗口并发送输入。CI 不传 `--hardware`。

`check-fast` 只检查文档链接和文本卫生，不编译或运行测试。`docs` 使用同一文档检查。`ui-check` 显式下载锁定的 Luau analyzer 并检查 NativeVN controller；它需要联网。

## 原生依赖

依赖准备是显式操作，普通构建不代替你安装系统包。以下入口使用固定 vcpkg revision，按需准备 FFmpeg 和 libass 字幕依赖：

```sh
cargo xtask deps --target x86_64-pc-windows-msvc --subtitle
cargo xtask deps --target x86_64-pc-windows-msvc --ffmpeg --subtitle
```

不带 `--ffmpeg` 或 `--subtitle` 时不会下载。FFmpeg 是否需要取决于实际编译的 crate/features 和解码 provider；项目明确绑定的 WMF、GStreamer、AVFoundation 等仍需各自原生环境，不会因准备 FFmpeg 而自动替换。

工具将 vcpkg 放在 `.work/vcpkg`，字幕使用独立的 `.work/vcpkg-subtitle`。它打印环境设置并在 GitHub Actions 写入环境文件；本地后续命令要自行应用这些设置。Windows PowerShell 的字幕设置为：

```powershell
$env:ROUKAKU_SUBTITLE_VCPKG_ROOT = (Resolve-Path .work/vcpkg-subtitle).Path
$env:PATH = "$env:ROUKAKU_SUBTITLE_VCPKG_ROOT/installed/x64-windows-static-md-dynbidi/bin;$env:ROUKAKU_SUBTITLE_VCPKG_ROOT/installed/x64-windows-static-md-dynbidi/debug/bin;$env:PATH"
```

准备 FFmpeg 后还要设置打印的 `VCPKG_ROOT`、`VCPKG_DEFAULT_TRIPLET` 和运行库 PATH。非 Windows 目标按输出设置 `PKG_CONFIG_PATH` 及 `LD_LIBRARY_PATH` 或 `DYLD_LIBRARY_PATH`。原生依赖的分发许可与动态库仍需随最终产品处理。

## Android Player

入口是 `cargo xtask android`。需要 JDK 17、`cargo-ndk`、Rust `aarch64-linux-android` target，设置 `ANDROID_SDK_ROOT` 或 `ANDROID_HOME`。SDK 需包含 Android 36、Build Tools 36.0.0 和 NDK 30.0.15729638。

先按[快速开始](quick-start.md)生成含 Android host profile 的 `.roukakupkg`，再运行：

```sh
cargo xtask android --package .work/nativevn.roukakupkg --target nativevn-flagship-game --application-id org.shinkirou.roukakuplayer --output .work/nativevn-android
```

这里的 `--target` 是产品 target ID，不是 Rust triple。当前入口构建 arm64-v8a JNI 库，运行工程内 Gradle wrapper 并收集真实 APK/AAB、JNI 库及 hash 清单；输出目录必须是新目录。`--profile` 选择 Cargo profile，默认 `release`。`--signing-properties` 可传入本地签名配置；没有它时 debug APK 使用调试签名，release 产物标为未签名，不能当作正式安装包。不要提交签名文件。

Android 包必须有对应平台 profile，不能靠重命名桌面包获得 Android 支持。触控、返回键、前后台、音频和存读档需要真机检查。

## macOS 交叉检查

在已有 osxcross 和合法 macOS SDK 的环境中，设置 `ROUKAKU_OSXCROSS_ROOT`。SDK 不唯一时显式设置 `SDKROOT`，并预先安装目标 Rust 标准库：

```sh
cargo xtask cross --target aarch64-apple-darwin check -- -p roukaku-core
cargo xtask cross --target x86_64-apple-darwin clippy -- -p roukaku-core -- -D warnings
```

入口只接受这两个 macOS target 和 `check|clippy`，检查对应编译器、archiver 与 SDK，将结果隔离到 `.work/cross/<target>`，部署目标为 macOS 13.0。它不下载 SDK、不生成签名应用，也不替代原生 macOS 运行检查。

回到[文档首页](index.md)或继续[快速开始](quick-start.md)。
