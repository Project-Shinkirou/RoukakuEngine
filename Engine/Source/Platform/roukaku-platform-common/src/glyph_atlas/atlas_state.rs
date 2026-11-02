use super::*;

pub(super) const ATLAS_SIDE: u32 = 4096;

pub(super) const MAX_ATLAS_WIDTH: u32 = 8192;

pub(super) const MIN_ATLAS_SIDE: u32 = 1024;

pub(super) const ATLAS_PADDING: u32 = 1;

pub(super) const MAX_SCENE_RESOURCES: usize = 65_536;

/// Retained resource bytes may fill this many maximum-size atlas pages; a set
/// that overflows one page spills onto further pages.
pub(super) const MAX_ATLAS_PAGE_BUDGET: usize = 4;

/// Packing padding and shelf fragmentation can need more pages than the raw
/// byte budget; this caps the physical page count.
pub(super) const MAX_ATLAS_PAGES: usize = MAX_ATLAS_PAGE_BUDGET * 2;

pub(super) const MAX_SCENE_RESOURCE_BYTES: usize =
    MAX_ATLAS_PAGE_BUDGET * MAX_ATLAS_WIDTH as usize * ATLAS_SIDE as usize * 4;

pub(super) const MAX_ATLAS_UPLOAD_BYTES: usize = ATLAS_SIDE as usize * ATLAS_SIDE as usize * 4;

pub(super) const ATLAS_STAGING_RING_SIZE: usize = 1;

pub(super) const ATLAS_STAGING_CHUNK_BYTES: u64 = 4 * 1024 * 1024;

pub(super) const VERTEX_STRIDE: wgpu::BufferAddress = 52;

pub(crate) struct WgpuGlyphAtlasRenderer {
    pub(super) resources: BTreeMap<String, AtlasResource>,
    pub(super) atlas: Option<GpuAtlas>,
    pub(super) layout: wgpu::BindGroupLayout,
    pub(super) linear_sampler: wgpu::Sampler,
    pub(super) nearest_sampler: wgpu::Sampler,
    pub(super) pipelines: BlendPipelines,
    pub(super) default_compositing: SceneCompositing2D,
    pub(super) output: Option<CachedOutput>,
    pub(super) vertex_buffer: Option<wgpu::Buffer>,
    pub(super) vertex_capacity: u64,
    pub(super) last_upload_bytes: u64,
    pub(super) last_draw_calls: u64,
    pub(super) last_engine_allocation_bytes: u64,
    pub(super) last_engine_allocation_count: u64,
    pub(super) last_command_allocation_bytes: u64,
    pub(super) last_command_allocation_breakdown: Option<[u64; 4]>,
    pub(super) command_allocation_diagnostics_emitted: u32,
    pub(super) last_atlas_allocation_bytes: u64,
    pub(super) last_geometry_allocation_bytes: u64,
    pub(super) vertex_bytes: Vec<u8>,
    pub(super) uploaded_vertex_bytes: Vec<u8>,
    pub(super) draw_batches: Vec<DrawBatch>,
    pub(super) atlas_upload_pixels: Vec<u8>,
    pub(super) atlas_staging_belts: [wgpu::util::StagingBelt; ATLAS_STAGING_RING_SIZE],
    pub(super) atlas_staging_index: usize,
    pub(super) reserved_side: Option<u32>,
    /// Lazily sized depth attachment for depth-tested projective meshes.
    pub(super) depth_target: Option<DepthTarget>,
}

pub(crate) struct CachedOutput {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) compositing: SceneCompositing2D,
    pub(super) texture: wgpu::Texture,
}

/// Every semantic scene command shares the atlas and vertex format; only its
/// compositor equation differs. Keeping these pipelines adjacent prevents a
/// non-alpha legacy blend from falling back to a CPU framebuffer.
pub(crate) struct BlendPipelines {
    pub(super) linear: BlendPipelineSet,
    pub(super) encoded: BlendPipelineSet,
    pub(super) projective: ProjectivePipelines,
    pub(super) layout: wgpu::PipelineLayout,
    pub(super) shader: wgpu::ShaderModule,
}

pub(crate) struct BlendPipelineSet {
    pub(super) alpha: wgpu::RenderPipeline,
    pub(super) opaque: wgpu::RenderPipeline,
    pub(super) add: wgpu::RenderPipeline,
    pub(super) multiply: wgpu::RenderPipeline,
    pub(super) screen: wgpu::RenderPipeline,
    pub(super) screen_rgb: wgpu::RenderPipeline,
    pub(super) mask_gradient: wgpu::RenderPipeline,
    pub(super) mask_threshold: wgpu::RenderPipeline,
    pub(super) crossfade: wgpu::RenderPipeline,
}

impl BlendPipelines {
    pub(super) fn select(
        &self,
        compositing: SceneCompositing2D,
        blend: BlendMode,
    ) -> &wgpu::RenderPipeline {
        let pipelines = match compositing {
            SceneCompositing2D::LinearSrgb => &self.linear,
            SceneCompositing2D::EncodedSrgb => &self.encoded,
        };
        match blend {
            BlendMode::Alpha => &pipelines.alpha,
            BlendMode::Opaque => &pipelines.opaque,
            BlendMode::Add => &pipelines.add,
            BlendMode::Multiply => &pipelines.multiply,
            BlendMode::Screen => &pipelines.screen,
            BlendMode::ScreenRgb => &pipelines.screen_rgb,
        }
    }

    pub(super) fn select_batch(&self, batch: &DrawBatch) -> &wgpu::RenderPipeline {
        let pipelines = match batch.compositing {
            SceneCompositing2D::LinearSrgb => &self.linear,
            SceneCompositing2D::EncodedSrgb => &self.encoded,
        };
        match batch.pipeline {
            BatchPipeline::Blend(blend) => self.select(batch.compositing, blend),
            BatchPipeline::Mask(MaskRevealMode::Gradient) => &pipelines.mask_gradient,
            BatchPipeline::Mask(MaskRevealMode::Threshold) => &pipelines.mask_threshold,
            BatchPipeline::Crossfade => &pipelines.crossfade,
            BatchPipeline::Projective(blend, depth) => {
                self.select_projective(batch.compositing, blend, depth)
            }
        }
    }
}

pub(crate) struct PreparedGlyphFrame {
    pub(crate) texture: wgpu::Texture,
    pub(crate) cpu_profile: GlyphCpuProfile,
    pub(super) resource_mutations: ResourceMutationJournal,
    pub(super) atlas_update: PreparedAtlasUpdate,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct GlyphCpuProfile {
    pub(crate) command_ns: u64,
    pub(crate) atlas_ns: u64,
    pub(crate) geometry_ns: u64,
    pub(crate) vertex_upload_ns: u64,
    pub(crate) render_encode_ns: u64,
    pub(crate) queue_submit_ns: u64,
    pub(crate) render_submit_ns: u64,
}

pub(crate) struct GlyphProfileQueries<'a> {
    pub(crate) query_set: &'a wgpu::QuerySet,
    pub(crate) atlas_upload: Option<(u32, u32)>,
    pub(crate) scene: (u32, u32),
}

pub(crate) struct AtlasUpdateContext<'a> {
    pub(super) device: &'a wgpu::Device,
    pub(super) queue: &'a wgpu::Queue,
    pub(super) layout: &'a wgpu::BindGroupLayout,
    pub(super) linear_sampler: &'a wgpu::Sampler,
    pub(super) nearest_sampler: &'a wgpu::Sampler,
    pub(super) staging_belt: &'a mut wgpu::util::StagingBelt,
    pub(super) timestamp_query: Option<(&'a wgpu::QuerySet, u32, u32)>,
}

#[derive(Clone, Copy)]
pub(crate) struct TextureRegionUpload<'a> {
    pub(super) resource_id: &'a str,
    pub(super) texture_width: u32,
    pub(super) texture_height: u32,
    pub(super) x: u32,
    pub(super) y: u32,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) rgba8: &'a [u8],
}

pub(super) type PreparedRender = (
    wgpu::Texture,
    ResourceMutationJournal,
    PreparedAtlasUpdate,
    GlyphCpuProfile,
);

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum AtlasResource {
    Glyph(Arc<GlyphBitmap>),
    Texture(Arc<RetainedTexture>),
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct RetainedTexture {
    pub(super) base: Arc<TextureFrame>,
    pub(super) patches: Vec<RetainedTexturePatch>,
    pub(super) patch_bytes: usize,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct RetainedTexturePatch {
    pub(super) x: u32,
    pub(super) y: u32,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) rgba8: OwnedPixelBuffer,
}

pub(super) type ResourceMutationJournal = BTreeMap<String, Option<AtlasResource>>;

pub(crate) struct AtlasResourceView<'a> {
    pub(super) base: &'a BTreeMap<String, AtlasResource>,
    pub(super) mutations: &'a ResourceMutationJournal,
}

impl<'a> AtlasResourceView<'a> {
    pub(super) fn new(
        base: &'a BTreeMap<String, AtlasResource>,
        mutations: &'a ResourceMutationJournal,
    ) -> Self {
        Self { base, mutations }
    }

    pub(super) fn get(&self, resource_id: &str) -> Option<&AtlasResource> {
        match self.mutations.get(resource_id) {
            Some(resource) => resource.as_ref(),
            None => self.base.get(resource_id),
        }
    }

    pub(super) fn contains_key(&self, resource_id: &str) -> bool {
        self.get(resource_id).is_some()
    }

    pub(super) fn len(&self) -> usize {
        self.base.len()
            + self
                .mutations
                .iter()
                .filter(|(id, resource)| resource.is_some() && !self.base.contains_key(*id))
                .count()
            - self
                .mutations
                .iter()
                .filter(|(id, resource)| resource.is_none() && self.base.contains_key(*id))
                .count()
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (&String, &AtlasResource)> {
        self.base
            .iter()
            .filter(|(id, _)| !self.mutations.contains_key(*id))
            .chain(
                self.mutations
                    .iter()
                    .filter_map(|(id, resource)| resource.as_ref().map(|resource| (id, resource))),
            )
    }

    pub(super) fn values(&self) -> impl Iterator<Item = &AtlasResource> {
        self.iter().map(|(_, resource)| resource)
    }
}

/// GPU side of a packed atlas: one texture per page. Every page reserves its
/// origin texel as opaque white, so untextured primitives can be drawn with
/// whichever page is bound.
pub(crate) struct GpuAtlas {
    pub(super) textures: Vec<wgpu::Texture>,
    /// `pages * pages` entries indexed by `page * pages + mask_page`: binding 0
    /// samples `page` and the mask binding samples `mask_page`.
    pub(super) bind_groups: Vec<AtlasBindGroups>,
    pub(super) packed: PackedAtlas,
}

impl GpuAtlas {
    pub(super) fn bind_group(
        &self,
        page: usize,
        mask_page: usize,
        compositing: SceneCompositing2D,
        filter: TextureFilter2D,
    ) -> &wgpu::BindGroup {
        self.bind_groups[page * self.textures.len() + mask_page].select(compositing, filter)
    }
}

pub(crate) struct AtlasBindGroups {
    pub(super) linear: wgpu::BindGroup,
    pub(super) nearest: wgpu::BindGroup,
    pub(super) encoded_linear: wgpu::BindGroup,
    pub(super) encoded_nearest: wgpu::BindGroup,
}

impl AtlasBindGroups {
    pub(super) fn select(
        &self,
        compositing: SceneCompositing2D,
        filter: TextureFilter2D,
    ) -> &wgpu::BindGroup {
        match (compositing, filter) {
            (SceneCompositing2D::LinearSrgb, TextureFilter2D::Linear) => &self.linear,
            (SceneCompositing2D::LinearSrgb, TextureFilter2D::Nearest) => &self.nearest,
            (SceneCompositing2D::EncodedSrgb, TextureFilter2D::Linear) => &self.encoded_linear,
            (SceneCompositing2D::EncodedSrgb, TextureFilter2D::Nearest) => &self.encoded_nearest,
        }
    }
}

pub(crate) enum PreparedAtlasUpdate {
    None,
    Replace(GpuAtlas),
    Mutate(PackedAtlasMutation),
}

pub(super) type AtlasPlacementJournal = BTreeMap<String, Option<AtlasPlacement>>;

pub(crate) struct PackedAtlasMutation {
    pub(super) placements: AtlasPlacementJournal,
    /// One entry per committed page, in page order.
    pub(super) pages: SmallVec<[AtlasPageMutation; 1]>,
}

pub(crate) struct AtlasPageMutation {
    pub(super) cursor_x: u32,
    pub(super) cursor_y: u32,
    pub(super) row_height: u32,
    pub(super) consumed_free_slots: BTreeSet<usize>,
    pub(super) added_free_slots: Vec<AtlasSlot>,
    pub(super) freed_area: u64,
}

pub(crate) struct AtlasAllocatorState<'a> {
    pub(super) page: usize,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) cursor_x: u32,
    pub(super) cursor_y: u32,
    pub(super) row_height: u32,
    pub(super) base_free_slots: &'a [AtlasSlot],
    pub(super) consumed_free_slots: BTreeSet<usize>,
    pub(super) added_free_slots: Vec<AtlasSlot>,
    pub(super) freed_area: u64,
}

impl<'a> AtlasAllocatorState<'a> {
    pub(super) fn new(page_index: usize, page: &'a AtlasPageAllocation) -> Self {
        Self {
            page: page_index,
            width: page.width,
            height: page.height,
            cursor_x: page.cursor_x,
            cursor_y: page.cursor_y,
            row_height: page.row_height,
            base_free_slots: &page.free_slots,
            consumed_free_slots: BTreeSet::new(),
            added_free_slots: Vec::new(),
            freed_area: page.freed_area,
        }
    }

    pub(super) fn into_mutation(self) -> AtlasPageMutation {
        AtlasPageMutation {
            cursor_x: self.cursor_x,
            cursor_y: self.cursor_y,
            row_height: self.row_height,
            consumed_free_slots: self.consumed_free_slots,
            added_free_slots: self.added_free_slots,
            freed_area: self.freed_area,
        }
    }
}

pub(crate) struct AtlasPlacementView<'a> {
    pub(super) packed: &'a PackedAtlas,
    pub(super) mutations: Option<&'a AtlasPlacementJournal>,
}

impl<'a> AtlasPlacementView<'a> {
    pub(super) fn committed(packed: &'a PackedAtlas) -> Self {
        Self {
            packed,
            mutations: None,
        }
    }

    pub(super) fn pending(packed: &'a PackedAtlas, mutations: &'a AtlasPlacementJournal) -> Self {
        Self {
            packed,
            mutations: Some(mutations),
        }
    }

    pub(super) fn get(&self, resource_id: &str) -> Option<&AtlasPlacement> {
        match self
            .mutations
            .and_then(|mutations| mutations.get(resource_id))
        {
            Some(placement) => placement.as_ref(),
            None => self.packed.placements.get(resource_id),
        }
    }

    pub(super) fn width(&self, page: usize) -> u32 {
        self.packed.pages[page].width
    }

    pub(super) fn height(&self, page: usize) -> u32 {
        self.packed.pages[page].height
    }
}

impl AtlasResource {
    pub(super) fn texture(frame: TextureFrame) -> Self {
        Self::Texture(Arc::new(RetainedTexture {
            base: Arc::new(frame),
            patches: Vec::new(),
            patch_bytes: 0,
        }))
    }

    pub(super) fn width(&self) -> u32 {
        match self {
            Self::Glyph(value) => value.width,
            Self::Texture(value) => value.base.width,
        }
    }

    pub(super) fn height(&self) -> u32 {
        match self {
            Self::Glyph(value) => value.height,
            Self::Texture(value) => value.base.height,
        }
    }

    pub(super) fn byte_len(&self) -> usize {
        match self {
            Self::Glyph(value) => value.pixels.len(),
            Self::Texture(value) => value.base.rgba8.len().saturating_add(value.patch_bytes),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct AtlasPlacement {
    pub(super) page: usize,
    pub(super) x: u32,
    pub(super) y: u32,
    pub(super) width: u32,
    pub(super) height: u32,
}

#[derive(Clone)]
pub(crate) struct PackedAtlas {
    pub(super) placements: BTreeMap<String, AtlasPlacement>,
    /// Never empty; a texture set that fits one page keeps exactly one.
    pub(super) pages: Vec<AtlasPageAllocation>,
}

impl PackedAtlas {
    pub(super) fn byte_len(&self) -> u64 {
        self.pages
            .iter()
            .map(|page| u64::from(page.width) * u64::from(page.height) * 4)
            .sum()
    }
}

#[derive(Clone)]
pub(crate) struct AtlasPageAllocation {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) cursor_x: u32,
    pub(super) cursor_y: u32,
    pub(super) row_height: u32,
    pub(super) free_slots: Vec<AtlasSlot>,
    pub(super) freed_area: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct AtlasSlot {
    pub(super) x: u32,
    pub(super) y: u32,
    pub(super) width: u32,
    pub(super) height: u32,
}

pub(crate) enum QuadSource<'a> {
    Resource {
        resource_id: Cow<'a, str>,
        source: RectI,
    },
    White,
}

pub(crate) struct DrawQuad<'a> {
    pub(super) source: QuadSource<'a>,
    pub(super) destination: RectI,
    pub(super) rotation_quadrants: u8,
}

pub(crate) struct DrawRun<'a> {
    pub(super) pixel_mask: u64,
    // Classic text runs regularly contain more than eight glyphs. Keeping the
    // expected bounded run inline avoids a per-frame growth allocation while
    // retaining an explicit spill path for authored long text.
    pub(super) quads: SmallVec<[DrawQuad<'a>; 32]>,
    pub(super) compositing: SceneCompositing2D,
    pub(super) blend: BlendMode,
    pub(super) rgba: [u8; 4],
    pub(super) opacity: f32,
    pub(super) clip: RectI,
    pub(super) transform: Transform2D,
    /// Image and mask share the atlas but use independent sampling coordinates.
    pub(super) mask: Option<MaskRunParams<'a>>,
}

pub(crate) struct MaskRunParams<'a> {
    pub(super) mode: MaskRevealMode,
    pub(super) phase: f32,
    pub(super) mask_resource: Cow<'a, str>,
}

pub(crate) struct MeshRun<'a> {
    pub(super) pixel_mask: u64,
    pub(super) vertices: &'a [MeshVertex2D],
    pub(super) indices: &'a [u32],
    pub(super) texture_id: Option<&'a str>,
    pub(super) texture_filter: TextureFilter2D,
    pub(super) compositing: SceneCompositing2D,
    pub(super) opacity: f32,
    pub(super) blend: BlendMode,
    pub(super) clip: RectI,
    pub(super) transform: Transform2D,
}

pub(crate) enum DrawPrimitive<'a> {
    Quads(usize),
    Mesh(MeshRun<'a>),
    Crossfade(CrossfadeRun<'a>),
    ProjectiveMesh(ProjectiveMeshRun<'a>),
}
