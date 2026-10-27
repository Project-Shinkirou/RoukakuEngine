use super::*;
use roukaku_platform::PlatformError;
use std::collections::BTreeMap;

pub(crate) fn update_gpu_atlas(
    mut context: AtlasUpdateContext<'_>,
    current: Option<&GpuAtlas>,
    old_resources: &BTreeMap<String, AtlasResource>,
    new_resources: &AtlasResourceView<'_>,
    texture_region_uploads: &[TextureRegionUpload<'_>],
    upload_pixels: &mut Vec<u8>,
) -> Result<(PreparedAtlasUpdate, u64), PlatformError> {
    let Some(current) = current else {
        let (atlas, upload_bytes) =
            create_full_gpu_atlas(&mut context, new_resources, upload_pixels)?;
        return Ok((PreparedAtlasUpdate::Replace(atlas), upload_bytes));
    };
    let mut allocators = current
        .packed
        .pages
        .iter()
        .enumerate()
        .map(|(index, page)| AtlasAllocatorState::new(index, page))
        .collect::<SmallVec<[_; 1]>>();
    let mut placement_mutations = AtlasPlacementJournal::new();
    for stale_id in current
        .packed
        .placements
        .keys()
        .filter(|id| !old_resources.contains_key(*id))
        .cloned()
        .collect::<Vec<_>>()
    {
        let placement = *current
            .packed
            .placements
            .get(&stale_id)
            .ok_or_else(|| invalid("stale resource has no atlas placement"))?;
        release_pending_atlas_slot(&mut allocators[placement.page], placement)?;
        placement_mutations.insert(stale_id, None);
    }
    for resource_id in old_resources.keys() {
        if !new_resources.contains_key(resource_id) {
            let placement = *current
                .packed
                .placements
                .get(resource_id)
                .ok_or_else(|| invalid("released resource has no atlas placement"))?;
            release_pending_atlas_slot(&mut allocators[placement.page], placement)?;
            placement_mutations.insert(resource_id.clone(), None);
        }
    }
    let uploads = new_resources
        .iter()
        .filter(|(id, resource)| {
            old_resources.get(*id) != Some(*resource)
                && !texture_region_uploads
                    .iter()
                    .any(|upload| upload.resource_id == id.as_str())
        })
        .collect::<Vec<_>>();
    for (resource_id, resource) in &uploads {
        // Transient ids may be reused by the next frame. Their old placement
        // was released above; consult the pending removal before reusing it.
        if let Some(placement) = current
            .packed
            .placements
            .get(*resource_id)
            .filter(|_| !matches!(placement_mutations.get(*resource_id), Some(None)))
        {
            if placement.width == resource.width() && placement.height == resource.height() {
                continue;
            }
            // A retained cache entry may change dimensions. Retire the entire
            // padded slot before allocating, including when the image shrinks.
            release_pending_atlas_slot(&mut allocators[placement.page], *placement)?;
            placement_mutations.insert((*resource_id).clone(), None);
        }
        let placement = allocators.iter_mut().find_map(|allocator| {
            allocate_pending_atlas_slot(allocator, resource.width(), resource.height()).ok()
        });
        let Some(placement) = placement else {
            tracing::debug!(
                event = "platform.wgpu.atlas.repacked",
                resource_count = new_resources.len(),
                freed_area = allocators
                    .iter()
                    .map(|allocator| allocator.freed_area)
                    .sum::<u64>(),
                previous_pages = current.packed.pages.len(),
                previous_bytes = current.packed.byte_len(),
                "glyph atlas capacity or fragmentation triggered a GPU-side repack"
            );
            return repack_gpu_atlas(
                &mut context,
                current,
                old_resources,
                new_resources,
                upload_pixels,
            );
        };
        placement_mutations.insert((*resource_id).clone(), Some(placement));
    }
    let mut upload_encoder = context.timestamp_query.map(|(query_set, begin, _)| {
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("roukaku-glyph-atlas-upload-encoder"),
            });
        encoder.write_timestamp(query_set, begin);
        encoder
    });
    let mut upload_bytes = 0u64;
    for (resource_id, resource) in uploads {
        let placement = placement_mutations
            .get(resource_id)
            .and_then(|placement| *placement)
            .or_else(|| current.packed.placements.get(resource_id).copied())
            .ok_or_else(|| invalid("new atlas resource has no planned placement"))?;
        upload_resource_rect(
            context.queue,
            upload_encoder.as_mut(),
            &current.textures[placement.page],
            resource,
            placement,
            upload_pixels,
            context.staging_belt,
        )?;
        upload_bytes = upload_bytes
            .checked_add(padded_resource_bytes(resource))
            .ok_or_else(|| invalid("glyph atlas upload byte count overflowed"))?;
    }
    for upload in texture_region_uploads {
        let placement = current
            .packed
            .placements
            .get(upload.resource_id)
            .copied()
            .ok_or_else(|| invalid("texture region target has no atlas placement"))?;
        let bytes = upload_texture_region(
            context.queue,
            upload_encoder.as_mut(),
            &current.textures[placement.page],
            placement,
            *upload,
            upload_pixels,
            context.staging_belt,
        )?;
        upload_bytes = upload_bytes
            .checked_add(bytes)
            .ok_or_else(|| invalid("glyph atlas upload byte count overflowed"))?;
    }
    if let (Some((query_set, _, end)), Some(mut encoder)) =
        (context.timestamp_query, upload_encoder)
    {
        encoder.write_timestamp(query_set, end);
        context.staging_belt.finish();
        context.queue.submit([encoder.finish()]);
        context.staging_belt.recall();
    }
    Ok((
        PreparedAtlasUpdate::Mutate(PackedAtlasMutation {
            placements: placement_mutations,
            pages: allocators
                .into_iter()
                .map(AtlasAllocatorState::into_mutation)
                .collect(),
        }),
        upload_bytes,
    ))
}

pub(crate) fn repack_gpu_atlas(
    context: &mut AtlasUpdateContext<'_>,
    current: &GpuAtlas,
    old_resources: &BTreeMap<String, AtlasResource>,
    new_resources: &AtlasResourceView<'_>,
    upload_pixels: &mut Vec<u8>,
) -> Result<(PreparedAtlasUpdate, u64), PlatformError> {
    let max_dimension = context.device.limits().max_texture_dimension_2d;
    let packed = match pack_atlas(
        new_resources,
        &REPACK_ATLAS_EXTENTS,
        current.packed.byte_len() / 4,
        max_dimension,
    ) {
        Ok(packed) => packed,
        Err(error) => {
            log_capacity_exceeded(new_resources, max_dimension)?;
            return Err(error);
        }
    };
    let atlas = create_empty_gpu_atlas(context, packed);
    let upload_bytes = fill_empty_gpu_atlas(
        context,
        &atlas,
        new_resources,
        Some((current, old_resources)),
        upload_pixels,
    )?;
    Ok((PreparedAtlasUpdate::Replace(atlas), upload_bytes))
}

fn log_capacity_exceeded(
    resources: &AtlasResourceView<'_>,
    max_dimension: u32,
) -> Result<(), PlatformError> {
    let (resource_bytes, padded_area, widest, tallest) = resources.values().try_fold(
        (0_u64, 0_u64, 0_u32, 0_u32),
        |(bytes, area, widest, tallest), resource| {
            let padded_width = resource.width() + ATLAS_PADDING * 2;
            let padded_height = resource.height() + ATLAS_PADDING * 2;
            Ok::<_, PlatformError>((
                bytes
                    .checked_add(resource.byte_len() as u64)
                    .ok_or_else(|| invalid("scene resource byte count overflowed"))?,
                area.checked_add(u64::from(padded_width) * u64::from(padded_height))
                    .ok_or_else(|| invalid("glyph atlas total area overflowed"))?,
                widest.max(padded_width),
                tallest.max(padded_height),
            ))
        },
    )?;
    tracing::error!(
        event = "platform.wgpu.atlas.capacity_exceeded",
        resource_count = resources.len(),
        resource_bytes,
        padded_area,
        widest,
        tallest,
        max_texture_dimension_2d = max_dimension,
        max_atlas_width = MAX_ATLAS_WIDTH,
        max_atlas_height = ATLAS_SIDE,
        max_atlas_pages = MAX_ATLAS_PAGES,
        "retained scene resources cannot be packed into the GPU atlas"
    );
    Ok(())
}

fn padded_resource_bytes(resource: &AtlasResource) -> u64 {
    u64::from(resource.width() + ATLAS_PADDING * 2)
        * u64::from(resource.height() + ATLAS_PADDING * 2)
        * 4
}

/// Writes every resource into a freshly created atlas. Resources unchanged
/// since `previous` are copied GPU-side from their old page; the rest are
/// staged from CPU memory.
fn fill_empty_gpu_atlas(
    context: &mut AtlasUpdateContext<'_>,
    atlas: &GpuAtlas,
    resources: &AtlasResourceView<'_>,
    previous: Option<(&GpuAtlas, &BTreeMap<String, AtlasResource>)>,
    upload_pixels: &mut Vec<u8>,
) -> Result<u64, PlatformError> {
    let mut encoder = context
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("roukaku-glyph-atlas-repack-encoder"),
        });
    if let Some((query_set, begin, _)) = context.timestamp_query {
        encoder.write_timestamp(query_set, begin);
    }
    let mut upload_bytes = 0u64;
    let mut staged_uploads = false;
    for (resource_id, resource) in resources.iter() {
        let placement = *atlas
            .packed
            .placements
            .get(resource_id)
            .ok_or_else(|| invalid("repacked atlas resource has no placement"))?;
        let copy_bytes = padded_resource_bytes(resource);
        let old_placement = previous.and_then(|(current, old_resources)| {
            (old_resources.get(resource_id) == Some(resource))
                .then(|| current.packed.placements.get(resource_id))
                .flatten()
                .map(|old_placement| (current, *old_placement))
        });
        if let Some((current, old_placement)) = old_placement {
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &current.textures[old_placement.page],
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: old_placement.x - ATLAS_PADDING,
                        y: old_placement.y - ATLAS_PADDING,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &atlas.textures[placement.page],
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: placement.x - ATLAS_PADDING,
                        y: placement.y - ATLAS_PADDING,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: resource.width() + ATLAS_PADDING * 2,
                    height: resource.height() + ATLAS_PADDING * 2,
                    depth_or_array_layers: 1,
                },
            );
        } else {
            upload_resource_rect(
                context.queue,
                Some(&mut encoder),
                &atlas.textures[placement.page],
                resource,
                placement,
                upload_pixels,
                context.staging_belt,
            )?;
            staged_uploads = true;
        }
        upload_bytes = upload_bytes
            .checked_add(copy_bytes)
            .ok_or_else(|| invalid("glyph atlas repack byte count overflowed"))?;
    }
    if let Some((query_set, _, end)) = context.timestamp_query {
        encoder.write_timestamp(query_set, end);
    }
    if staged_uploads {
        context.staging_belt.finish();
    }
    context.queue.submit([encoder.finish()]);
    if staged_uploads {
        context.staging_belt.recall();
    }
    Ok(upload_bytes)
}

fn create_atlas_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("roukaku-glyph-atlas"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb],
    })
}

/// Builds the bind groups for every (page, mask page) pair.
fn assemble_gpu_atlas(
    context: &AtlasUpdateContext<'_>,
    textures: Vec<wgpu::Texture>,
    packed: PackedAtlas,
) -> GpuAtlas {
    let views = textures
        .iter()
        .map(|texture| {
            let linear = texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
                ..Default::default()
            });
            let encoded = texture.create_view(&wgpu::TextureViewDescriptor::default());
            (linear, encoded)
        })
        .collect::<Vec<_>>();
    let mut bind_groups = Vec::with_capacity(views.len() * views.len());
    for (linear_view, encoded_view) in &views {
        for (_, mask_view) in &views {
            bind_groups.push(AtlasBindGroups {
                linear: create_atlas_bind_group(
                    context.device,
                    context.layout,
                    linear_view,
                    mask_view,
                    context.linear_sampler,
                    "roukaku-glyph-atlas-linear-bind-group",
                ),
                nearest: create_atlas_bind_group(
                    context.device,
                    context.layout,
                    linear_view,
                    mask_view,
                    context.nearest_sampler,
                    "roukaku-glyph-atlas-nearest-bind-group",
                ),
                encoded_linear: create_atlas_bind_group(
                    context.device,
                    context.layout,
                    encoded_view,
                    mask_view,
                    context.linear_sampler,
                    "roukaku-glyph-atlas-encoded-linear-bind-group",
                ),
                encoded_nearest: create_atlas_bind_group(
                    context.device,
                    context.layout,
                    encoded_view,
                    mask_view,
                    context.nearest_sampler,
                    "roukaku-glyph-atlas-encoded-nearest-bind-group",
                ),
            });
        }
    }
    GpuAtlas {
        textures,
        bind_groups,
        packed,
    }
}

pub(crate) fn create_empty_gpu_atlas(
    context: &AtlasUpdateContext<'_>,
    packed: PackedAtlas,
) -> GpuAtlas {
    let textures = packed
        .pages
        .iter()
        .map(|page| {
            let texture = create_atlas_texture(context.device, page.width, page.height);
            // Solid-color primitives sample the reserved texel at the page
            // origin. Every replacement page must establish the same
            // invariant as the initial/full atlas before any draw observes it.
            context.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &[255, 255, 255, 255],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: Some(1),
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
            texture
        })
        .collect();
    assemble_gpu_atlas(context, textures, packed)
}

pub(crate) fn create_reserved_gpu_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    linear_sampler: &wgpu::Sampler,
    nearest_sampler: &wgpu::Sampler,
    side: u32,
) -> GpuAtlas {
    let packed = PackedAtlas {
        placements: BTreeMap::new(),
        pages: vec![AtlasPageAllocation {
            width: side,
            height: side,
            cursor_x: ATLAS_PADDING * 3,
            cursor_y: ATLAS_PADDING,
            row_height: 0,
            free_slots: Vec::new(),
            freed_area: 0,
        }],
    };
    let mut staging_belt = primed_staging_belt(device);
    let context = AtlasUpdateContext {
        device,
        queue,
        layout,
        linear_sampler,
        nearest_sampler,
        staging_belt: &mut staging_belt,
        timestamp_query: None,
    };
    create_empty_gpu_atlas(&context, packed)
}

/// Builds the first atlas for `resources`. A set that fits one page within
/// the staging budget uploads that page in a single write; an overflowing set
/// fills its pages resource by resource. Returns the atlas and uploaded bytes.
pub(crate) fn create_full_gpu_atlas(
    context: &mut AtlasUpdateContext<'_>,
    resources: &AtlasResourceView<'_>,
    upload_pixels: &mut Vec<u8>,
) -> Result<(GpuAtlas, u64), PlatformError> {
    let max_dimension = context.device.limits().max_texture_dimension_2d;
    let packed = match pack_atlas(resources, &INITIAL_ATLAS_EXTENTS, 0, max_dimension) {
        Ok(packed) => packed,
        Err(error) => {
            log_capacity_exceeded(resources, max_dimension)?;
            return Err(error);
        }
    };
    if packed.pages.len() > 1 || packed.byte_len() > MAX_ATLAS_UPLOAD_BYTES as u64 {
        tracing::debug!(
            event = "platform.wgpu.atlas.paged",
            resource_count = resources.len(),
            pages = packed.pages.len(),
            atlas_bytes = packed.byte_len(),
            "scene resources overflow one atlas page"
        );
        let atlas = create_empty_gpu_atlas(context, packed);
        let upload_bytes = fill_empty_gpu_atlas(context, &atlas, resources, None, upload_pixels)?;
        return Ok((atlas, upload_bytes));
    }
    let page = &packed.pages[0];
    build_atlas_pixels(resources, &packed, 0, upload_pixels)?;
    let texture = create_atlas_texture(context.device, page.width, page.height);
    upload_atlas(
        context.device,
        context.queue,
        &texture,
        upload_pixels,
        page.width,
        page.height,
        context.timestamp_query,
    )?;
    upload_pixels.clear();
    upload_pixels.shrink_to(ATLAS_STAGING_CHUNK_BYTES as usize);
    let upload_bytes = packed.byte_len();
    Ok((
        assemble_gpu_atlas(context, vec![texture], packed),
        upload_bytes,
    ))
}

pub(crate) fn create_atlas_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    mask_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    label: &'static str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(mask_view),
            },
        ],
    })
}
