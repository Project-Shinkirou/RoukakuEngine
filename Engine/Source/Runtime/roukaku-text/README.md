# roukaku-text

项目字体字节驱动的共享文字布局。`PangoTextLayoutProvider` 通过 Pango FT2、HarfBuzz、FriBidi、FreeType 和独立 Fontconfig 字体表完成 shaping、双向排版及字形栅格化。Engine 通过 roukaku-media 使用同一实现，独立消费者不需要 RuntimeWorld、package 或 registry。

## 使用与所有权

调用方用 `new(context, fonts, config)` 传入 `PackagedFont`，包括字节、SHA-256、必填的 `source_mount`、face index、license、coverage 和 target/profile 声明。VFS 消费者传入实际 mount identity，独立内存消费者指定自己的命名空间（如 `memory`）。provider 校验描述及字体元数据，各 worker 将字体写入独占临时目录，供 Fontconfig 延迟读取；目录随字体表销毁，不注册系统字体，也不读取用户字体目录。

`TextLayoutRequest` 指定正文、语言、方向、OpenType features、约束及有序字体链。回退按 grapheme 和 coverage 选择，Pango 返回的实际 face 仍需通过声明校验。`layout_shared` 返回缓存中的 `Arc<TextLayoutResult>`，包含实际 glyph、bitmap、行及 cluster/source byte range。UI 测量和绘制复用该结果；`TextRenderResourceOwner` 管理 Scene2D upload/draw/release。`shutdown` 返回需要提交的释放命令。

Ruby 组合位于 `ruby` 模块。Pango 禁止在被注音正文内部断行，Roukaku 将独立排版的注音中心对齐到正文。竖排通过 Pango EAST gravity 选择纵向字形和标点，再映射到列坐标；短数字组采用 SOUTH gravity 保持横排。RTL 和混排由 Pango/FriBidi 决定视觉顺序，保留原文 UTF-8 cluster 范围。

字体增删、替换会清空缓存。字体字节在 worker 间共享，字形资源键包含 mount identity 与内容 hash；即使字节相同，换挂载来源也会重建资源身份。每个 worker 独占 native font map，按字体、glyph、字号及 Pango gravity 缓存位图，并沿用进程级 WorkerBudgetBroker 和同请求 single-flight；没有全局 Pango font map 或 Fontconfig 配置。

## 原生依赖

运行 `cargo xtask deps --target <triple> --text`，将 `ROUKAKU_TEXT_VCPKG_ROOT` 指向 `.work/vcpkg-subtitle/installed-<triplet>`，并配置 `<triplet>/bin` 动态库搜索路径。文字和字幕共用固定 vcpkg `0c2a9ace1964b818b15f7ae50ba1c42404918a7b`，包括 Pango 1.58、HarfBuzz、FriBidi、FreeType、Fontconfig 和 libass 0.17.5。非 Windows 也支持 pkg-config 发现 `pangoft2 >= 1.50`。

## 依赖边界

直接使用 roukaku-core 的值类型、roukaku-media-core 的 GlyphBitmap/SceneCommand/MediaError、roukaku-worker-budget 的进程级 CPU 预算，以及字体元数据解析、serde、schemars、tracing、unicode-segmentation。package 字体装载入口为 `roukaku_media::text_layout_from_package` 和 `text_layout_from_package_with_crypto`。package 和 replay 逻辑仍由 roukaku-media 负责。
