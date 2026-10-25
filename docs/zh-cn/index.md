---
title: RoukakuEngine 文档
description: 了解 RoukakuEngine、RoukakuVN、Player、Editor 和 CLI 的分工。
sidebar: { order: 1 }
---

# RoukakuEngine

RoukakuEngine 用于创作和运行原生 2D 作品与视觉小说。共享运行层提供场景、任务、时钟、输入、文字、媒体和资源管理；RoukakuVN 在其上实现剧情、选择、演出、系统页与存读档。

- **RoukakuPlayer** 是作品运行宿主，加载打包数据，管理窗口、设备及平台事件。
- **RoukakuVN** 是视觉小说产品模块。作者编写 `.roukaku` 源码，经 Cook 编译后由同一套 `VnSession` 执行。
- **RoukakuEditor** 提供文本、剧情图、时间线、Details 和独立 Player 预览。面板编辑回写同一份源码。
- **`roukaku`** 是 CLI，负责脚本与 UI 检查、Cook、包构建和平台 bundle。

ShinkiEMU 是运行原生旧引擎的独立产品。它按固定 Git revision 使用 RoukakuEngine 的共享库；RoukakuEngine 不依赖 ShinkiEMU。

## 从这里开始

1. [快速开始](quick-start.md)：构建工具，将公开 `Examples/NativeVN` 示例打包并交给 Player。
2. [构建](build.md)：Rust 与原生依赖、workspace 检查、Android 和 macOS 交叉编译。
3. [架构与接口](architecture.md)：crate 职责、所有权、预算、取消与数据完整性。
4. [VN 创作](vn-authoring.md)：项目、剧情、UI、Luau、本地化和资源。
5. [Editor](editor.md)：编辑、保存、导入与产品预览。

配置中声明某个平台不等于该平台的窗口、输入、声音与存读档已经通过设备验证。具体构建入口以[构建文档](build.md)和当前 manifest 为准。

[English](../en/index.md) · [日本語](../ja/index.md)
