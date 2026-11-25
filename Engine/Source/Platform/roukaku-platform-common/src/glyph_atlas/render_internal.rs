use super::*;

impl WgpuGlyphAtlasRenderer {
    pub(super) fn render_internal(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &SceneFrame,
        apply_mutations: bool,
        target: Option<&mut WgpuSceneTarget>,
        queries: Option<GlyphProfileQueries<'_>>,
    ) -> Result<PreparedRender, PlatformError> {
        let profile_cpu = queries.is_some();
        let atlas_upload_query = queries.as_ref().and_then(|queries| {
            queries
                .atlas_upload
                .map(|(begin, end)| (queries.query_set, begin, end))
        });
        let timestamp_query =
            queries.map(|queries| (queries.query_set, queries.scene.0, queries.scene.1));
        let command_started = Instant::now();
        let engine_allocation_before = roukaku_observability::thread_allocation_snapshot();
        validate_scene_resource_payloads(&frame.commands)?;
        let resource_validation_allocation = roukaku_observability::thread_allocation_snapshot();
        let mut walked = walk::WalkedScene::new(self.atlas.is_none(), self.default_compositing);
        let command_setup_allocation = roukaku_observability::thread_allocation_snapshot();
        walked.consume(&self.resources, frame, apply_mutations)?;
        let walk::WalkedScene {
            render_mutations,
            committed_mutations,
            resources_changed,
            quad_runs,
            draw_runs,
            run_ids,
            drawn_resources,
            upload_texture_count,
            texture_region_uploads,
            upload_glyph_count,
            release_resource_count,
            transient_texture_count,
            transient_glyph_count,
            mesh_command_allocation_bytes,
            mesh_command_allocation_count,
            has_linear_text,
            ..
        } = walked;
        let command_walk_allocation = roukaku_observability::thread_allocation_snapshot();
        let resource_view = AtlasResourceView::new(&self.resources, &render_mutations);
        validate_resource_budget(&resource_view)?;
        let command_ns = profiled_elapsed_ns(profile_cpu, command_started)?;
        let command_allocation_after = roukaku_observability::thread_allocation_snapshot();
        let command_allocation_bytes = command_allocation_after
            .allocated_bytes
            .saturating_sub(engine_allocation_before.allocated_bytes);
        let command_allocation_breakdown = [
            resource_validation_allocation
                .allocated_bytes
                .saturating_sub(engine_allocation_before.allocated_bytes),
            command_setup_allocation
                .allocated_bytes
                .saturating_sub(resource_validation_allocation.allocated_bytes),
            command_walk_allocation
                .allocated_bytes
                .saturating_sub(command_setup_allocation.allocated_bytes),
            command_allocation_after
                .allocated_bytes
                .saturating_sub(command_walk_allocation.allocated_bytes),
        ];
        if command_allocation_bytes > 0
            && self.last_command_allocation_breakdown != Some(command_allocation_breakdown)
            && self.command_allocation_diagnostics_emitted < 32
        {
            self.command_allocation_diagnostics_emitted += 1;
            tracing::debug!(
                event = "platform.wgpu.scene.command_allocation",
                total_bytes = command_allocation_bytes,
                resource_validation_bytes = command_allocation_breakdown[0],
                command_setup_bytes = command_allocation_breakdown[1],
                command_walk_bytes = command_allocation_breakdown[2],
                resource_budget_bytes = command_allocation_breakdown[3],
                quad_runs = quad_runs.len(),
                quad_runs_spilled = quad_runs.spilled(),
                draw_primitives = draw_runs.len(),
                draw_primitives_spilled = draw_runs.spilled(),
                run_ids = run_ids.len(),
                run_ids_spilled = run_ids.spilled(),
                drawn_resources = drawn_resources.len(),
                drawn_resources_spilled = drawn_resources.spilled(),
                render_mutations = render_mutations.len(),
                committed_mutations = committed_mutations.len(),
                upload_texture_count,
                upload_glyph_count,
                release_resource_count,
                transient_texture_count,
                transient_glyph_count,
                mesh_command_allocation_bytes,
                mesh_command_allocation_count,
                "WGPU scene command allocation profile changed"
            );
        }
        self.last_command_allocation_bytes = command_allocation_bytes;
        self.last_command_allocation_breakdown = Some(command_allocation_breakdown);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("roukaku-glyph-render-encoder"),
        });
        let atlas_started = Instant::now();
        let (atlas_update, upload_bytes) = if resources_changed {
            if atlas_upload_query.is_some() {
                device
                    .poll(wgpu::PollType::Poll)
                    .map_err(|_| invalid("glyph atlas staging ring polling failed"))?;
            }
            let staging_index = self.atlas_staging_index;
            let result = update_gpu_atlas(
                AtlasUpdateContext {
                    device,
                    queue,
                    layout: &self.layout,
                    linear_sampler: &self.linear_sampler,
                    nearest_sampler: &self.nearest_sampler,
                    frame_encoder: Some(&mut encoder),
                    staging_belt: &mut self.atlas_staging_belts[staging_index],
                    timestamp_query: atlas_upload_query,
                },
                self.atlas.as_ref(),
                &self.resources,
                &resource_view,
                &texture_region_uploads,
                &mut self.atlas_upload_pixels,
            );
            if atlas_upload_query.is_some() {
                self.atlas_staging_index = 0;
            }
            result?
        } else {
            (PreparedAtlasUpdate::None, 0)
        };
        let atlas_ns = profiled_elapsed_ns(profile_cpu, atlas_started)?;
        let atlas_allocation_after = roukaku_observability::thread_allocation_snapshot();
        self.last_atlas_allocation_bytes = atlas_allocation_after
            .allocated_bytes
            .saturating_sub(command_allocation_after.allocated_bytes);
        self.last_upload_bytes = upload_bytes;
        let (active_atlas, placement_view) = match &atlas_update {
            PreparedAtlasUpdate::None => {
                let atlas = self
                    .atlas
                    .as_ref()
                    .ok_or_else(|| invalid("glyph atlas is unavailable"))?;
                (atlas, AtlasPlacementView::committed(&atlas.packed))
            }
            PreparedAtlasUpdate::Replace(atlas) => {
                (atlas, AtlasPlacementView::committed(&atlas.packed))
            }
            PreparedAtlasUpdate::Mutate(mutation) => {
                let atlas = self
                    .atlas
                    .as_ref()
                    .ok_or_else(|| invalid("incremental atlas update has no base atlas"))?;
                (
                    atlas,
                    AtlasPlacementView::pending(&atlas.packed, &mutation.placements),
                )
            }
        };
        let geometry_started = Instant::now();
        self.vertex_bytes.clear();
        self.draw_batches.clear();
        build_vertices(
            frame,
            &quad_runs,
            &draw_runs,
            &placement_view,
            &resource_view,
            &mut self.vertex_bytes,
            &mut self.draw_batches,
        )?;
        let first_compositing = self.draw_batches.first().map(|batch| batch.compositing);
        let has_linear_text =
            has_linear_text && self.default_compositing == SceneCompositing2D::EncodedSrgb;
        let compositing = if has_linear_text {
            self.default_compositing
        } else {
            first_compositing
                .or_else(|| target.as_ref().map(|target| target.output.compositing))
                .unwrap_or_default()
        };
        let mixed = self
            .draw_batches
            .iter()
            .any(|batch| batch.compositing != compositing);
        if mixed && !has_linear_text {
            return Err(invalid(
                "a scene frame cannot mix linear and encoded compositing batches without linear text",
            ));
        }
        for batch in &self.draw_batches {
            if let BatchPipeline::Projective(blend, depth) = batch.pipeline {
                self.pipelines
                    .prepare_projective(device, batch.compositing, blend, depth);
            }
        }
        let geometry_ns = profiled_elapsed_ns(profile_cpu, geometry_started)?;
        let engine_allocation_after = roukaku_observability::thread_allocation_snapshot();
        self.last_geometry_allocation_bytes = engine_allocation_after
            .allocated_bytes
            .saturating_sub(atlas_allocation_after.allocated_bytes);
        self.last_engine_allocation_bytes = engine_allocation_after
            .allocated_bytes
            .saturating_sub(engine_allocation_before.allocated_bytes);
        self.last_engine_allocation_count = engine_allocation_after
            .allocation_count
            .saturating_sub(engine_allocation_before.allocation_count);
        let vertex_upload_started = Instant::now();
        if !self.vertex_bytes.is_empty() {
            let required = self.vertex_bytes.len() as u64;
            let mut vertex_buffer_recreated = false;
            if self.vertex_capacity < required {
                let capacity = required
                    .max(4096)
                    .checked_next_power_of_two()
                    .ok_or_else(|| invalid("glyph vertex buffer capacity overflowed"))?;
                self.vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("roukaku-glyph-vertex-buffer"),
                    size: capacity,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
                self.vertex_capacity = capacity;
                vertex_buffer_recreated = true;
            }
            if vertex_upload_required(
                &self.vertex_bytes,
                &self.uploaded_vertex_bytes,
                vertex_buffer_recreated,
            ) {
                queue.write_buffer(
                    self.vertex_buffer
                        .as_ref()
                        .ok_or_else(|| invalid("glyph vertex buffer is unavailable"))?,
                    0,
                    &self.vertex_bytes,
                );
                self.last_upload_bytes = self
                    .last_upload_bytes
                    .checked_add(required)
                    .ok_or_else(|| invalid("GPU upload byte counter overflowed"))?;
                self.uploaded_vertex_bytes.clear();
                self.uploaded_vertex_bytes
                    .extend_from_slice(&self.vertex_bytes);
            }
        }
        let vertex_upload_ns = profiled_elapsed_ns(profile_cpu, vertex_upload_started)?;
        let render_submit_started = Instant::now();
        let (output, mixed_output, linear_view) = if let Some(target) = target {
            // A single color space needs only a view of the shared texture;
            // switching between spaces still uses the normal transfer passes.
            let linear_view = (compositing == SceneCompositing2D::LinearSrgb).then(|| {
                &*target.linear_view.get_or_insert_with(|| {
                    target.output.texture.create_view(&wgpu::TextureViewDescriptor {
                        format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
                        ..Default::default()
                    })
                })
            });
            (&target.output, &mut target.mixed_output, linear_view)
        } else {
            if self.output.as_ref().is_none_or(|output| {
                output.width != frame.width
                    || output.height != frame.height
                    || output.compositing != compositing
            }) {
                self.output = Some(CachedOutput::new(device, frame.width, frame.height, compositing));
                self.mixed_output = None;
            }
            (
                self.output.as_ref().expect("scene output was prepared"),
                &mut self.mixed_output,
                None,
            )
        };
        let output_view = linear_view.unwrap_or(&output.view);
        let mixed = mixed.then(|| {
            let transfer = self
                .compositing_transfer
                .get_or_insert_with(|| CompositingTransfer::new(device));
            let target =
                mixed_output.get_or_insert_with(|| MixedOutput::new(device, transfer, output));
            (&*target, &*transfer)
        });
        let depth_view = self
            .draw_batches
            .iter()
            .any(|batch| batch.pipeline.depth_tested())
            .then(|| DepthTarget::view(&mut self.depth_target, device, frame.width, frame.height));
        // Changing depth mode or color space ends the pass, but neither change
        // resets the scene depth buffer or permits reordering a draw.
        let mut segments: SmallVec<[(usize, usize, bool, SceneCompositing2D); 4]> = SmallVec::new();
        for (index, batch) in self.draw_batches.iter().enumerate() {
            let depth_tested = batch.pipeline.depth_tested();
            let mode = batch.compositing;
            match segments.last_mut() {
                Some((_, end, depth, current)) if *depth == depth_tested && *current == mode => {
                    *end = index + 1;
                }
                _ => segments.push((index, index + 1, depth_tested, mode)),
            }
        }
        if segments.is_empty() {
            segments.push((0, 0, false, compositing));
        }
        let clear = frame.clear_rgba;
        let last_segment = segments.len() - 1;
        let finalize = segments[last_segment].3 != compositing;
        let mut depth_cleared = false;
        let mut transfers = 0;
        let mut previous_mode = segments[0].3;
        for (segment_index, &(start, end, depth_tested, mode)) in segments.iter().enumerate() {
            let view = if let Some((alternate, transfer)) = mixed {
                if previous_mode != mode {
                    alternate.transfer_to(transfer, &mut encoder, output, mode, None);
                    transfers += 1;
                    previous_mode = mode;
                }
                if mode == compositing {
                    output_view
                } else {
                    &alternate.alternate.view
                }
            } else {
                output_view
            };
            let clear_channel = |channel: u8| match mode {
                SceneCompositing2D::LinearSrgb => f64::from(srgb_byte_to_linear(channel)),
                SceneCompositing2D::EncodedSrgb => f64::from(channel) / 255.0,
            };
            let color_load = if segment_index == 0 {
                wgpu::LoadOp::Clear(wgpu::Color {
                    r: clear_channel(clear[0]),
                    g: clear_channel(clear[1]),
                    b: clear_channel(clear[2]),
                    a: f64::from(clear[3]) / 255.0,
                })
            } else {
                wgpu::LoadOp::Load
            };
            let depth_stencil_attachment = match (&depth_view, depth_tested) {
                (Some(view), true) => {
                    let load = if depth_cleared {
                        wgpu::LoadOp::Load
                    } else {
                        wgpu::LoadOp::Clear(1.0)
                    };
                    depth_cleared = true;
                    Some(wgpu::RenderPassDepthStencilAttachment {
                        view,
                        depth_ops: Some(wgpu::Operations {
                            load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    })
                }
                _ => None,
            };
            let timestamp_writes = timestamp_query.and_then(|(query_set, beginning, end_index)| {
                let beginning = (segment_index == 0).then_some(beginning);
                let end_index = (segment_index == last_segment && !finalize).then_some(end_index);
                (beginning.is_some() || end_index.is_some()).then_some(
                    wgpu::RenderPassTimestampWrites {
                        query_set,
                        beginning_of_pass_write_index: beginning,
                        end_of_pass_write_index: end_index,
                    },
                )
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("roukaku-glyph-render-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: color_load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment,
                timestamp_writes,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let Some(vertex_buffer) = &self.vertex_buffer else {
                continue;
            };
            for batch in &self.draw_batches[start..end] {
                pass.set_bind_group(
                    0,
                    active_atlas.bind_group(
                        batch.page,
                        batch.secondary_page,
                        batch.compositing,
                        batch.texture_filter,
                    ),
                    &[],
                );
                if batch.pipeline == BatchPipeline::Crossfade {
                    pass.set_bind_group(
                        1,
                        active_atlas.bind_group(
                            batch.secondary_page,
                            batch.secondary_page,
                            batch.compositing,
                            batch.texture_filter,
                        ),
                        &[],
                    );
                }
                pass.set_pipeline(self.pipelines.select_batch(batch));
                pass.set_scissor_rect(
                    batch.clip.x as u32,
                    batch.clip.y as u32,
                    batch.clip.width,
                    batch.clip.height,
                );
                pass.set_vertex_buffer(0, vertex_buffer.slice(batch.byte_offset..));
                pass.draw(0..batch.vertex_count, 0..1);
            }
        }
        if finalize && let Some((target, transfer)) = mixed {
            let timestamp_writes = timestamp_query.map(|(query_set, _, end_index)| {
                wgpu::RenderPassTimestampWrites {
                    query_set,
                    beginning_of_pass_write_index: None,
                    end_of_pass_write_index: Some(end_index),
                }
            });
            target.transfer_to(transfer, &mut encoder, output, compositing, timestamp_writes);
            transfers += 1;
        }
        let render_encode_ns = profiled_elapsed_ns(profile_cpu, render_submit_started)?;
        self.last_draw_calls = self.draw_batches.len() as u64 + transfers;
        Ok((
            committed_mutations,
            atlas_update,
            GlyphCpuProfile {
                command_ns,
                atlas_ns,
                geometry_ns,
                vertex_upload_ns,
                render_encode_ns,
                queue_submit_ns: 0,
                render_submit_ns: render_encode_ns,
            },
            encoder,
        ))
    }
}
