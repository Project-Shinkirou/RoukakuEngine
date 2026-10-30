# 玻璃雨中的信号 / Signal in the Glass Rain

这是直接替换原两路线技术样例的 NativeVN 旗舰项目。默认入口是连续的分层演出，复用已有授权视听资产；旧故事保留为可玩分支，不要求新演出维持旧对白数量或路线组织。`Scripts/*.luau`、UI 和 controller 是作者源码；项目刷新工具不再根据旧剧本 JSON 覆盖它们。

## 范围

本目录包含以下内容：

- `Narrative/` 与 `Localization/` 下的中英文剧本、路线、镜头、voice cue 和审校记录
- `Visual/` 下的角色、背景、CG、UI、图标、视频、alt text 与生成记录
- `Audio/` 下的原创 BGM、stinger、SE master、分发文件与检测报告
- `Design/` 下的视觉规范、UI tokens、alt-text policy 和代码原生 SVG system icon set
- `Schemas/` 下的四个内容内部 JSON Schema
- `Manifests/` 下的内容索引、provenance 与人工审查记录
- `Scripts/`、`UI/`、`Themes/`、`Controllers/`、`AssetSidecars/` 和 `project.yaml` 组成的 Cook 输入
- `README.md`、`STATUS.md`

当前项目打包 180 条中文配音：林瑶使用 `Eve`、周衡使用 `Rex`，由 `x-ai/grok-voice-tts-1.0` 生成并转为 48 kHz/24-bit WAV master 与 48 kHz OGG distribution。用户已明确授权这些既有声线输出进入发行版。产品验证使用正常 Cook/package/Player 路径，不另建剧情执行器。

## 状态摘要

| 状态项 | 当前值 | 说明 |
| --- | --- | --- |
| `content_creation` | `complete` | 180 条中英文对白、三路线、79 张视觉文件、25 个原创音频源及其分发版本、UI 与视频素材已经入包；不等于可运行或可发布。 |
| `public_release_assets` | `ready_with_authorized_voice` | 180 条配音与其他素材按 manifest/provenance 进入发行 Cook 输入。 |
| `engine_integration` | `luau_story_cutover` | 剧情已转换为 Luau 段落；Cook、Player、UI 和媒体的集成验收由正常产品路径执行。 |
| `S3-FLAGSHIP-DEMO-01` | `IN_PROGRESS` | Cook evidence 不能替代 真实平台 Runtime/Player/UI 验证。 |

完整状态见 [STATUS.md](STATUS.md)，迁移边界见 [NativeVN Flagship Demo Migration](../../Docs/migrations/nativevn-flagship-demo-migration.md)。

运行时媒体统一绑定现有 205 份 WAV master 和 `rain-signal-loop.mp4`。逻辑 asset ID、显式 decode provider 与许可保持不变；OGG/WebM 保留为来源资产，不进入本项目的运行时 sidecar。当前契约没有按平台选择候选格式或解码失败后切换的行为。

## Cook

```sh
python Tools/NativeVN/build_nativevn_project.py
cargo run -p roukaku-cli -- cook Examples/NativeVN/project.yaml --profile advanced-vn --target nativevn-flagship-game --out .tmp/nativevn-flagship-cook
```

该命令只证明 source、UI、localization、asset sidecar、provider binding descriptor 与 package section 能被真实 Cook 主路径接受。Windows/Linux/macOS/Android 输入、画面变化、音频 meter、路线、save/load 和同 run identity 仍留给后续运行验收。

## 新架构接续

项目已迁到 platform host profile v3，明确区分 Kira mixer 与平台 output，并补齐音频缓存/块大小。目标覆盖 Windows、Linux、macOS 与 Android；列入配置不代表设备运行已验证。`advanced-vn` 的系统页绑定与 policy 已对齐，允许存读档、设置和历史页。

target manifest v3 已移除 gameplay/UI provider 元数据字段；NativeVN 和 Yakui 由产品静态组合。修改后的项目必须重新 Cook，旧 target/policy schema 直接拒绝。

## 雨中的片刻

[experience.luau](Scripts/experience.luau) 是模块 `story.main` 的默认连续片段：雨夜站台读字、林瑶走近、周衡在前景等待、镜头推进、信号变暗与恢复，再到可保存的重逢。背景、人物、前景、影片、效果与文字有独立深度层；位移与镜头使用声明式多轨片段，透明度使用独立片段并显式替换/取消。结束处可听雨看完整影片，或返回选择页进入旧故事。标题页也能重进“雨中的片刻”。

[main.luau](Scripts/main.luau) 保留 `story.signal_in_the_glass_rain` 的十五个段落、原对白本地化 key、配音和三条路线。[system.luau](Scripts/system.luau) 提供 `story.system` 系统页与 UI 动作。每个模块返回 `paragraphs`；跨段落使用 `rk.enter`，默认对白、选择、菜单和舞台动作调用 `roukaku.vn.*` 模块。

`Scripts/*.luau` 是剧情和演出的唯一作者源；`Narrative/` 是旧内容批次的素材/配音来源记录，不负责重建当前场景。阻塞渐变与影片等待使用宿主完成的明确 fence，不把按键当作媒体结束。保存从段落入口重放变量和场景，渐变期间的存读档行为仍由实际运行验收。没有生成新语音或图片。

可生成连续片段的物理输入，然后交给同树 `roukaku-headless run --gpu` 执行：

```bash
python Tools/NativeVN/experience_inputs.py --output .tmp/nativevn-reading.jsonl
target/debug/roukaku-headless validate-input --input .tmp/nativevn-reading.jsonl
```

输入只发送 Resume/Focus、物理 Enter、只读状态等待、checkpoint 和 shutdown，不直接推进剧情或选择答案。该路径检查阅读→并行位移/镜头→替换/取消→可保存时刻，不替代影片、实际写槽/读档、可听音频、完整路线或冷启动恢复。
