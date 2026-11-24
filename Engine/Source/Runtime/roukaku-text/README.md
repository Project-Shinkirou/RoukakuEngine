# roukaku-text

本库从调用方提供的字体字节建立字库，用 cosmic-text 做 shaping、fallback 和排版，再用 Swash 栅格化字形。Engine 通过 roukaku-media 使用这套实现；独立调用方可以直接依赖本 crate，无需创建 RuntimeWorld、package、registry 或 Headless session。

## 使用与所有权

调用方持有 `CosmicTextLayoutProvider`，用 `new(context, fonts, config)` 传入字体。`PackagedFont` 保留既有名称，但它只持有调用方提供的字体字节、SHA-256、face index、license、coverage 和 target/profile 声明，不读取 package 或磁盘。`FontBindingContext` 的 target/profile 用来校验字体 eligibility；独立调用方同样需要显式提供匹配值。

`TextLayoutRequest` 指定正文、语言、方向、OpenType features、约束和有序字体链。`layout_shared` 返回缓存中的 `Arc<TextLayoutResult>`，包含实际 shaped glyph、bitmap、行与 cluster/source range。结果可直接交给自有绘制器；需要 Scene2D 命令时使用 `TextRenderResourceOwner` 管理 upload/draw/release。`shutdown` 返回资源释放命令，调用方负责提交到绘制器。

## 输出密度

`shape_shared(&request)` 缓存逻辑行断点、cluster、advance 和 ruby 位置，不生成位图。需要固定字格、对齐或逐字显示时，按定位策略缓存一份逻辑布局；只在策略改变时用 `Arc::make_mut` 和 `LogicalTextLayout::place_glyphs` 更新。回调可改逻辑 `x`、`y`、`advance`，返回 `None` 时隐藏当前字形；行断点和 ruby 元数据不变。

`rasterize_shared(&logical, scale, physical_offset)` 先按 `scale`（每逻辑单位的输出像素数）缩放，再加物理像素偏移，最后选择 X/Y 相位。结果中的位图、绘制坐标和 clip 使用物理像素，其余度量保持逻辑单位。已传入最终偏移时，提交给 `TextRenderResourceOwner` 的平移取 `(0, 0)`，再由调用方的绘制变换映回逻辑舞台。`layout_shared` 使用同一路径，等价于密度 `1.0`、偏移 `(0.0, 0.0)`。

栅格布局缓存按逻辑 revision、密度和两个物理偏移区分；字形资源按字体内容、face、glyph、物理字号、X/Y 相位、weight 和 raster flags 区分。整像素平移可以复用同一位图。更换字库后应重新取得逻辑布局。

字体安装、替换与卸载保留 hash 校验、事务失败和缓存失效语义；空字库、缺失字体、非法 coverage、无效 fallback 和预算超限返回既有 `ROUKAKU_TEXT_*` diagnostic。字体 bytes 只通过显式 API 输入；本 crate 不增加文件/网络权限入口。库只发 tracing 事件，不初始化 sink；日志 category 为 `roukaku_text`。

## 依赖边界

直接依赖 `roukaku-core`（Hash256/Diagnostic 等值类型）、`roukaku-media-core`（GlyphBitmap、SceneCommand 和 MediaError）、`roukaku-worker-budget`（进程级有界 CPU 工作预算），以及 cosmic-text、serde、schemars、tracing、unicode-segmentation。media-core 还依赖 roukaku-byte-source，并包含 CPU renderer/filter executor，因此这一阶段仍有媒体 contract/CPU executor 的链接耦合；未把它声称为完全独立的最小字体算法库。

普通与测试依赖均不引入 roukaku-runtime、roukaku-package、roukaku-asset、roukaku-media 或 Headless server。现有 worker budget 仍使用全局 broker，并非新建 registry。未来是否拆分 media-core 由实际共享绘制消费者决定。

## Engine 迁移

`roukaku-media` re-export 这里拥有的文字类型，保持 `roukaku_media::TextLayoutRequest` 等类型身份。验证 package/VFS 并加载字体的代码仍归 roukaku-media：

- `CosmicTextLayoutProvider::from_package(...)` 改为 `roukaku_media::text_layout_from_package(...)`。
- `CosmicTextLayoutProvider::from_package_with_crypto(...)` 改为 `roukaku_media::text_layout_from_package_with_crypto(...)`。

package gate、Player 和测试调用方同步迁移；验证逻辑和返回类型不变。旧文字 replay 功能保留在 roukaku-media，未进入 SDK。此重构不修改 save/package 格式，也不增加兼容 provider 或动态 ABI。

## 验证

```bash
cargo test -p roukaku-text
cargo clippy -p roukaku-text --all-targets -- -D warnings
cargo test -p roukaku-media --test text_layout
```

原文字行为测试迁到本 crate，覆盖真实授权字体的 CJK/Arabic/emoji shaping、有序 fallback、竖排/ruby、wrap/ellipsis、glyph bitmap、资源事务/释放、缓存复用、字体替换和并发 single-flight。普通 Rust 测试直接运行，不启动 Headless。package 与 replay 的集成测试留在 roukaku-media。此验证证明库行为保留，不代表四平台产品或 Musica 完成验收。
