use super::*;

pub(super) fn push_quad_run<'a>(
    quad_runs: &mut SmallVec<[DrawRun<'a>; 64]>,
    primitives: &mut SmallVec<[DrawPrimitive<'a>; 64]>,
    run: DrawRun<'a>,
) {
    let index = quad_runs.len();
    quad_runs.push(run);
    primitives.push(DrawPrimitive::Quads(index));
}

/// Fixed-function state a batch draws with.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchPipeline {
    Blend(BlendMode),
    Mask(MaskRevealMode),
    /// Group 0 binds `page` (previous texture), group 1 `secondary_page`.
    Crossfade,
    /// 48-byte homogeneous vertices; depth batches need a depth attachment.
    Projective(BlendMode, MeshDepth2D),
}

impl BatchPipeline {
    pub(super) fn depth_tested(self) -> bool {
        matches!(self, Self::Projective(_, MeshDepth2D::ReadWriteLess))
    }
}

#[derive(Clone, Copy)]
pub(crate) struct DrawBatch {
    /// Byte offset of the first vertex; batches mix vertex strides.
    pub(super) byte_offset: u64,
    pub(super) vertex_count: u32,
    pub(super) clip: RectI,
    pub(super) texture_filter: TextureFilter2D,
    pub(super) compositing: SceneCompositing2D,
    pub(super) pipeline: BatchPipeline,
    /// Atlas page sampled by the image binding.
    pub(super) page: usize,
    /// Atlas page sampled by the secondary (mask) binding; equals `page`
    /// unless the batch samples a second resource.
    pub(super) secondary_page: usize,
}

impl WgpuGlyphAtlasRenderer {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        Self::new_internal(device, None)
    }

    pub(crate) fn new_reserved(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        side: u32,
    ) -> Result<Self, PlatformError> {
        if !(MIN_ATLAS_SIDE..=ATLAS_SIDE).contains(&side) || !side.is_power_of_two() {
            return Err(invalid(
                "glyph atlas reservation side is outside the supported range",
            ));
        }
        let mut renderer = Self::new_internal(device, Some(side));
        renderer.atlas = Some(create_reserved_gpu_atlas(
            device,
            queue,
            &renderer.layout,
            &renderer.linear_sampler,
            &renderer.nearest_sampler,
            side,
        ));
        Ok(renderer)
    }

    pub(super) fn new_internal(device: &wgpu::Device, reserved_side: Option<u32>) -> Self {
        let (layout, linear_sampler, nearest_sampler, pipelines) = create_pipelines(device);
        Self {
            resources: BTreeMap::new(),
            atlas: None,
            layout,
            linear_sampler,
            nearest_sampler,
            pipelines,
            default_compositing: SceneCompositing2D::LinearSrgb,
            output: None,
            vertex_buffer: None,
            vertex_capacity: 0,
            last_upload_bytes: 0,
            last_draw_calls: 0,
            last_engine_allocation_bytes: 0,
            last_engine_allocation_count: 0,
            last_command_allocation_bytes: 0,
            last_command_allocation_breakdown: None,
            command_allocation_diagnostics_emitted: 0,
            last_atlas_allocation_bytes: 0,
            last_geometry_allocation_bytes: 0,
            vertex_bytes: Vec::new(),
            uploaded_vertex_bytes: Vec::new(),
            draw_batches: Vec::new(),
            atlas_upload_pixels: Vec::with_capacity(ATLAS_STAGING_CHUNK_BYTES as usize),
            atlas_staging_belts: std::array::from_fn(|_| primed_staging_belt(device)),
            atlas_staging_index: 0,
            reserved_side,
            depth_target: None,
        }
    }

    pub(in super::super) fn set_default_compositing(&mut self, compositing: SceneCompositing2D) {
        self.default_compositing = compositing;
    }

    pub(in super::super) fn recover(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let (layout, linear_sampler, nearest_sampler, pipelines) = create_pipelines(device);
        self.layout = layout;
        self.linear_sampler = linear_sampler;
        self.nearest_sampler = nearest_sampler;
        self.pipelines = pipelines;
        self.atlas = self.reserved_side.map(|side| {
            create_reserved_gpu_atlas(
                device,
                queue,
                &self.layout,
                &self.linear_sampler,
                &self.nearest_sampler,
                side,
            )
        });
        self.output = None;
        self.depth_target = None;
        self.vertex_buffer = None;
        self.vertex_capacity = 0;
        self.uploaded_vertex_bytes.clear();
        self.atlas_staging_belts = std::array::from_fn(|_| primed_staging_belt(device));
        self.atlas_staging_index = 0;
        self.last_upload_bytes = 0;
        self.last_draw_calls = 0;
        self.last_engine_allocation_bytes = 0;
        self.last_engine_allocation_count = 0;
        self.last_command_allocation_bytes = 0;
        self.last_command_allocation_breakdown = None;
        self.command_allocation_diagnostics_emitted = 0;
        self.last_atlas_allocation_bytes = 0;
        self.last_geometry_allocation_bytes = 0;
    }

    pub(crate) fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &SceneFrame,
    ) -> Result<PreparedGlyphFrame, PlatformError> {
        let (texture, resource_mutations, atlas_update, cpu_profile) =
            self.render_internal(device, queue, frame, true, None, None)?;
        Ok(PreparedGlyphFrame {
            texture,
            cpu_profile,
            resource_mutations,
            atlas_update,
        })
    }

    pub(crate) fn render_profiled(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &SceneFrame,
        queries: GlyphProfileQueries<'_>,
    ) -> Result<PreparedGlyphFrame, PlatformError> {
        let (texture, resource_mutations, atlas_update, cpu_profile) = self.render_internal(
            device,
            queue,
            frame,
            true,
            queries
                .atlas_upload
                .map(|(begin, end)| (queries.query_set, begin, end)),
            Some((queries.query_set, queries.scene.0, queries.scene.1)),
        )?;
        Ok(PreparedGlyphFrame {
            texture,
            cpu_profile,
            resource_mutations,
            atlas_update,
        })
    }

    pub(in super::super) fn render_retained(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &SceneFrame,
    ) -> Result<wgpu::Texture, PlatformError> {
        self.render_internal(device, queue, frame, false, None, None)
            .map(|(texture, _, _, _)| texture)
    }

    pub(crate) fn commit(&mut self, prepared: PreparedGlyphFrame) -> wgpu::Texture {
        for (resource_id, resource) in prepared.resource_mutations {
            match resource {
                Some(resource) => {
                    self.resources.insert(resource_id, resource);
                }
                None => {
                    self.resources.remove(&resource_id);
                }
            }
        }
        match prepared.atlas_update {
            PreparedAtlasUpdate::None => {}
            PreparedAtlasUpdate::Replace(atlas) => self.atlas = Some(atlas),
            PreparedAtlasUpdate::Mutate(mutation) => {
                let atlas = self
                    .atlas
                    .as_mut()
                    .expect("incremental atlas mutation requires a committed atlas");
                for (resource_id, placement) in mutation.placements {
                    match placement {
                        Some(placement) => {
                            atlas.packed.placements.insert(resource_id, placement);
                        }
                        None => {
                            atlas.packed.placements.remove(&resource_id);
                        }
                    }
                }
                for (page, mutation) in atlas.packed.pages.iter_mut().zip(mutation.pages) {
                    page.cursor_x = mutation.cursor_x;
                    page.cursor_y = mutation.cursor_y;
                    page.row_height = mutation.row_height;
                    for index in mutation.consumed_free_slots.into_iter().rev() {
                        page.free_slots.remove(index);
                    }
                    page.free_slots.extend(mutation.added_free_slots);
                    page.free_slots.sort_by_key(atlas_slot_sort_key);
                    page.freed_area = mutation.freed_area;
                }
            }
        }
        prepared.texture
    }

    pub(crate) fn resource_bytes(&self) -> u64 {
        let output = self.output.as_ref().map_or(0, |output| {
            u64::from(output.width) * u64::from(output.height) * 4
        });
        let depth = self.depth_target.as_ref().map_or(0, DepthTarget::bytes);
        self.atlas_bytes() + output + depth + self.vertex_capacity
    }

    pub(crate) fn atlas_bytes(&self) -> u64 {
        self.atlas
            .as_ref()
            .map_or(0, |atlas| atlas.packed.byte_len())
    }

    pub(crate) fn last_upload_bytes(&self) -> u64 {
        self.last_upload_bytes
    }

    pub(crate) fn last_draw_calls(&self) -> u64 {
        self.last_draw_calls
    }

    pub(crate) fn last_engine_allocation_bytes(&self) -> u64 {
        self.last_engine_allocation_bytes
    }

    pub(crate) fn last_engine_allocation_count(&self) -> u64 {
        self.last_engine_allocation_count
    }

    pub(crate) fn last_command_allocation_bytes(&self) -> u64 {
        self.last_command_allocation_bytes
    }

    pub(crate) fn last_atlas_allocation_bytes(&self) -> u64 {
        self.last_atlas_allocation_bytes
    }

    pub(crate) fn last_geometry_allocation_bytes(&self) -> u64 {
        self.last_geometry_allocation_bytes
    }

    pub(crate) fn requires_atlas_update(&self, frame: &SceneFrame) -> bool {
        self.atlas.is_none()
            || walk::mask_reveal_resources_changed(&self.resources, frame)
            || frame.commands.iter().any(|command| {
                matches!(
                    command,
                    SceneCommand::UploadTexture { .. }
                        | SceneCommand::UpdateTextureRegion { .. }
                        | SceneCommand::UploadGlyph { .. }
                        | SceneCommand::ReleaseResource { .. }
                        | SceneCommand::Texture { .. }
                        | SceneCommand::VideoFrame { .. }
                        | SceneCommand::Glyph { .. }
                )
            })
    }
}
