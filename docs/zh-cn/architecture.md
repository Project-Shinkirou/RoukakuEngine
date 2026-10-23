---
title: 架构与接口
description: crate 分工、会话所有权、运行预算、取消和包与存档完整性。
sidebar: { order: 4 }
---

# 架构与接口

## 代码地图

| 目录 | 职责与主要 crate |
| --- | --- |
| `Engine/Source/Runtime` | `roukaku-core` 的基础类型，`roukaku-runtime` 的场景、任务与会话，`roukaku-package` 的容器，`roukaku-asset` 与 `roukaku-byte-source` 的资源和有界读取，以及文字、音视频、字幕、UI 与 Player 公共机制 |
| `Engine/Source/Platform` | `roukaku-platform` 的宿主契约、`roukaku-platform-common` 的共享宿主实现、各平台后端和 `roukaku-target` 的项目/target 校验 |
| `Engine/Source/Developer` | `roukaku-cook` 的导入与 Cook，`roukaku-release` 的发布数据校验，`roukaku-observability` 的日志与诊断，以及 Headless 测试工具 |
| `Engine/Source/Programs` | `roukaku-cli`（可执行文件 `roukaku`）、桌面/Android/Web Player、Headless、Crash Reporter 与 Player VN 接线 |
| `Engine/Source/Modules/RoukakuVN` | 脚本编译、剧情语义、策略、演出、UI、包 section、编辑事务与 typed VN 产品入口 |
| `Editor/Source/roukaku-editor` | 独立 workspace 中的 GPUI 应用、工程面板、外部 Agent/MCP 接线和 Player 预览进程管理 |

RoukakuVN 内部，`roukaku-vn-script` 编译 `.roukaku` 并生成 source map；`roukaku-vn-core` 拥有剧情游标、变量、选择、等待、backlog 和系统状态；`roukaku-vn-policy` 承载 Luau；`roukaku-vn-presentation` 管理舞台与演出；`roukaku-vn-package` 管理产品 section；`roukaku-vn-ui` 持有 UI 模型；`roukaku-vn-editor` 提供不依赖 GPUI 的编辑事务。`roukaku-vn::VnSession` 组合这些产品能力。

## 会话与平台边界

`roukaku-runtime::EngineSession` 持有公共 World、任务作用域和逻辑步生命周期；`VnSession` 另持有 VN 剧情与系统状态。Player 与 Editor 预览使用同一套产品会话，不另建预览剧情执行器。桌面 Player 使用约 60 Hz 的逻辑调度，呈现和平台事件独立处理。

平台 host profile 明确 renderer、decode、audio mixer、audio output、save provider 和包来源。显式 provider 绑定失败会报告错误，不能将另一后端的输出当作原绑定成功。文字、媒体和字节源等共享库可由实际消费者独立组合，不要求先创建 VN 会话。

## 已实施的运行保护

这些保护属于产品行为，重构时必须保留：

- **预算**：host profile 校验非零队列、帧、音频、PCM cache 和包读取限额；呈现、解码请求、存档读写和包 range 读取在入口检查相应大小。容器另限制 section 数量、表大小和解码后长度；Cook batch 也检查节点和输入字节预算。它们有各自的故障边界，不是一项通用“无限内存保护”。
- **路径**：项目资源与 section 使用校验后的相对路径；bundle 校验路径后写入，场景引用拒绝非法相对路径。Editor 资源导入检查实际目录仍位于工程内，拒绝重名路径和 asset ID。路径校验不授予任意磁盘访问权限。
- **取消**：任务绑定 owner、scope 和句柄。已取消、已完成或属于另一 World 代次的 completion 会被拒绝。关闭会话取消所属任务；恢复用新的任务上下文替换旧上下文，迟到结果不能提交。Cook 使用共享取消 token；Editor 工程切换取消旧编译、导入、Agent 和预览工作。
- **存档完整性**：保存显式运行和 VN 状态，不保存原生句柄或执行栈。读取先解码并校验候选状态，再替换 live World；容器损坏或状态校验失败保留当前状态。成功恢复同时重建任务上下文。不要通过忽略错误来跳过这一边界。

## 包与格式

`.roukakupkg` 和 `.roukakupak` 用于产品包及预览中间包。`roukaku-package` 的 v2 容器使用八字节头 magic `ROUKACT2`，footer magic 为八字节 `ROUKAF2\0`。读取器验证容器种类、版本、长度、section 范围及头、表和 payload 的 hash；延迟读取 section 仍校验存储数据。

更名前的产品容器、schema 和自有存档不属于兼容输入。旧 magic 在入口直接拒绝；改扩展名不会转换内容。应从当前作者源重新 Cook 和打包，保留旧文件与旧用户数据原状。

## 与 ShinkiEMU 的单向共享

ShinkiEMU 使用 `roukaku-core`、`roukaku-byte-source`、`roukaku-media-core`、`roukaku-text`、`roukaku-media`、`roukaku-subtitle`、`roukaku-platform`、`roukaku-platform-common` 和 `roukaku-observability`。生产依赖在 ShinkiEMU workspace 中集中固定到同一个 RoukakuEngine Git revision，不依赖开发机相邻目录，不复制 Rust 实现。

依赖方向只允许 **ShinkiEMU → RoukakuEngine**。RoukakuEngine 及这些共享 crate 不得依赖 ShinkiEMU、其 Manager 或核心 ABI。共享机制留在已有 owner crate；剧情、原生引擎格式和产品规则留在各自产品。

相关操作见[构建](build.md)、[VN 创作](vn-authoring.md)与 [Editor](editor.md)。
