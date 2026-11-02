use super::{
    ATLAS_PADDING, ATLAS_SIDE, AtlasAllocatorState, AtlasResource, AtlasResourceView,
    INITIAL_ATLAS_EXTENTS, MAX_ATLAS_UPLOAD_BYTES, MAX_ATLAS_WIDTH, PackedAtlas,
    REPACK_ATLAS_EXTENTS, ResourceMutationJournal, RetainedTexture, allocate_pending_atlas_slot,
    pack_atlas, prepare_upload_pixels, release_pending_atlas_slot, updated_retained_texture,
    validate_resource_budget, vertex_upload_required, write_padded_resource,
};
use roukaku_media_core::TextureFrame;
use roukaku_platform::{PlatformError, PlatformErrorCode};
use std::{collections::BTreeMap, sync::Arc};

fn initial_pack(resources: &AtlasResourceView<'_>) -> Result<PackedAtlas, PlatformError> {
    pack_atlas(resources, &INITIAL_ATLAS_EXTENTS, 0, MAX_ATLAS_WIDTH)
}

fn sized_texture(width: u32, height: u32) -> AtlasResource {
    AtlasResource::texture(TextureFrame {
        width,
        height,
        rgba8: Vec::new().into(),
    })
}

fn assert_padded_slots_disjoint(packed: &PackedAtlas) {
    let placements = packed.placements.values().collect::<Vec<_>>();
    for (index, left) in placements.iter().enumerate() {
        let page = &packed.pages[left.page];
        assert!(left.x + left.width + ATLAS_PADDING <= page.width);
        assert!(left.y + left.height + ATLAS_PADDING <= page.height);
        assert!(left.x > ATLAS_PADDING * 2 || left.y > ATLAS_PADDING * 2);
        for right in &placements[index + 1..] {
            assert!(
                left.page != right.page
                    || left.x + left.width + ATLAS_PADDING <= right.x - ATLAS_PADDING
                    || right.x + right.width + ATLAS_PADDING <= left.x - ATLAS_PADDING
                    || left.y + left.height + ATLAS_PADDING <= right.y - ATLAS_PADDING
                    || right.y + right.height + ATLAS_PADDING <= left.y - ATLAS_PADDING,
                "padded atlas slots overlap"
            );
        }
    }
}

#[test]
fn atlas_width_tracks_total_area_and_packs_multiple_stage_textures() {
    let rgba8 = vec![0x7f; 800 * 600 * 4];
    let texture = TextureFrame {
        width: 800,
        height: 600,
        rgba8: rgba8.into(),
    };
    let resources = (0..8)
        .map(|index| {
            (
                format!("texture.stage.{index}"),
                AtlasResource::texture(texture.clone()),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mutations = ResourceMutationJournal::new();
    let packed = initial_pack(&AtlasResourceView::new(&resources, &mutations))
        .expect("stage textures must fit the bounded atlas");

    assert_eq!(packed.placements.len(), 8);
    assert_eq!(packed.pages.len(), 1);
    assert_eq!(
        (packed.pages[0].width, packed.pages[0].height),
        (4096, 4096)
    );
}

#[test]
fn rectangular_growth_packs_wide_stage_resources_without_raising_resource_budget() {
    let texture = sized_texture;
    let mut resources = BTreeMap::from([("texture.stage.large".to_string(), texture(2560, 1840))]);
    for index in 0..20 {
        resources.insert(format!("texture.stage.strip.{index}"), texture(512, 1000));
    }
    let mutations = ResourceMutationJournal::new();
    let view = AtlasResourceView::new(&resources, &mutations);

    // The square extent is tried first and overflows; the wide one holds all.
    let packed = pack_atlas(
        &view,
        &[(ATLAS_SIDE, ATLAS_SIDE), (MAX_ATLAS_WIDTH, ATLAS_SIDE)],
        0,
        MAX_ATLAS_WIDTH,
    )
    .expect("wide stage resources must fit the bounded rectangular atlas");
    assert_eq!(packed.pages.len(), 1);
    assert_eq!(
        (packed.pages[0].width, packed.pages[0].height),
        (8192, 4096)
    );
}

#[test]
fn texture_set_overflowing_one_page_spills_onto_additional_pages() {
    // Nine large backgrounds: about 134 MB raw, more than one 8192x4096 page.
    let mut resources = (0..9)
        .map(|index| (format!("texture.stage.{index}"), sized_texture(2730, 1365)))
        .collect::<BTreeMap<_, _>>();
    resources.insert("glyph.small".to_owned(), sized_texture(44, 44));
    let mutations = ResourceMutationJournal::new();
    let view = AtlasResourceView::new(&resources, &mutations);
    validate_resource_budget(&view).expect("the overflowing set stays within the budget");

    let packed = pack_atlas(&view, &REPACK_ATLAS_EXTENTS, 0, MAX_ATLAS_WIDTH).unwrap();

    assert_eq!(packed.placements.len(), 10);
    assert!(packed.pages.len() > 1);
    assert!(packed.pages.iter().all(|page| {
        page.width <= MAX_ATLAS_WIDTH
            && page.height <= ATLAS_SIDE
            && page.width.is_power_of_two()
            && page.height.is_power_of_two()
    }));
    // Pages beyond the first shrink to what their resources occupy.
    assert!(packed.byte_len() < packed.pages.len() as u64 * 8192 * 4096 * 4);
    // First fit: the small resource fills a gap on an earlier page instead
    // of opening a page of its own.
    let small_page = packed.placements["glyph.small"].page;
    assert!(
        packed
            .placements
            .iter()
            .any(|(id, placement)| id != "glyph.small" && placement.page == small_page)
    );
    assert_padded_slots_disjoint(&packed);
}

#[test]
fn device_limit_bounds_every_atlas_page() {
    let resources = (0..3)
        .map(|index| (format!("texture.{index}"), sized_texture(1500, 1500)))
        .collect::<BTreeMap<_, _>>();
    let mutations = ResourceMutationJournal::new();
    let view = AtlasResourceView::new(&resources, &mutations);

    let packed = pack_atlas(&view, &REPACK_ATLAS_EXTENTS, 0, 2048).unwrap();
    assert_eq!(packed.pages.len(), 3);
    assert!(
        packed
            .pages
            .iter()
            .all(|page| page.width <= 2048 && page.height <= 2048)
    );
    assert_padded_slots_disjoint(&packed);

    let oversized = BTreeMap::from([("texture.wide".to_owned(), sized_texture(3000, 100))]);
    let oversized = AtlasResourceView::new(&oversized, &mutations);
    assert!(pack_atlas(&oversized, &REPACK_ATLAS_EXTENTS, 0, 2048).is_err());
}

#[test]
fn resource_view_applies_mutations_without_cloning_the_base_map() {
    let texture = |value| {
        let rgba8 = vec![value; 4];
        AtlasResource::texture(TextureFrame {
            width: 1,
            height: 1,
            rgba8: rgba8.into(),
        })
    };
    let base = [
        ("keep".to_string(), texture(1)),
        ("release".to_string(), texture(2)),
        ("replace".to_string(), texture(3)),
    ]
    .into_iter()
    .collect::<BTreeMap<_, _>>();
    let mutations = [
        ("add".to_string(), Some(texture(4))),
        ("release".to_string(), None),
        ("replace".to_string(), Some(texture(5))),
    ]
    .into_iter()
    .collect::<ResourceMutationJournal>();
    let view = AtlasResourceView::new(&base, &mutations);

    assert_eq!(view.len(), 3);
    assert!(view.contains_key("keep"));
    assert!(view.contains_key("add"));
    assert!(!view.contains_key("release"));
    assert!(view.get("replace") == mutations["replace"].as_ref());
    assert_eq!(
        view.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
        vec!["keep", "add", "replace"]
    );
}

#[test]
fn released_atlas_slot_is_reused_without_repacking_live_resources() {
    let rgba8 = vec![0x7f; 64 * 64 * 4];
    let texture = TextureFrame {
        width: 64,
        height: 64,
        rgba8: rgba8.into(),
    };
    let resources = [(
        "texture.stable".to_string(),
        AtlasResource::texture(texture),
    )]
    .into_iter()
    .collect::<BTreeMap<_, _>>();
    let mutations = ResourceMutationJournal::new();
    let packed = initial_pack(&AtlasResourceView::new(&resources, &mutations)).unwrap();
    let original = packed.placements["texture.stable"];
    let mut allocator = AtlasAllocatorState::new(0, &packed.pages[0]);
    release_pending_atlas_slot(&mut allocator, original).unwrap();
    let reused = allocate_pending_atlas_slot(&mut allocator, 32, 32).unwrap();
    assert_eq!((reused.x, reused.y), (original.x, original.y));
    let original_area = u64::from(original.width + ATLAS_PADDING * 2)
        * u64::from(original.height + ATLAS_PADDING * 2);
    let reused_area = u64::from(32 + ATLAS_PADDING * 2).pow(2);
    assert_eq!(allocator.freed_area, original_area - reused_area);
    assert_eq!(allocator.added_free_slots.len(), 2);
    release_pending_atlas_slot(&mut allocator, reused).unwrap();
    assert_eq!(allocator.freed_area, original_area);
    assert_eq!(allocator.added_free_slots.len(), 1);
    assert_eq!(
        (
            allocator.added_free_slots[0].width,
            allocator.added_free_slots[0].height
        ),
        (
            original.width + ATLAS_PADDING * 2,
            original.height + ATLAS_PADDING * 2
        )
    );
}

#[test]
fn wider_than_current_atlas_requires_repacking_after_row_wrap() {
    let resources = BTreeMap::new();
    let mutations = ResourceMutationJournal::new();
    let packed = initial_pack(&AtlasResourceView::new(&resources, &mutations)).unwrap();
    assert_eq!(packed.pages[0].width, 1024);
    let mut allocator = AtlasAllocatorState::new(0, &packed.pages[0]);
    allocate_pending_atlas_slot(&mut allocator, 600, 10).unwrap();
    let cursor = (allocator.cursor_x, allocator.cursor_y, allocator.row_height);
    assert!(allocate_pending_atlas_slot(&mut allocator, 1280, 720).is_err());
    // A failed allocation leaves the page untouched so another page or a
    // repack can take the resource.
    assert_eq!(
        (allocator.cursor_x, allocator.cursor_y, allocator.row_height),
        cursor
    );

    let resources = BTreeMap::from([("texture.stage".to_owned(), sized_texture(1280, 720))]);
    let repacked = initial_pack(&AtlasResourceView::new(&resources, &mutations)).unwrap();
    assert_eq!(repacked.pages.len(), 1);
    assert_padded_slots_disjoint(&repacked);
}

#[test]
fn stable_vertex_payload_skips_upload_until_bytes_or_buffer_change() {
    let uploaded = [1_u8, 2, 3, 4];
    assert!(!vertex_upload_required(&uploaded, &uploaded, false));
    assert!(vertex_upload_required(&[1, 2, 3, 5], &uploaded, false));
    assert!(vertex_upload_required(&uploaded, &uploaded, true));
}

#[test]
fn rgba_row_upload_preserves_clamped_padding_pixels() {
    let rgba8 = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
    let resource = AtlasResource::texture(TextureFrame {
        width: 2,
        height: 2,
        rgba8: rgba8.into(),
    });
    let mut destination = vec![0; 4 * 4 * 4];
    write_padded_resource(&resource, &mut destination, 4, 0, 0).unwrap();
    let pixels = destination
        .as_chunks::<4>()
        .0
        .iter()
        .map(|pixel| pixel[0])
        .collect::<Vec<_>>();
    assert_eq!(
        pixels,
        vec![1, 1, 5, 5, 1, 1, 5, 5, 9, 9, 13, 13, 9, 9, 13, 13]
    );
}

#[test]
fn partial_texture_update_retains_base_without_full_payload_copy() {
    let rgba8 = roukaku_media_core::OwnedPixelBuffer::from_vec(vec![1, 2, 3, 4, 5, 6, 7, 8]);
    let base_pointer = rgba8.as_ptr();
    let retained = RetainedTexture {
        base: Arc::new(TextureFrame {
            width: 2,
            height: 1,
            rgba8,
        }),
        patches: vec![],
        patch_bytes: 0,
    };
    let patch = roukaku_media_core::OwnedPixelBuffer::from_vec(vec![9, 10, 11, 12]);
    let updated = updated_retained_texture(&retained, 1, 0, 1, 1, patch).unwrap();
    assert_eq!(updated.base.rgba8.as_ptr(), base_pointer);
    assert_eq!(updated.patch_bytes, 4);

    let resource = AtlasResource::Texture(Arc::new(updated));
    let mut destination = vec![0; 4 * 3 * 4];
    write_padded_resource(&resource, &mut destination, 4, 0, 0).unwrap();
    let pixels = destination.as_chunks::<4>().0.iter().collect::<Vec<_>>();
    assert_eq!(pixels[5], &[1, 2, 3, 4]);
    assert_eq!(pixels[6], &[9, 10, 11, 12]);
}

#[test]
fn ordered_partial_updates_retain_each_owned_patch_without_full_payload_copy() {
    let rgba8 =
        roukaku_media_core::OwnedPixelBuffer::from_vec(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    let base_pointer = rgba8.as_ptr();
    let retained = RetainedTexture {
        base: Arc::new(TextureFrame {
            width: 3,
            height: 1,
            rgba8,
        }),
        patches: vec![],
        patch_bytes: 0,
    };
    let first = roukaku_media_core::OwnedPixelBuffer::from_vec(vec![13, 14, 15, 16]);
    let first_pointer = first.as_ptr();
    let updated = updated_retained_texture(&retained, 0, 0, 1, 1, first).unwrap();
    let second = roukaku_media_core::OwnedPixelBuffer::from_vec(vec![17, 18, 19, 20]);
    let second_pointer = second.as_ptr();
    let updated = updated_retained_texture(&updated, 2, 0, 1, 1, second).unwrap();

    assert_eq!(updated.base.rgba8.as_ptr(), base_pointer);
    assert_eq!(updated.patches.len(), 2);
    assert_eq!(updated.patches[0].rgba8.as_ptr(), first_pointer);
    assert_eq!(updated.patches[1].rgba8.as_ptr(), second_pointer);
    assert_eq!(updated.patch_bytes, 8);
}

#[test]
fn reserved_atlas_staging_reuses_storage_across_upload_sizes() {
    let mut pixels = Vec::with_capacity(MAX_ATLAS_UPLOAD_BYTES);
    let storage = pixels.as_ptr();

    prepare_upload_pixels(&mut pixels, 800, 600).unwrap();
    assert_eq!(pixels.len(), 800 * 600 * 4);
    assert_eq!(pixels.as_ptr(), storage);

    prepare_upload_pixels(&mut pixels, 1280, 720).unwrap();
    assert_eq!(pixels.len(), 1280 * 720 * 4);
    assert_eq!(pixels.as_ptr(), storage);

    let error = prepare_upload_pixels(&mut pixels, ATLAS_SIDE + 1, ATLAS_SIDE).unwrap_err();
    assert_eq!(error.code, PlatformErrorCode::InvalidState);
}

#[test]
fn initial_and_incremental_atlas_slots_keep_disjoint_sampling_guards() {
    let colors = [
        ("a", [255, 0, 0, 255]),
        ("b", [0, 255, 0, 255]),
        ("c", [0, 0, 255, 255]),
    ];
    let resources = colors
        .iter()
        .map(|(id, rgba)| {
            (
                (*id).to_owned(),
                AtlasResource::texture(TextureFrame::from_vec(500, 3, rgba.repeat(1500)).unwrap()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mutations = ResourceMutationJournal::new();
    let view = AtlasResourceView::new(&resources, &mutations);
    let packed = initial_pack(&view).unwrap();
    let mut pixels = Vec::new();
    super::build_atlas_pixels(&view, &packed, 0, &mut pixels).unwrap();
    for (id, rgba) in colors {
        let placement = packed.placements[id];
        for y in placement.y - ATLAS_PADDING..placement.y + placement.height + ATLAS_PADDING {
            for x in placement.x - ATLAS_PADDING..placement.x + placement.width + ATLAS_PADDING {
                let offset = ((y * packed.pages[0].width + x) * 4) as usize;
                assert_eq!(
                    &pixels[offset..offset + 4],
                    &rgba,
                    "{id} guard at ({x}, {y})"
                );
            }
        }
    }
    let mut allocator = AtlasAllocatorState::new(0, &packed.pages[0]);
    let next = allocate_pending_atlas_slot(&mut allocator, 2, 2).unwrap();
    let mut packed = packed;
    packed.placements.insert("next".to_owned(), next);
    assert_padded_slots_disjoint(&packed);
}
