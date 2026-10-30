# RoukakuVN Advanced Presentation Sample

本页保留 advanced presentation 的设计参考；Cook 入口已经合并到 `Examples/NativeVN`。旗舰项目的三条路线展示多层舞台、camera、video layer、shader/filter、voice sync、复杂 text effect、system story、system UI 和 release gate 的组合方式；普通商业 VN 发布不需要启用该 profile。

## Files

| 文件 | 用途 |
| --- | --- |
| [project.yaml](project.yaml) / `Examples/NativeVN/project.yaml` | Game target、profile 和 scenario refs |
| [main.luau](main.luau) / `Examples/NativeVN/Scripts/main.luau` | Luau 段落、多层舞台、camera、video、voice、choice 和 system story |
| [system.luau](system.luau) | `story.system` 系统页段落，调用默认 Luau 菜单模块 |
| [advanced_policy.luau](advanced_policy.luau) | Timeline、fallback、shader/filter 和 text effect 策略参考 |

## Gate

```bash
roukaku cook Examples/NativeVN/project.yaml --profile advanced-vn --target nativevn-game --out target/nativevn-advanced-cooked
roukaku package build target/nativevn-advanced-cooked --target nativevn-game --out target/nativevn-advanced.roukakupkg
roukaku package validate target/nativevn-advanced.roukakupkg --profile advanced-vn --target nativevn-game --report target/reports/advanced-release.yaml
```

上述命令只覆盖 Cook/package 静态路径，不代表实际演出、输入、声音或路线通过。旧 YAML 执行样例不再保留，真实验证使用 NativeVN session 与对应物理输入。

Expected report fields: `vn.commercial_baseline`, `vn.system_ui_profile`, `vn.advanced_presentation`, `timeline.join_cancel`, `voice.sync`, `presentation.fallback`, `renderer.effect_budget` and player route checks.

剧情模块返回 `paragraphs`；`rk.enter` 切换段落并建立新的存档入口。舞台调用 `roukaku.vn.stage`，数值与时长沿用原资产契约；音频结束等待由宿主完成对应 fence，物理推进不会冒充媒体结束。
