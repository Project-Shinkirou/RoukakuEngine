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

## 编译集成作品扩展

作品扩展实现 `roukaku_plugin::WorkExtension`，在工程的 `player` crate 中通过
`WorkExtensionRegistry::register` 注册 Rust 类型。调用 `roukaku_player::run()`
前安装注册表，每个会话独立创建扩展实例。Player 启动时检查 provider、命令和 schema；
未注册的 provider 报 `ROUKAKU_EXTENSION_PROVIDER_UNREGISTERED`。
命令通过 `ExtensionInvocation` 接收参数，输出真实 `SceneCommand`，按 provider
保留到后续帧。标量参数对应 JSON 值，定点参数使用整数百万分位。

`Examples/ExtensionPlayer/extension` 提供可公开复用的进度条扩展，旁边的 `player`
crate 将它链接进正常 Player 宿主。测试启动作品包中的命令并检查最终呈现输出。

```sh
cargo xtask project-player Examples/ExtensionPlayer --output ./project-player.exe
# 可用 --target 指定 Cargo 目标，构建机需安装对应原生 SDK。
cargo xtask project-player ./ContentProject --prebuilt ./roukaku-player.exe --output ./content-player.exe
```

`project-player` 用 `--locked --release` 构建 `player/Cargo.toml`，从 Cargo
产物消息中读取唯一可执行文件并复制到输出路径。纯内容工程没有 `player/Cargo.toml`，
直接复用 `--prebuilt`。把输出文件传给现有 package bundle 命令对应平台的 Player
选项即可打包；扩展工程不能用预构建文件替代专用 Player。

`WorkExtensionDeclaration` 和 `ExtensionCommandDeclaration` 只含 provider ID、
命令 schema 和参数 schema。编译集成的 Editor 面板可以共享这些类型，Player
无需依赖 Editor UI。运行时动态库加载器、二进制描述符和插件 ABI 门禁已移除。
`roukaku-engine` 和 `roukaku-vn` 按普通 `rlib` 依赖编译，不再生成多余的 Rust
`dylib`。Android 和 Web 宿主的 `cdylib` 入口仍作为平台产物保留。
扩展必须通过 `restoration()` 明确选择 `Transient` 或 `Restorable`；
`ExtensionState` 提供业务状态采集、候选验证和恢复接口，由检查点/恢复切片接入。

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

## 从 Linux 构建 Apple 目标

原生 macOS CI 继续使用 `macos-15` / `macos-15-intel` 上的 Cargo 和 Xcode。Linux 上的 Apple 构建通过 cross 0.2.5 使用 `Cross.toml` 中的私有镜像。最低版本为 macOS 13.0、iOS 16.0。目标包括 `x86_64-apple-darwin`、`aarch64-apple-darwin`、设备目标 `aarch64-apple-ios`，以及模拟器目标 `aarch64-apple-ios-sim`、`x86_64-apple-ios`。

### 准备合法 SDK

从 [Mac App Store](https://apps.apple.com/app/xcode/id497799835) 或 [Apple Developer Downloads](https://developer.apple.com/download/all/) 取得 Xcode，遵守 Apple 许可。在 Mac 上用 `xip --expand` 展开官方 `.xip`，选择这套 Xcode，并在该 Mac 上接受许可。单独的 Command Line Tools 不含 iOS SDK。随后导出 SDK 内容，保留内部符号链接：

```sh
for sdk in macosx iphoneos iphonesimulator; do
  tar -C "$(xcrun --sdk "$sdk" --show-sdk-path)" -cJf "$sdk.sdk.tar.xz" .
done
```

私下把归档传到 Linux，每个 SDK 解压到独立目录。`SHINKI_APPLE_SDK_DIR` 直接指向含 `SDKSettings.json`、`usr/`、`System/` 的 SDK 内容目录。Darwin 使用 `macosx`，设备使用 `iphoneos`，两个模拟器目标使用 `iphonesimulator`。xtask 和 Dockerfile 都不下载 Apple SDK。SDK 归档及含 SDK 的镜像不得提交到 Git 或公共镜像仓库。

### 准备工具与构建

将源码、缓存和工具放在专用目录，预留足够空间；创建镜像时 `/` 可用空间不足 8 GiB 会直接拒绝。不要删除无关缓存或 Docker 镜像。Linux 配置示例：

Docker 必须提供 Buildx/BuildKit（用 `docker buildx version` 检查）。任务专用插件可放在 `DOCKER_CONFIG=~/shinkirou-apple/docker` 的 `cli-plugins/docker-buildx` 路径。[Buildx v0.38.1 官方 Linux x86_64 附件](https://github.com/docker/buildx/releases/download/v0.38.1/buildx-v0.38.1.linux-amd64) 的 SHA256 为 `e25d5cd675092bb5d1d03632fdc46cdf3fede7b514dc5f023a105e00d3ce5ad4`，核对后再赋予执行权限。无需 sudo，也不修改已有 Docker 配置。

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

每个目标单独创建镜像，构建 iOS 时切换 SDK 目录。镜像固定 Rust 1.98.0；macOS 使用固定 revision 的 [osxcross](https://github.com/tpoechtrager/osxcross)，iOS 使用 LLVM/LLD，并配置目标专用的 C/C++、CMake 工具及 `build/cross/` 中固定 vcpkg 输入的静态 libass。宿主工具仍用 Linux 编译器。原生依赖许可导出到 `.work/apple-native/<target>`，只有镜像身份相同时才复用。构建 JSON 包含 Cargo 实际返回的 `compiler-artifact.executable`，打包使用该路径，不猜测 target 目录。

### Player 资源与测试包

共享 Apple factory 使用 winit/Metal、AVFoundation 解码、CoreAudio 播放及已激活的 iOS 音频会话，接入 AccessKit 语义、触控/输入法、沙箱原子存档和带持久授权的原生 Files 选择器。iOS 的主线程由 UIKit 持有，VM future 在 owner 工作线程运行，不使用桌面事件泵。iOS 不启用桌面的 gilrs 手柄后端。

先 cook 含 `ios-release` profile 的包（对应 `PlatformHostProfile::ios_release`），再用 `roukaku package bundle --platform ios --ios-player <built-player>` 生成平铺资源，补齐通常的 package、target、profile、out 参数。最后把 Cargo 返回的真实可执行文件和资源目录交给打包入口：

```sh
cargo xtask package --target aarch64-apple-ios --binary <compiler-artifact-executable> --resources .work/ios-resources --version 0.1.0-beta.1 --out .work/ios-player-test
```

macOS 输出 ad-hoc 签名的 `.app` 和 ZIP；模拟器输出明确标为测试用的 `.app` 和 ZIP；设备输出带 `Payload/*.app` 的 `adhoc-device-test.ipa`。打包先检查 Mach-O 架构、设备/模拟器平台和依赖闭包，再修改并签署副本。Linux 使用 `rcodesign` 签名并逐个验证 Mach-O，原生 macOS 使用 `codesign`。ad-hoc 签名不包含 provisioning profile、公证或 App Store 身份；安装到设备仍需有效的 Apple 签名与 provisioning，不能当作正式安装包。设备/模拟器的生命周期和运行时符合性另行验证。

真实产物应保留路径、大小，以及 `file`、`llvm-objdump --macho --dylibs-used --private-headers <executable>`、`rcodesign verify <executable>` 的输出。缺少合法 SDK 时，SDK 构建和真实签名验收仍然受阻。


回到[文档首页](index.md)或继续[快速开始](quick-start.md)。
