---
title: VN 创作
description: 用项目 manifest、.roukaku、Luau、本地化和资源 sidecar 创作视觉小说。
sidebar: { order: 5 }
---

# VN 创作

公开参考工程是 `Examples/NativeVN`。剧情和 UI 的作者源为 `.roukaku`，controller 使用 `.luau`，主题使用 JSON/YAML。Cook 读取这些文件并生成产品包；运行时执行编译后的剧情，不直接解释 `.roukaku` 文本。

## 项目入口

`project.yaml` 使用 `roukaku.target_manifest.v3`。它声明产品 target、可用 profile、各平台 host profile、NativeVN 源目录和额外 package section。示例中 NativeVN 配置包括：

```yaml
nativevn:
  default_locale: zh-Hans
  sources: [Scripts]
  ui_sources: [UI]
  ui_themes: [Themes]
  ui_controllers: [Controllers]
  profiles: [classic, advanced-vn]
  asset_roots: [AssetSidecars]
```

这是完整项目里的一个配置块，不是独立可打包项目。`targets` 中的 `id` 对应 CLI 的 `--target`，`--profile` 选择参与本次构建的内容。平台 profile 明确指定 renderer、decode、audio 与 save provider，不能只更改平台名称而沿用不匹配的绑定。

## 剧情与演出

`.roukaku` 使用缩进块和具名属性。以下片段采用示例已有的结构与本地化 key：

```text
story main #@id story.main
state experience_read #@id state.experience_read
  scene reading #@id scene.experience.reading
    text key:experience.read.first window:main #@id experience.read.first
    text key:experience.read.second window:main #@id experience.read.second
```

保留稳定的 `#@id`，供诊断、source map、Editor 选择和属性修改定位。使用 `state`、`scene` 组织内容，`choice`/`option` 和 `jump` 表达控制流；素材通过 `asset:/...` ID 引用。舞台使用 layer、camera、show/background 等命令，时间线为目标属性声明 keyframes、easing 与 join 策略。真实语法可查 `Scripts/experience.roukaku`，不要把尚未实现的 DSL 当作编译器输入。

Graph 和 Timeline 修改同一源码，作者不需要维护第二份运行时图。剧情变量、选择、backlog、已读与存读档由 VN Core 持有。

## UI 与 Luau

`UI/flagship.roukaku` 中的 `ui_view` 定义控件树，`ui_bind` 按 profile 将 message、choice 或 system page 绑定到 view、controller、policy 和 theme。控件读取 `$model` 数据，通过 `vn.advance`、`vn.choose` 等 typed action 发出请求。

Luau 的产品全局名是 **`roukaku`**。示例 controller 使用 `roukaku.ui.controller.register(...)` 注册处理器，用 `roukaku.ui.effect.forward(action)` 转交 UI action。Controller 文件由 `nativevn.ui_controllers` 收集；它们不是 `.roukaku` 文件。策略也通过 `roukaku` 暴露的机制处理命令和状态修改，不能靠修改私有 Lua table 改写 VN 权威状态。

使用现有系统页与模型组织存读档、设置和历史记录，不在 controller 中另写存档容器或剧情推进器。

## 本地化

剧本文本引用稳定的 `key`。例如 `Localization/en.json` 的格式是：

```json
{
  "schema": "roukaku.vn.localization_table.v1",
  "locale": "en",
  "strings": {
    "experience.read.first": "The last train has left, but a reflection remains."
  }
}
```

在 `package_sections` 中将文件声明为 `vn.localization.<locale>`，schema 使用 `roukaku.vn.localization_table.v1`，并选择适用的 target/profile。默认语言由 `nativevn.default_locale` 指定；Cook 拒绝没有符合当前 profile 的默认语言 section 的项目。翻译应保持相同 key，UI 文案可用 `l10n:` 引用。

## 资源与 sidecar

`.roukaku-asset.yaml` 放在声明的 `asset_roots` 内，描述稳定 asset ID、项目相对 `source`、`source_hash`、类型、许可证、importer、cook processor 和 target profiles。`AssetSidecars/` 与原始图片、音频、视频和字体是不同职责：sidecar 描述资源，源码通过 asset ID 使用它。

修改资源后用现有 importer 更新 hash 与元数据，不要保留旧 hash 或伪造授权。Editor 的 Import image 可导入 PNG/JPEG/WebP 并生成 sidecar，详见 [Editor](editor.md)。将可分发许可、来源和作者记录随资源保留；商业游戏正文、密钥和存档不进入工程仓库。

### 覆盖挂载

package 的 VFS manifest 为基础 package section 和覆盖层声明优先级。宿主只能挂载已明确授权的本地目录；Player 会拒绝未授权目录并给出 VFS 授权诊断。覆盖内容沿用 catalog 中已有的 asset ID 和 URI，让图片、音频、视频、字体、UI 文档、本地化表和脚本共用同一资源命名空间。同优先级冲突会报错；whiteout 用于明确隐藏低优先级资源。文件替换后内容 revision 改变，只使该资源对应的缓存失效。

## 检查与播放

```sh
cargo run --locked -p roukaku-cli -- ui check Examples/NativeVN/project.yaml --profile advanced-vn --target nativevn-flagship-game
cargo xtask ui-check
```

单文件语法检查不能替代全项目的跨文件语义检查。完成修改后走[快速开始](quick-start.md)的 Cook、package 与 Player 路径，检查实际文本、布局、媒体和保存恢复行为。
