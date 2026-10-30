---
title: 构建
description: Rust 工具链、原生依赖、xtask 检查及 Android 和 macOS 构建入口。
sidebar: { order: 3 }
---

# 构建

## 工具链与 workspace

`rust-toolchain.toml` 选择 Rust `1.99.1`，并声明 `rustfmt`、`clippy` 和目标列表。Engine crate 使用 Rust 2024 edition，最低需要 Rust 1.99。目标列表用于安装 Rust 标准库，不是平台运行支持矩阵。安装本机 C/C++ 编译器、链接器、CMake，以及所选原生依赖需要的工具；Windows 使用 MSVC 工具链。

根 workspace 包含 Engine、RoukakuVN、Player、CLI 和共享库。Editor 使用 `Editor/Cargo.toml` 及自己的 lockfile。根目录的 `cargo xtask` 统一选择 `engine`、`editor` 或 `all`，底层 Cargo 命令使用 `--locked`。

依赖基线固定为 vcpkg `0c2a9ace1964b818b15f7ae50ba1c42404918a7b`、libass 0.17.5，以及可选 FFmpeg 9.0.2 port revision 2。`mlua` 保持最新发行版 0.12.2；其发行版 `mlua-sys` 0.13.0 会在 VM 初始化时拒绝 Luau 744 生成的字节码，因此 workspace 固定使用[上游 loader 修复](https://github.com/mlua-rs/mlua/blob/ff96ed786295cae96f43ef1372bd22be0fbf7866/mlua-sys/src/luau/compat.rs)，不降级 Luau，也不跳过运行时测试。

Engine 渲染使用 wgpu 30.0.x，与 ShinkiEMU 及 Slint 1.18.1 的 `unstable-wgpu-30` 后端一致。Windows 保留 `static-dxc` feature。呈现通过提交命令的 queue 执行，surface 使用默认 `Auto` 色彩空间，保持 SDR 输出。Editor 当前不通过 Engine crate 依赖 wgpu，其 GPUI 渲染器及独立 lockfile 不变。

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

以下测试不打开窗口，会在原生 GPU 上实际执行场景绘制、WGSL 滤镜、纹理上传与回读：

```sh
cargo test --locked -p roukaku-platform-common --test offscreen native_offscreen_gpu_renders_scene_filter_and_readback -- --ignored --nocapture
```

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

工具将 vcpkg 放在 `.work/vcpkg`，并为每个 target 使用独立安装根目录 `.work/vcpkg-subtitle/installed-<triplet>`。它打印 target 对应的环境设置，也会写入 GitHub Actions 环境文件。`ROUKAKU_TEXT_VCPKG_ROOT` 可精确覆盖安装根目录；普通构建根据 Cargo target 自动确定。Windows MSVC 配置如下：

```powershell
$env:ROUKAKU_TEXT_VCPKG_ROOT = (Resolve-Path .work/vcpkg-subtitle/installed-x64-windows-static-md-dynbidi).Path
$env:ROUKAKU_SUBTITLE_VCPKG_ROOT = $env:ROUKAKU_TEXT_VCPKG_ROOT
$env:PATH = "$env:ROUKAKU_TEXT_VCPKG_ROOT/x64-windows-static-md-dynbidi/bin;$env:ROUKAKU_TEXT_VCPKG_ROOT/x64-windows-static-md-dynbidi/debug/bin;$env:PATH"
```

准备 FFmpeg 后还要设置打印的 `VCPKG_ROOT`、`VCPKG_DEFAULT_TRIPLET` 和运行库 PATH。非 Windows 目标按输出设置 `PKG_CONFIG_PATH` 及 `LD_LIBRARY_PATH` 或 `DYLD_LIBRARY_PATH`。原生依赖的分发许可与动态库仍需随最终产品处理。

文字与字幕共用固定版本的原生依赖。`cargo xtask deps --target <triple> --text` 会将 Pango 1.58、HarfBuzz、FriBidi、FreeType、Fontconfig 和 libass 0.17.5 安装到独立的 `.work/vcpkg-subtitle/installed-<triplet>`。`ROUKAKU_TEXT_VCPKG_ROOT` 和 `ROUKAKU_SUBTITLE_VCPKG_ROOT` 都指向该安装根目录。动态依赖需要把对应 `<triplet>/bin` 加入 PATH。也可通过 pkg-config 使用系统安装的 `pangoft2 >= 1.50`。

目标与 vcpkg triplet 的对应关系为：Windows MSVC → `x64-windows-static-md-dynbidi`；Linux x64 → `x64-linux`；Android arm64/x64 → `arm64-android-dynbidi` / `x64-android-dynbidi`；macOS arm64/x64 → `arm64-osx` / `x64-osx`；iOS arm64 真机 → `arm64-ios`；iOS arm64/x64 模拟器 → `arm64-ios-simulator` / `x64-ios`。Android 使用 NDK 目标编译器和目标 pkg-config 元数据，Apple 构建需要对应 SDK。OHOS 与 emscripten 后续在同一 FT2 bridge 上接入工具链。


## 视频播放与音画同步

平台解码器仍是主路径：Windows 用 WMF，Apple 平台用 AVFoundation，Linux 用
GStreamer，Android 用 MediaCodec，实验性 Web 目标用 WebCodecs。FFmpeg 保持可选。
AVFoundation 视频通过 `AVAssetReader` 按需读取带 PTS 的帧，只保留一个预读 sample
和 Player 的一个待呈现帧，不再将整段视频聚合成 `CpuBuffer`。关闭流会取消 reader；
seek 会重建解码器，让旧呈现工作失效。

Player 将同一视频容器的音轨交给所选平台音频解码器，再送入现有 Kira mixer。
公共媒体 scheduler 用 PCM voice cursor 扣除已渲染但未消费的样本和设备输出延迟，
得到音频主时钟。CPAL 输出读取 callback/playback 时间戳，Android 读取 Oboe 延迟。
无音轨的视频才使用宿主时间。暂停会冻结 transport；seek 清空旧输出队列，在目标位置
重建双轨。设备恢复从可听位置重建 PCM voice。电影音频沿用现有 PCM 预算；
音轨结束后，设备实际消费的 mixer 静音继续驱动视频尾部；完成时间取双轨实际 EOS，
不依赖容器时长估计。

Windows 回归用 WMF 解码双轨，经真实 Kira mixer 写入有界原生输出队列，再用受控软件
callback 消费样本：

```sh
cargo test --locked -p roukaku-audio-kira --test native_movie_sync -- --nocapture
cargo test --locked -p roukaku-media --test container_tracks
```

测试覆盖暂停、seek 和输出重建，检查视频 PTS 与音频进度的偏差不超过 scheduler 的
100 ms 滞后 / 20 ms 提前阈值，不打开物理音频设备。Apple 的 reader、PTS、seek、
cancel 回归需要 Apple SDK，由 CI `products` 的 `macos-15` 和 `macos-15-intel` 行执行。

`roukaku-media/tests/fixtures` 中的 `av-clock.mp4`、`silent.mp4`、`silent.webm`
由测试图案和正弦波生成，按 CC0 提供，不含下载的媒体内容。音视频样本生成命令：

```sh
ffmpeg -f lavfi -i testsrc2=size=64x64:rate=25:duration=3 -f lavfi -i sine=frequency=440:sample_rate=48000:duration=1.8 -c:v libx264 -pix_fmt yuv420p -bf 0 -g 25 -c:a aac -ac 2 -b:a 32k -movflags +faststart av-clock.mp4
```


## Android Player

入口是 `cargo xtask android`。需要 JDK 27、`cargo-ndk`、Rust `aarch64-linux-android` target，设置 `ANDROID_SDK_ROOT` 或 `ANDROID_HOME`。安装 SDK 平台 `platforms;android-37.2`、Build Tools `37.0.0` 和稳定版 [NDK r30](https://developer.android.com/ndk/downloads) `30.0.16248370`。工程使用 AGP 9.4.1 和 Gradle 9.8.2。JNI 库继续使用 16 KiB ELF 页对齐；APK 中不压缩的原生库使用现代 zip 对齐。

构建 JVM 继续使用 JDK 27：[Gradle 9.8.2 明确支持 Java 27](https://docs.gradle.org/9.8.2/userguide/compatibility.html)，[Android 的 JDK 指引](https://developer.android.com/build/jdks)要求构建 JDK 不低于 AGP 的最低版本（AGP 9.4 为 JDK 17）。这里选择的是运行构建工具的 JVM，不是 Android 应用的 Java API 或字节码版本。

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
rustup toolchain install 1.99.1 --profile minimal
rustup target add --toolchain 1.99.1 x86_64-apple-darwin aarch64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
cargo +1.99.1 install --locked --root "$HOME/shinkirou-apple/tools" cross --version 0.2.5
cargo +1.99.1 install --locked --root "$HOME/shinkirou-apple/tools" apple-codesign --version 0.29.0
export SHINKI_APPLE_SDK_DIR="$HOME/shinkirou-apple/sdks/macosx.sdk"
cargo xtask apple-image --target aarch64-apple-darwin
cargo xtask build --workspace engine --target aarch64-apple-darwin -p roukaku-player --release
cargo xtask cross --target aarch64-apple-darwin check -- -p roukaku-core
```

每个目标单独创建镜像，构建 iOS 时切换 SDK 目录。镜像固定 Rust 1.99.1；macOS 使用固定 revision 的 [osxcross](https://github.com/tpoechtrager/osxcross)，iOS 使用 LLVM/LLD，并配置目标专用的 C/C++、CMake 工具及 `build/cross/` 中固定 vcpkg 输入的静态 libass。宿主工具仍用 Linux 编译器。原生依赖许可导出到 `.work/apple-native/<target>`，只有镜像身份相同时才复用。构建 JSON 包含 Cargo 实际返回的 `compiler-artifact.executable`，打包使用该路径，不猜测 target 目录。

### Player 资源与测试包

共享 Apple factory 使用 winit/Metal、AVFoundation 解码、CoreAudio 播放及已激活的 iOS 音频会话，接入 AccessKit 语义、触控/输入法、沙箱原子存档和带持久授权的原生 Files 选择器。iOS 的主线程由 UIKit 持有，VM future 在 owner 工作线程运行，不使用桌面事件泵。iOS 不启用桌面的 gilrs 手柄后端。

先 cook 含 `ios-release` profile 的包（对应 `PlatformHostProfile::ios_release`），再用 `roukaku package bundle --platform ios --ios-player <built-player>` 生成平铺资源，补齐通常的 package、target、profile、out 参数。最后把 Cargo 返回的真实可执行文件和资源目录交给打包入口：

```sh
cargo xtask package --target aarch64-apple-ios --binary <compiler-artifact-executable> --resources .work/ios-resources --version 0.1.0-beta.1 --out .work/ios-player-test
```

macOS 输出 ad-hoc 签名的 `.app` 和 ZIP；模拟器输出明确标为测试用的 `.app` 和 ZIP；设备输出带 `Payload/*.app` 的 `adhoc-device-test.ipa`。打包先检查 Mach-O 架构、设备/模拟器平台和依赖闭包，再修改并签署副本。Linux 使用 `rcodesign` 签名并逐个验证 Mach-O，原生 macOS 使用 `codesign`。ad-hoc 签名不包含 provisioning profile、公证或 App Store 身份；安装到设备仍需有效的 Apple 签名与 provisioning，不能当作正式安装包。设备/模拟器的生命周期和运行时符合性另行验证。

真实产物应保留路径、大小，以及 `file`、`llvm-objdump --macho --dylibs-used --private-headers <executable>`、`rcodesign verify <executable>` 的输出。缺少合法 SDK 时，SDK 构建和真实签名验收仍然受阻。


回到[文档首页](index.md)或继续[快速开始](quick-start.md)。
