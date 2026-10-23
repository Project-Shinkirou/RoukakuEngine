---
title: 快速开始
description: 构建 CLI，Cook 公开 NativeVN 示例，打包并启动 Player。
sidebar: { order: 2 }
---

# 快速开始

以下命令在仓库根目录运行。先按[构建](build.md)准备 Rust、C/C++ 工具链和原生依赖，并获取仓库的 Git LFS 资产。示例直接使用已提交的作者源，不需要生成脚本。

## 构建工具

```sh
cargo build --locked -p roukaku-cli -p roukaku-player -p roukaku-crash-reporter
cargo run --locked -p roukaku-cli -- --help
```

CLI 的 Cargo package 名是 `roukaku-cli`，可执行文件名是 `roukaku`。以下用 `cargo run`，无需先安装到 PATH。

## Cook 与包构建

公开项目位于 `Examples/NativeVN`，项目入口是 `project.yaml`。使用其中实际声明的 target 与 profile：

```sh
cargo run --locked -p roukaku-cli -- cook Examples/NativeVN/project.yaml --profile advanced-vn --target nativevn-flagship-game --out .work/nativevn-cooked
cargo run --locked -p roukaku-cli -- package build .work/nativevn-cooked --target nativevn-flagship-game --out .work/nativevn.roukakupkg
```

Cook 编译剧情与 UI，并收集本地化、资源 sidecar 和项目 section。`package build` 校验 Cook 产物 hash 后生成容器。输出选用新的路径；重新制作时另选路径，避免覆盖已有产物。

## Windows Player 预览

在 PowerShell 中，将 `$env:VC_CRT_REDIST` 指向与构建工具链匹配、允许随程序分发的 Microsoft VC x64 CRT 目录。目录必须包含 `msvcp140.dll`、`vcruntime140.dll`、`vcruntime140_1.dll`，不能使用调试版 DLL。

```powershell
cargo run --locked -p roukaku-cli -- package bundle .work/nativevn.roukakupkg --out .work/nativevn-preview --target nativevn-flagship-game --profile advanced-vn --platform windows --windows-player target/debug/roukaku-player.exe --windows-runtime "$env:VC_CRT_REDIST" --crash-reporter target/debug/RoukakuCrashReporter.exe
.work/nativevn-preview/RoukakuPlayer.exe
```

运行 bundle 内的 Player，它从相邻的 `RoukakuPlayer.config.json` 读取配置并打开独立窗口。原始 `target/debug/roukaku-player.exe` 需要 bundle 配置，不能直接接收项目源码路径。

用物理输入和 UI 操作示例。逐字显示期间，第一次推进补全文字，下一次推进继续剧情。检查画面、可听音频、选择和存读档；Cook 或编译成功不代表这些运行行为已经验证。

Linux 使用同一 Cook 和包，bundle 参数改为 `--platform linux --linux-player target/debug/roukaku-player`，省去 Windows CRT 与 crash reporter 参数，运行输出目录中的 `roukaku-player`。macOS 使用 `--platform macos --macos-player target/debug/roukaku-player`，入口为输出目录中的 `Contents/MacOS/roukaku-player`。各自仍需满足项目对应的原生 provider 依赖。

## Editor 中预览

```sh
cargo run --locked --manifest-path Editor/Cargo.toml -p roukaku-editor -- Examples/NativeVN/project.yaml
```

通过 Preview setup 载入本地工具配置，再使用 Save & Preview。它执行同一条 Cook、package、bundle 和 Player 路径，并提供实际 Player 握手后的暂停、恢复与停止。配置字段和当前编辑能力见 [Editor](editor.md)。

接下来阅读 [VN 创作](vn-authoring.md)或[架构](architecture.md)。
