# RoukakuVN Script Sample

本目录展示 RoukakuVN 脚本机制/策略分离的产品级样例。当前真实 package/player gate 使用 `Examples/NativeVN`；本目录保留为文档样例，不作为 release evidence。

| 文件 | 内容 |
| --- | --- |
| [project.yaml](project.yaml) | 项目入口、Game target、策略包、系统页、本地化和发布锁定策略 |
| [main.luau](main.luau) | `story.main` 段落入口、对白、选择、变量和并行演出 |
| [system.luau](system.luau) | `story.system` 的 title/config/gallery/replay/chart 段落 |
| [standard_policy.luau](standard_policy.luau) | 官方标准策略包形态 |
| [cinematic_policy.luau](cinematic_policy.luau) | 第三方复杂演出策略包形态 |

验证文档链接：

```bash
python Tools/check_docs.py
```

旧 YAML 执行样例已删除。保留的源码用于说明业务语义，不等于可执行产品包；实际验证使用真实 VN session、Rust tests 与物理输入流程，见[开发手册](../../manual/development.md)。

剧情只以 Luau 为作者源。每个模块返回 `paragraphs`，跨段落使用 `rk.enter` 建立存档重放边界；对白、选择和菜单调用 `roukaku.vn.*` 默认模块。`ending.good` 与 `ending.normal` 沿用原样例的外部目标，项目必须提供对应段落，运行时不会补造结局。
