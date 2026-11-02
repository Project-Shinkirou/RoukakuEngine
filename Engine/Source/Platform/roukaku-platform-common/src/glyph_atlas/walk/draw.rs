use super::*;
impl<'a> WalkedScene<'a> {
    pub(super) fn apply_draw(
        &mut self,
        command: &'a SceneCommand,
        resources: &'a BTreeMap<String, AtlasResource>,
        frame: &'a SceneFrame,
    ) -> Result<(), PlatformError> {
        macro_rules! resource_get {
            ($id:expr) => {
                match self.render_mutations.get($id) {
                    Some(resource) => resource.as_ref(),
                    None => resources.get($id),
                }
            };
        }
        match command {
            SceneCommand::GlyphRun {
                id,
                glyphs,
                rgba,
                opacity,
                blend,
            } => {
                if id.is_empty()
                    || id.len() > 256
                    || !self.run_ids.insert(id)
                    || !opacity.is_finite()
                    || !(0.0..=1.0).contains(opacity)
                {
                    return Err(invalid(
                        "glyph run identity, opacity, or blend mode is invalid",
                    ));
                }
                let mut quads: SmallVec<[DrawQuad<'_>; 32]> = SmallVec::new();
                for glyph in glyphs.iter() {
                    let bitmap = resource_get!(&glyph.resource_id).ok_or_else(|| {
                        invalid("glyph run references a resource that is not live")
                    })?;
                    let AtlasResource::Glyph(bitmap) = bitmap else {
                        return Err(invalid("glyph run references a non-glyph resource"));
                    };
                    quads.push(DrawQuad {
                        source: QuadSource::Resource {
                            resource_id: Cow::Borrowed(glyph.resource_id.as_str()),
                            source: RectI::new(0, 0, bitmap.width, bitmap.height),
                        },
                        destination: RectI::new(glyph.x, glyph.y, bitmap.width, bitmap.height),
                        rotation_quadrants: glyph.rotation_quadrants,
                    });
                    self.drawn_resources.insert(&glyph.resource_id);
                }
                push_quad_run(
                    &mut self.quad_runs,
                    &mut self.draw_runs,
                    DrawRun {
                        pixel_mask: self.pixel_masks.last().copied().unwrap_or(u64::MAX),
                        compositing: self.default_compositing,
                        blend: *blend,
                        quads,
                        rgba: *rgba,
                        opacity: *opacity * self.opacity_stack.last().copied().unwrap_or(1.0),
                        clip: self.clip_stack.last().copied().unwrap_or(RectI::new(
                            0,
                            0,
                            frame.width,
                            frame.height,
                        )),
                        transform: current_transform(self.camera, &self.transform_stack),
                        mask: None,
                    },
                );
            }
            SceneCommand::Sprite {
                id,
                texture_id,
                source,
                destination,
                opacity,
                blend,
            } => {
                if id.is_empty()
                    || id.len() > 256
                    || !self.run_ids.insert(id)
                    || !opacity.is_finite()
                    || !(0.0..=1.0).contains(opacity)
                {
                    return Err(invalid(
                        "sprite identity, opacity, or blend mode is invalid",
                    ));
                }
                let texture = resource_get!(texture_id)
                    .ok_or_else(|| invalid("sprite references a resource that is not live"))?;
                let AtlasResource::Texture(texture) = texture else {
                    return Err(invalid("sprite references a non-texture resource"));
                };
                let source =
                    source.unwrap_or(RectI::new(0, 0, texture.base.width, texture.base.height));
                validate_source_rect(source, texture.base.width, texture.base.height)?;
                validate_destination(*destination)?;
                self.drawn_resources.insert(texture_id);
                push_quad_run(
                    &mut self.quad_runs,
                    &mut self.draw_runs,
                    DrawRun {
                        pixel_mask: self.pixel_masks.last().copied().unwrap_or(u64::MAX),
                        compositing: self.default_compositing,
                        blend: *blend,
                        quads: smallvec![DrawQuad {
                            source: QuadSource::Resource {
                                resource_id: Cow::Borrowed(texture_id.as_str()),
                                source,
                            },
                            destination: *destination,
                            rotation_quadrants: 0,
                        }],
                        rgba: [255; 4],
                        opacity: *opacity * self.opacity_stack.last().copied().unwrap_or(1.0),
                        clip: self.clip_stack.last().copied().unwrap_or(RectI::new(
                            0,
                            0,
                            frame.width,
                            frame.height,
                        )),
                        transform: current_transform(self.camera, &self.transform_stack),
                        mask: None,
                    },
                );
            }
            SceneCommand::Rect {
                id,
                x,
                y,
                width,
                height,
                rgba,
            } => {
                let duplicate = self.run_ids.contains(id);
                if id.is_empty() || id.len() > 256 || duplicate || *width == 0 || *height == 0 {
                    let id_hash = format!(
                        "sha256:{:x}",
                        base16ct::HexDisplay(&Sha256::digest(id.as_bytes()))
                    );
                    tracing::error!(
                        event = "platform.wgpu.scene.rectangle_rejected",
                        id_hash,
                        id_empty = id.is_empty(),
                        id_too_long = id.len() > 256,
                        duplicate,
                        width = *width,
                        height = *height,
                        "wgpu scene rejected an invalid rectangle"
                    );
                    return Err(invalid("rectangle identity or dimensions are invalid"));
                }
                self.run_ids.insert(id);
                push_quad_run(
                    &mut self.quad_runs,
                    &mut self.draw_runs,
                    DrawRun {
                        pixel_mask: self.pixel_masks.last().copied().unwrap_or(u64::MAX),
                        compositing: self.default_compositing,
                        blend: BlendMode::Alpha,
                        quads: smallvec![DrawQuad {
                            source: QuadSource::White,
                            destination: RectI::new(*x as i32, *y as i32, *width, *height),
                            rotation_quadrants: 0,
                        }],
                        rgba: *rgba,
                        opacity: self.opacity_stack.last().copied().unwrap_or(1.0),
                        clip: self.clip_stack.last().copied().unwrap_or(RectI::new(
                            0,
                            0,
                            frame.width,
                            frame.height,
                        )),
                        transform: current_transform(self.camera, &self.transform_stack),
                        mask: None,
                    },
                );
            }
            SceneCommand::Mesh2D {
                id,
                vertices,
                indices,
                material,
                texture_id,
                texture_filter,
                opacity,
                blend,
            } => {
                let mesh_command_started = roukaku_observability::thread_allocation_snapshot();
                if id.is_empty()
                    || id.len() > 256
                    || !self.run_ids.insert(id)
                    || !opacity.is_finite()
                    || !(0.0..=1.0).contains(opacity)
                    || !matches!(blend, BlendMode::Alpha | BlendMode::Screen)
                    || vertices.is_empty()
                    || indices.is_empty()
                    || indices.len() % 3 != 0
                {
                    return Err(invalid(
                        "mesh identity, topology, opacity, or blend is invalid",
                    ));
                }
                if vertices.len() > 250_000
                    || indices.len() > 750_000
                    || indices
                        .iter()
                        .any(|index| *index as usize >= vertices.len())
                    || vertices.iter().any(|vertex| {
                        !vertex.position.into_iter().all(f32::is_finite)
                            || !vertex.uv.into_iter().all(f32::is_finite)
                            || vertex.premultiplied_rgba[..3]
                                .iter()
                                .any(|channel| *channel > vertex.premultiplied_rgba[3])
                    })
                {
                    return Err(invalid("mesh vertex or index payload is invalid"));
                }
                let resolved_texture = match (material, texture_id) {
                    (MeshMaterial2D::Solid, None) => None,
                    (
                        MeshMaterial2D::ColorTexture | MeshMaterial2D::GlyphMask,
                        Some(resource_id),
                    ) => {
                        match resource_get!(resource_id) {
                            Some(AtlasResource::Texture(_)) => {}
                            Some(AtlasResource::Glyph(_)) => {
                                return Err(invalid("mesh references a glyph resource as texture"));
                            }
                            None => {
                                return Err(invalid("mesh references a resource that is not live"));
                            }
                        }
                        self.drawn_resources.insert(resource_id);
                        Some(resource_id.as_str())
                    }
                    _ => return Err(invalid("mesh material and texture binding mismatch")),
                };
                self.draw_runs.push(DrawPrimitive::Mesh(MeshRun {
                    pixel_mask: self.pixel_masks.last().copied().unwrap_or(u64::MAX),
                    vertices,
                    indices,
                    texture_id: resolved_texture,
                    texture_filter: *texture_filter,
                    compositing: self.default_compositing,
                    opacity: *opacity * self.opacity_stack.last().copied().unwrap_or(1.0),
                    blend: *blend,
                    clip: self.clip_stack.last().copied().unwrap_or(RectI::new(
                        0,
                        0,
                        frame.width,
                        frame.height,
                    )),
                    transform: current_transform(self.camera, &self.transform_stack),
                }));
                let mesh_command_finished = roukaku_observability::thread_allocation_snapshot();
                self.mesh_command_allocation_bytes =
                    self.mesh_command_allocation_bytes.saturating_add(
                        mesh_command_finished
                            .allocated_bytes
                            .saturating_sub(mesh_command_started.allocated_bytes),
                    );
                self.mesh_command_allocation_count =
                    self.mesh_command_allocation_count.saturating_add(
                        mesh_command_finished
                            .allocation_count
                            .saturating_sub(mesh_command_started.allocation_count),
                    );
            }
            SceneCommand::MeshBatch2D {
                vertices,
                indices,
                draws,
                compositing,
            } => {
                if vertices.is_empty()
                    || draws.is_empty()
                    || vertices.len() > 250_000
                    || indices.len() > 750_000
                {
                    return Err(invalid("mesh batch geometry is invalid"));
                }
                for draw in draws.iter() {
                    let vertex_start = draw.vertex_start as usize;
                    let vertex_end = vertex_start
                        .checked_add(draw.vertex_count as usize)
                        .ok_or_else(|| invalid("mesh batch vertex range overflowed"))?;
                    let index_start = draw.index_start as usize;
                    let index_end = index_start
                        .checked_add(draw.index_count as usize)
                        .ok_or_else(|| invalid("mesh batch index range overflowed"))?;
                    if vertex_end > vertices.len()
                        || index_end > indices.len()
                        || draw.vertex_count == 0
                        || draw.index_count == 0
                        || !draw.index_count.is_multiple_of(3)
                        || !draw.opacity.is_finite()
                        || !(0.0..=1.0).contains(&draw.opacity)
                        || indices[index_start..index_end]
                            .iter()
                            .any(|index| *index as usize >= draw.vertex_count as usize)
                    {
                        return Err(invalid("mesh batch draw range is invalid"));
                    }
                    let resolved_texture = match (&draw.material, &draw.texture_id) {
                        (MeshMaterial2D::Solid, None) => None,
                        (
                            MeshMaterial2D::ColorTexture | MeshMaterial2D::GlyphMask,
                            Some(resource_id),
                        ) => {
                            match resource_get!(resource_id) {
                                Some(AtlasResource::Texture(_)) => {}
                                Some(AtlasResource::Glyph(_)) => {
                                    return Err(invalid(
                                        "mesh batch references a glyph resource as texture",
                                    ));
                                }
                                None => {
                                    return Err(invalid(
                                        "mesh batch references a resource that is not live",
                                    ));
                                }
                            }
                            self.drawn_resources.insert(resource_id);
                            Some(resource_id.as_str())
                        }
                        _ => {
                            return Err(invalid(
                                "mesh batch material and texture binding mismatch",
                            ));
                        }
                    };
                    self.draw_runs.push(DrawPrimitive::Mesh(MeshRun {
                        pixel_mask: self.pixel_masks.last().copied().unwrap_or(u64::MAX),
                        vertices: &vertices[vertex_start..vertex_end],
                        indices: &indices[index_start..index_end],
                        texture_id: resolved_texture,
                        texture_filter: draw.texture_filter,
                        compositing: *compositing,
                        opacity: draw.opacity * self.opacity_stack.last().copied().unwrap_or(1.0),
                        blend: draw.blend,
                        clip: draw.scissor.unwrap_or_else(|| {
                            self.clip_stack.last().copied().unwrap_or(RectI::new(
                                0,
                                0,
                                frame.width,
                                frame.height,
                            ))
                        }),
                        transform: current_transform(self.camera, &self.transform_stack),
                    }));
                }
            }
            SceneCommand::Clear { rgba } => {
                push_quad_run(
                    &mut self.quad_runs,
                    &mut self.draw_runs,
                    DrawRun {
                        pixel_mask: self.pixel_masks.last().copied().unwrap_or(u64::MAX),
                        compositing: self.default_compositing,
                        blend: BlendMode::Alpha,
                        quads: smallvec![DrawQuad {
                            source: QuadSource::White,
                            destination: RectI::new(0, 0, frame.width, frame.height),
                            rotation_quadrants: 0,
                        }],
                        rgba: *rgba,
                        opacity: 1.0,
                        clip: RectI::new(0, 0, frame.width, frame.height),
                        transform: Transform2D::IDENTITY,
                        mask: None,
                    },
                );
            }
            SceneCommand::Texture {
                id,
                frame: texture,
                destination,
                opacity,
                blend,
            }
            | SceneCommand::VideoFrame {
                id,
                frame: texture,
                destination,
                opacity,
                blend,
                ..
            } => {
                self.transient_texture_count += 1;
                validate_draw_identity(id, opacity, *blend, &mut self.run_ids)?;
                validate_texture_metadata(texture)?;
                validate_destination(*destination)?;
                let resource_id = transient_resource_id(
                    &AtlasResourceView::new(resources, &self.render_mutations),
                    &mut self.transient_sequence,
                    "texture",
                )?;
                self.render_mutations.insert(
                    resource_id.clone(),
                    Some(AtlasResource::texture(texture.clone())),
                );
                self.resources_changed = true;
                push_quad_run(
                    &mut self.quad_runs,
                    &mut self.draw_runs,
                    DrawRun {
                        pixel_mask: self.pixel_masks.last().copied().unwrap_or(u64::MAX),
                        compositing: self.default_compositing,
                        blend: *blend,
                        quads: smallvec![DrawQuad {
                            source: QuadSource::Resource {
                                resource_id: Cow::Owned(resource_id.clone()),
                                source: RectI::new(0, 0, texture.width, texture.height),
                            },
                            destination: *destination,
                            rotation_quadrants: 0,
                        }],
                        rgba: [255; 4],
                        opacity: *opacity * self.opacity_stack.last().copied().unwrap_or(1.0),
                        clip: self.clip_stack.last().copied().unwrap_or(RectI::new(
                            0,
                            0,
                            frame.width,
                            frame.height,
                        )),
                        transform: current_transform(self.camera, &self.transform_stack),
                        mask: None,
                    },
                );
            }

            SceneCommand::MaskReveal {
                id,
                frame: texture,
                mask,
                destination,
                mode,
                phase,
                opacity,
            } => {
                self.transient_texture_count += 2;
                validate_draw_identity(id, opacity, BlendMode::Alpha, &mut self.run_ids)?;
                validate_texture_metadata(texture)?;
                validate_texture_metadata(mask)?;
                validate_destination(*destination)?;
                let maximum = match mode {
                    MaskRevealMode::Gradient => 512.0,
                    MaskRevealMode::Threshold => 256.0,
                };
                if !phase.is_finite() || !(0.0..=maximum).contains(phase) {
                    return Err(invalid("mask reveal phase is out of range"));
                }
                let image_id = self.mask_reveal_resource(resources, id, "image", texture);
                let mask_id = self.mask_reveal_resource(resources, id, "mask", mask);
                push_quad_run(
                    &mut self.quad_runs,
                    &mut self.draw_runs,
                    DrawRun {
                        pixel_mask: self.pixel_masks.last().copied().unwrap_or(u64::MAX),
                        compositing: self.default_compositing,
                        blend: BlendMode::Alpha,
                        quads: smallvec![DrawQuad {
                            source: QuadSource::Resource {
                                resource_id: image_id,
                                source: RectI::new(0, 0, texture.width, texture.height),
                            },
                            destination: *destination,
                            rotation_quadrants: 0,
                        }],
                        rgba: [255; 4],
                        opacity: *opacity * self.opacity_stack.last().copied().unwrap_or(1.0),
                        clip: self.clip_stack.last().copied().unwrap_or(RectI::new(
                            0,
                            0,
                            frame.width,
                            frame.height,
                        )),
                        transform: current_transform(self.camera, &self.transform_stack),
                        mask: Some(MaskRunParams {
                            mode: *mode,
                            phase: *phase,
                            mask_resource: mask_id,
                        }),
                    },
                );
            }
            SceneCommand::Glyph {
                id,
                glyph,
                x,
                y,
                rgba,
                opacity,
                blend,
            } => {
                self.transient_glyph_count += 1;
                validate_draw_identity(id, opacity, *blend, &mut self.run_ids)?;
                validate_glyph_metadata(glyph)?;
                let resource_id = transient_resource_id(
                    &AtlasResourceView::new(resources, &self.render_mutations),
                    &mut self.transient_sequence,
                    "glyph",
                )?;
                self.render_mutations.insert(
                    resource_id.clone(),
                    Some(AtlasResource::Glyph(Arc::new(glyph.clone()))),
                );
                self.resources_changed = true;
                push_quad_run(
                    &mut self.quad_runs,
                    &mut self.draw_runs,
                    DrawRun {
                        pixel_mask: self.pixel_masks.last().copied().unwrap_or(u64::MAX),
                        compositing: self.default_compositing,
                        blend: *blend,
                        quads: smallvec![DrawQuad {
                            source: QuadSource::Resource {
                                resource_id: Cow::Owned(resource_id.clone()),
                                source: RectI::new(0, 0, glyph.width, glyph.height),
                            },
                            destination: RectI::new(*x, *y, glyph.width, glyph.height),
                            rotation_quadrants: 0,
                        }],
                        rgba: *rgba,
                        opacity: *opacity * self.opacity_stack.last().copied().unwrap_or(1.0),
                        clip: self.clip_stack.last().copied().unwrap_or(RectI::new(
                            0,
                            0,
                            frame.width,
                            frame.height,
                        )),
                        transform: current_transform(self.camera, &self.transform_stack),
                        mask: None,
                    },
                );
            }
            SceneCommand::CrossfadeSprite {
                id,
                previous_texture_id,
                current_texture_id,
                source,
                progress,
                opacity,
                ..
            }
            | SceneCommand::CrossfadeQuad {
                id,
                previous_texture_id,
                current_texture_id,
                source,
                progress,
                opacity,
                ..
            } => {
                validate_draw_identity(id, opacity, BlendMode::Alpha, &mut self.run_ids)?;
                if !progress.is_finite() || !(0.0..=1.0).contains(progress) {
                    return Err(invalid("crossfade progress must be within 0..=1"));
                }
                for texture_id in [previous_texture_id, current_texture_id] {
                    let Some(AtlasResource::Texture(texture)) = resource_get!(texture_id) else {
                        return Err(invalid("crossfade references a texture that is not live"));
                    };
                    if let Some(source) = source {
                        validate_source_rect(*source, texture.base.width, texture.base.height)?;
                    }
                    self.drawn_resources.insert(texture_id);
                }
                let positions = match command {
                    SceneCommand::CrossfadeSprite { destination, .. } => {
                        validate_destination(*destination)?;
                        let left = destination.x as f32;
                        let top = destination.y as f32;
                        let right = left + destination.width as f32;
                        let bottom = top + destination.height as f32;
                        [[left, top], [right, top], [left, bottom], [right, bottom]]
                    }
                    SceneCommand::CrossfadeQuad { positions, .. } => {
                        if positions.iter().flatten().any(|value| !value.is_finite()) {
                            return Err(invalid("crossfade quad position is not finite"));
                        }
                        *positions
                    }
                    _ => unreachable!("matched crossfade commands only"),
                };
                self.draw_runs.push(DrawPrimitive::Crossfade(CrossfadeRun {
                    pixel_mask: self.pixel_masks.last().copied().unwrap_or(u64::MAX),
                    previous_resource: previous_texture_id,
                    current_resource: current_texture_id,
                    source: *source,
                    positions,
                    progress: *progress,
                    opacity: *opacity * self.opacity_stack.last().copied().unwrap_or(1.0),
                    compositing: self.default_compositing,
                    clip: self.clip_stack.last().copied().unwrap_or(RectI::new(
                        0,
                        0,
                        frame.width,
                        frame.height,
                    )),
                    transform: current_transform(self.camera, &self.transform_stack),
                }));
            }
            SceneCommand::ProjectiveMesh2D {
                id,
                vertices,
                indices,
                material,
                texture_id,
                texture_filter,
                opacity,
                blend,
                depth,
            } => {
                if id.is_empty()
                    || id.len() > 256
                    || !self.run_ids.insert(id)
                    || !opacity.is_finite()
                    || !(0.0..=1.0).contains(opacity)
                    || vertices.is_empty()
                    || indices.is_empty()
                    || indices.len() % 3 != 0
                {
                    return Err(invalid(
                        "projective mesh identity, topology, or opacity is invalid",
                    ));
                }
                if vertices.len() > 250_000
                    || indices.len() > 750_000
                    || indices
                        .iter()
                        .any(|index| *index as usize >= vertices.len())
                    || vertices.iter().any(|vertex| {
                        !vertex.position.into_iter().all(f32::is_finite)
                            || !vertex.uv.into_iter().all(f32::is_finite)
                            || !vertex
                                .premultiplied_rgba
                                .into_iter()
                                .all(|channel| (0.0..=1.0).contains(&channel))
                            || vertex.premultiplied_rgba[..3]
                                .iter()
                                .any(|channel| *channel > vertex.premultiplied_rgba[3])
                    })
                {
                    return Err(invalid(
                        "projective mesh vertex or index payload is invalid",
                    ));
                }
                let resolved_texture = match (material, texture_id) {
                    (MeshMaterial2D::Solid, None) => None,
                    (
                        MeshMaterial2D::ColorTexture | MeshMaterial2D::GlyphMask,
                        Some(resource_id),
                    ) => {
                        let Some(AtlasResource::Texture(_)) = resource_get!(resource_id) else {
                            return Err(invalid(
                                "projective mesh references a texture that is not live",
                            ));
                        };
                        self.drawn_resources.insert(resource_id);
                        Some(resource_id.as_str())
                    }
                    _ => {
                        return Err(invalid(
                            "projective mesh material and texture binding mismatch",
                        ));
                    }
                };
                self.draw_runs
                    .push(DrawPrimitive::ProjectiveMesh(ProjectiveMeshRun {
                        pixel_mask: self.pixel_masks.last().copied().unwrap_or(u64::MAX),
                        vertices,
                        indices,
                        texture_id: resolved_texture,
                        texture_filter: *texture_filter,
                        compositing: self.default_compositing,
                        opacity: *opacity * self.opacity_stack.last().copied().unwrap_or(1.0),
                        blend: *blend,
                        depth: *depth,
                        clip: self.clip_stack.last().copied().unwrap_or(RectI::new(
                            0,
                            0,
                            frame.width,
                            frame.height,
                        )),
                        transform: current_transform(self.camera, &self.transform_stack),
                    }));
            }
            _ => unreachable!("non-draw commands are consumed by apply_control"),
        }
        Ok(())
    }

    fn mask_reveal_resource(
        &mut self,
        resources: &'a BTreeMap<String, AtlasResource>,
        id: &str,
        kind: &str,
        frame: &TextureFrame,
    ) -> Cow<'a, str> {
        // Borrow live keys without allocations on phase-only frames.
        let key = mask_reveal_resource_key(id, kind);
        let key = std::str::from_utf8(&key).expect("mask resource key is UTF-8");
        let previous = resources.get_key_value(key);
        if let Some((key, AtlasResource::Texture(texture))) = previous
            && texture.patches.is_empty()
            && texture.base.as_ref() == frame
        {
            return Cow::Borrowed(key);
        }
        let resource = AtlasResource::texture(frame.clone());
        self.render_mutations
            .insert(key.to_owned(), Some(resource.clone()));
        self.committed_mutations
            .insert(key.to_owned(), Some(resource));
        self.resources_changed = true;
        match previous {
            Some((key, _)) => Cow::Borrowed(key),
            None => Cow::Owned(key.to_owned()),
        }
    }
}
