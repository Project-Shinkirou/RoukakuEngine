# TsuiNoSora NativeVN UI template

该目录只包含非商业的 RoukakuVN View、Controller、Theme token、系统文案和 profile contract。生成器会把它复制到 ignored 私有 NativeVN 项目；原作正文与媒体不会进入仓库。

`classic` 与 `modern` 共用同一 `RuntimeWorld`。两者都采用 800×600 严格 letterbox；`classic` 只提供原作阅读和右键存读档入口，`modern` 额外提供 Quick Panel、Backlog、Auto、Skip 与 Config。本次明确不包含 Remake 立绘、Gallery、Replay、Route Chart 和 Voice Replay。

[Scripts/system.luau](Scripts/system.luau) 是系统剧情模块 `story.system`。`paragraphs` 保留七个系统页入口，`actions` 保留原有 UI 动作 ID；mode/panty 使用显式注册变量，导演跳转使用 `rk.enter`。导演模块及原作内容由私有项目提供，模板不补造正文。菜单由 `roukaku.vn.menu` 执行，布局与主题不属于剧情模块。
