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

### VFS 覆盖层

Player 根据 package 的 VFS manifest 解析 catalog asset ID。基础 package section、覆盖 package 和宿主授权的本地目录遵循同一套优先级规则；同优先级匹配会报歧义，生效的 whiteout 会隐藏低优先级内容。本地路径只能通过有界 catalog 读取，并限制在宿主明确授权的目录内。资源缓存身份包含 asset ID、选中的来源层和内容 hash，覆盖版本变化时只替换受影响资源的解码缓存。

## Luau 创作机制契约（R1/R3 基础）

`roukaku-vn-policy::authoring::LuauAuthoringRuntime` 通过现有 `luau-runtime` feature 和 mlua 在官方 Luau VM 中运行项目源码。机制 API 独立于默认 VN UI。本集成分支保留原有 DSL，不改 Player/Cook 入口；后续产品切换直接使用这套 API。

### 段落、模块和输入

宿主以 `PolicyExecutionBudget`、所有者 `TaskScope` 和 `Rc<dyn AuthoringHost>` 创建运行时。`replace_module(module, source_name, source)` 执行源码块，取得其返回的导出表。`source_name` 应使用 `@Story/chapter1.luau` 这类真实文件名，让 Luau 错误和调用栈定位到作者源码。模块的全局环境只读；局部变量和显式注册状态可正常使用。

```lua
return {
    paragraphs = {
        intro = function()
            local line = rk.line("greeting", { text = "Hello" })
            rk.wait_input("advance")
            local choice = rk.choice("route", {
                { id = "left", text = "Left" },
                { id = "right", text = "Right" },
            })
            local selected = rk.wait_input("choose")
            rk.choice_result(choice, selected)
        end,
        -- 也支持入口表：intro = { run = function() ... end }
    },
}
```

`start_paragraph(ParagraphId { module, entry })` 从 `exports.paragraphs[entry]` 解析函数或 `{ run = function }`。这对标识是段落重启和读档的稳定边界。重启取消旧片段与输入，创建新的子作用域和协程，并清空节点访问计数。何时安装入口状态、何时重演，由存档/重演层决定。

`rk.wait_input(action: string): any` 只挂起剧情协程。宿主取得 `StoryStatus::Waiting(InputWait { token: u64, action: String })`，以 `input(token, action, json_payload)` 恢复匹配的 live wait。不相关输入和迟到输入返回 `false`。段落重启或运行时替换后，live token 仍不会重复；只有当前剧情协程可以发起等待。等待期间宿主仍调用 `advance_presentation(seconds)` 推进演出，不阻塞宿主线程、音频时钟或渲染器。

`rk.call(module: string, name: string, ...): ...` 每次调用都重新解析导出函数，支持项目函数内部等待。时间线回调使用 `{ module: string, name: string }`，同样在派发时解析。替换模块影响后续调用；已经挂起的调用帧继续使用旧函数。源码块执行失败时不替换导出表，但错误前显式执行的机制副作用仍然保留。结构不兼容时应取消并重启段落。

### 节点、文字和可恢复状态

| 机制 | 类型与语义 |
| --- | --- |
| `rk.line(node, data)` | 记录台词及项目纯数据，返回 `NodeOccurrence = { paragraph = { module, entry }, node, visit }`；不自动显示 UI 或等待。 |
| `rk.choice(node, options)` | 记录 `{ id: string, text: string }[]`，要求选项 ID 非空且不重复，返回同类 occurrence。 |
| `rk.choice_result(occurrence, selected)` | 向该次选择记录提交一个有效选项 ID，只能提交一次。 |
| `rk.measure_text(request)` | 接收完整序列化的 `roukaku_text::TextLayoutRequest`，保留方向、Ruby、字体链和约束，返回 `{ width, height, clipped, ellipsized, revision }`。 |
| `rk.display_text(id, request)` | 使用同一排版 provider，返回同类测量值；`AuthoringHost::display_text` 接收包含字形资源及 source/cluster 映射的 `Arc<TextLayoutResult>`，直接用于绘制。 |
| `rk.register_state(scope, key, initial)` | 注册非空状态键；已有值时返回已有值，首次注册才写入初值。重复注册不覆盖恢复的数据。 |
| `rk.get_state(scope, key)` / `rk.set_state(scope, key, value)` | 读写已注册键，未注册的键报错。scope 为 `"variables"`、`"scene"` 或 `"extensions"`。 |

visit 是**段落本次执行中，每个源码节点各自从 1 开始的访问次数**。循环中的多次访问各有身份；从同一入口重演相同路径时会生成相同身份。重启后逻辑节点身份可以重复，live wait/segment token 则重新分配。节点记录由机制持有，替换对话 UI 不会丢失这些记录；历史和重演层通过 `records()` 读取。

可恢复值为有限数值的 JSON 类纯数据，支持数组和 null，受深度和宿主内存预算限制。函数、线程、原生 userdata 句柄、任意 metatable 和循环表会被拒绝。`recoverable_state()` 返回显式变量、场景和扩展状态；`restore_registered_state(candidate)` 先校验再安装，要求当前没有 live 剧情协程或运行片段。它不创建原生对象，也不恢复执行栈。舞台属性同时记录在 `scene[target][property]`，因此 target 对应的场景状态必须是对象，项目不得在同一键存储非对象值。禁止序列化任意 Luau 栈。

### 声明演出与终态所有权

```lua
type FunctionRef = { module: string, name: string }
type Keyframe = { time: number, value: number }
type Track = { target: string, property: string, keys: {Keyframe} }
type Callback = { time: number, call: FunctionRef, complete: FunctionRef }
type Timeline = {
    duration: number, tracks: {Track}, callbacks: {Callback}?,
    on_complete: FunctionRef?, on_cancel: FunctionRef?,
}
```

`rk.segment(declaration: Timeline): number` 创建作用域内片段，并立即求值时间零。时间和时长以有限的秒数表示，属性值必须有限。每个 target/property 只能出现一次；关键帧时间在 `[0, duration]` 内严格递增，轨道采用线性插值和端点钳制。`TimelineDeclaration::evaluate(time, apply)` 提供任意时刻的纯求值，供编辑和预览使用，不调用自由逻辑，也不修改宿主。项目效果、媒体和自由逻辑通过回调引用与宿主属性接入，声明中不保存闭包。

`rk.set_property(target, property, value)` 经 `AuthoringHost::set_property` 应用有限数值属性，并更新可恢复场景数据。`advance_presentation(delta)` 独立于剧情输入求值运行轨道。到时的回调按声明顺序各执行一次。生命周期回调同步执行，不得 yield。

`rk.complete(id)`（宿主侧为 `complete_segment`）取得 `Completed` 所有权，应用所有轨道终值，对尚未派发的定时回调调用其 `complete` 引用，再调用 `on_complete`，最后释放资源。正常播放派发到时的 `call` 引用，到达 duration 后完成。`rk.cancel(id)`/`cancel_segment` 取得 `Cancelled` 所有权，停在当前状态，调用 `on_cancel` 并释放资源，不应用终值或必要完成结果。首次终态转换返回 `true`，片段已终结时返回 `false`。所有者作用域已经失效时，取消只释放原生资源，不再执行该作用域中的项目代码。

终态所有权在用户回调之前取得。回调失败向上传播，重试不会重复副作用；失败后仍尝试释放资源。未处理的剧情错误或演出 tick 错误会取消其余所属工作。父作用域取消、段落重启、显式取消和运行时析构都会释放所属片段。每个终结片段的 `AuthoringHost::release_segment(id)` 只调用一次，完成和取消都遵守这一规则。

VM 保留原有宿主 IO 沙箱与 Lua 内存限额；指令和输出额度只在宿主控制的派发边界重置。宿主纯数据按运行时生命周期保守累计计费，包括保留的节点记录、演出声明和状态副本；更新已有数值舞台属性不再计入一份存储副本。预算耗尽后持续拒绝操作，需要新建运行时。项目 API 不能重置预算，也不能将工作从所有者作用域分离。


## 与 ShinkiEMU 的单向共享

ShinkiEMU 使用 `roukaku-core`、`roukaku-byte-source`、`roukaku-media-core`、`roukaku-text`、`roukaku-media`、`roukaku-subtitle`、`roukaku-platform`、`roukaku-platform-common` 和 `roukaku-observability`。生产依赖在 ShinkiEMU workspace 中集中固定到同一个 RoukakuEngine Git revision，不依赖开发机相邻目录，不复制 Rust 实现。

依赖方向只允许 **ShinkiEMU → RoukakuEngine**。RoukakuEngine 及这些共享 crate 不得依赖 ShinkiEMU、其 Manager 或核心 ABI。共享机制留在已有 owner crate；剧情、原生引擎格式和产品规则留在各自产品。

相关操作见[构建](build.md)、[VN 创作](vn-authoring.md)与 [Editor](editor.md)。
