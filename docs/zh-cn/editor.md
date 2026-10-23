---
title: Editor
description: 打开工程、编辑同一份源码、安全保存、导入资源并运行真实 Player 预览。
sidebar: { order: 6 }
---

# RoukakuEditor

Editor 是独立 workspace 中的 GPUI 桌面应用。人工编辑不需要 Agent 或模型服务。

## 打开工程

先按[构建](build.md)准备依赖，在仓库根目录运行：

```sh
cargo run --locked --manifest-path Editor/Cargo.toml -p roukaku-editor -- Examples/NativeVN/project.yaml
```

也可传入单个 `.roukaku` 文件。工程模式读取项目声明的剧情、UI、主题和 Luau controller。使用 Open project 切换工程；切换前处理未保存修改，打开失败保留当前工程。

## 编辑与保存

Content Browser 查找源文件与声明的资源；Outliner 选择命令；Details 修改选中命令的属性。Text、Graph 和 Timeline 操作同一份版本化源码。稳定的 source ID 将面板选择和诊断定位回作者输入。

- **Graph** 展示实际 state 与路由。将 state 的连接端口拖到目标 state 可添加 jump；Delete route 删除对应控制流命令。当前节点采用固定网格，不支持自由拖动布局。
- **Timeline** 展示实际关键帧，可修改时间/值、插入、删除或在原轨道拖动。一次拖动释放后提交一个可撤销 batch；过期版本和重复时间会拒绝。支持现有 linear/ease_in/ease_out/ease_in_out，尚无自由曲线编辑和时间缩放。
- **Save / Ctrl+S / Cmd+S** 保存全部源文档。文件先写临时文件再替换；如果磁盘内容已被外部修改，拒绝覆盖并要求重开。多个文件逐个保存，IO 失败会报告错误，不承诺跨文件原子提交。
- **Undo batch / Redo batch** 使用共享编辑历史，撤销也产生新版本。文本、属性与外部编辑都受预期版本和 UTF-8 范围校验；旧结果不能因文字恢复原样而重新生效。

编译诊断来自现有项目编译器，显示源位置并可跳转。切换工程或更新源码后，旧编译结果不能覆盖当前诊断。关闭存在未保存修改的工程时，可取消、保存或丢弃。

## 导入图片

Content Browser 的 Import image 支持 PNG、JPEG、WebP。选择工程相对目标路径、asset ID、许可及已声明的 sidecar 根目录。后台通过 `roukaku-cook` importer 解码、计算 hash，并写入图片与 `.roukaku-asset.yaml`。

重名路径或 ID 会拒绝导入；目录必须在工程内。导入立即写入资源，不进入 `.roukaku` Undo 历史。单文件模式需先打开完整项目。当前没有音频、视频或字体导入表单，也没有资源缩略图。

## 配置 Player 预览

先构建 CLI、Player 和 Windows 所需 Crash Reporter，见[快速开始](quick-start.md)。预览配置是本地 JSON，字段如下：

| 字段 | 含义 |
| --- | --- |
| `project` | 当前 `project.yaml` 路径 |
| `cli` | 已构建的 `roukaku` 可执行文件 |
| `player` | 已构建的 `roukaku-player` 可执行文件 |
| `profile` | 本次预览的 profile，如 `advanced-vn` |
| `target` | 项目 target ID，如 `nativevn-flagship-game` |
| `windows_runtime` | Windows 必填，匹配的 Microsoft VC x64 CRT 可再发行目录 |
| `crash_reporter` | Windows 必填，已构建的 `RoukakuCrashReporter.exe` |

使用指向当前 checkout 的绝对路径，将配置保留在 ignored 目录。以下 PowerShell 从当前仓库生成配置；先设置 `$env:VC_CRT_REDIST`：

```powershell
New-Item -ItemType Directory -Force .work | Out-Null
@{
  project = (Resolve-Path Examples/NativeVN/project.yaml).Path
  cli = (Resolve-Path target/debug/roukaku.exe).Path
  player = (Resolve-Path target/debug/roukaku-player.exe).Path
  profile = 'advanced-vn'
  target = 'nativevn-flagship-game'
  windows_runtime = (Resolve-Path $env:VC_CRT_REDIST).Path
  crash_reporter = (Resolve-Path target/debug/RoukakuCrashReporter.exe).Path
} | ConvertTo-Json | Set-Content -Encoding utf8NoBOM .work/editor-preview.json
cargo run --locked --manifest-path Editor/Cargo.toml -p roukaku-editor -- Examples/NativeVN/project.yaml --preview-config .work/editor-preview.json
```

该示例需要 PowerShell 7，以无 BOM 的 UTF-8 写 JSON。Linux/macOS 使用相同字段指向本机可执行文件，省去两个 Windows 字段。也可在已打开的 Editor 中用 Preview setup 选择配置。

Save & Preview 编译并保存当前源码，随后执行 `cook → package build → package bundle → Player`，启动独立 GPU 窗口。临时中间包位于项目 `.roukaku-cache/preview` 下。Player 返回 Ready 后才出现 Pause/Resume；暂停时仅提供当前片段实际保留的 checkpoint 定位。Stop preview 先请求真实会话关闭，超时再回收子进程。

修改源码、切换项目、再次预览或关闭 Editor 会使旧预览失效并回收进程。失败显示退出状态与有界日志；不要把编译成功当作图像和可听音频的验证。

## 可选外部 Agent

`--agent` 接受外部 ACP Agent 命令；模型与认证由该 Agent 配置。`--mcp` 启动 stdio MCP，提供 `read_document` 和 `apply_batch`，访问同一份未保存文档。默认逐批确认，用户 Apply batch 或 Reject batch；自主模式仍执行相同版本、范围和取消校验。取消或人工编辑会拒绝旧代次结果。

Editor 不提供自己的模型请求循环，也不为外部进程授予通用沙箱保证。人工编辑与 Player 预览均可独立使用。

继续阅读 [VN 创作](vn-authoring.md)和[架构](architecture.md)。
