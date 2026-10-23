use super::*;

mod control;
mod draw;

pub(super) struct WalkedScene<'a> {
    pub(super) render_mutations: ResourceMutationJournal,
    pub(super) committed_mutations: ResourceMutationJournal,
    pub(super) resources_changed: bool,
    pub(super) quad_runs: SmallVec<[DrawRun<'a>; 64]>,
    pub(super) draw_runs: SmallVec<[DrawPrimitive<'a>; 64]>,
    pub(super) clip_stack: SmallVec<[RectI; 16]>,
    pub(super) pixel_masks: SmallVec<[u64; 16]>,
    pub(super) transform_stack: SmallVec<[Transform2D; 16]>,
    pub(super) camera: Transform2D,
    pub(super) opacity_stack: SmallVec<[f32; 16]>,
    pub(super) transient_sequence: u64,
    pub(super) run_ids: SceneIdSet<'a>,
    pub(super) drawn_resources: SceneIdSet<'a>,
    pub(super) upload_texture_count: u32,
    pub(super) texture_region_uploads: SmallVec<[TextureRegionUpload<'a>; 16]>,
    pub(super) upload_glyph_count: u32,
    pub(super) release_resource_count: u32,
    pub(super) transient_texture_count: u32,
    pub(super) transient_glyph_count: u32,
    pub(super) mesh_command_allocation_bytes: u64,
    pub(super) mesh_command_allocation_count: u64,
    pub(super) default_compositing: SceneCompositing2D,
}
impl<'a> WalkedScene<'a> {
    pub(super) fn new(atlas_missing: bool, default_compositing: SceneCompositing2D) -> Self {
        Self {
            render_mutations: ResourceMutationJournal::new(),
            committed_mutations: ResourceMutationJournal::new(),
            resources_changed: atlas_missing,
            transform_stack: smallvec![Transform2D::IDENTITY],
            camera: Transform2D::IDENTITY,
            opacity_stack: smallvec![1.0_f32],
            run_ids: SceneIdSet::default(),
            drawn_resources: SceneIdSet::default(),
            default_compositing,
            quad_runs: SmallVec::new(),
            draw_runs: SmallVec::new(),
            clip_stack: SmallVec::new(),
            pixel_masks: SmallVec::new(),
            transient_sequence: 0,
            upload_texture_count: 0,
            texture_region_uploads: SmallVec::new(),
            upload_glyph_count: 0,
            release_resource_count: 0,
            transient_texture_count: 0,
            transient_glyph_count: 0,
            mesh_command_allocation_bytes: 0,
            mesh_command_allocation_count: 0,
        }
    }
    pub(super) fn consume(
        &mut self,
        resources: &'a BTreeMap<String, AtlasResource>,
        frame: &'a SceneFrame,
        apply_mutations: bool,
    ) -> Result<(), PlatformError> {
        // Keep only masks used by this scene. Their pixels stay resident while
        // phase, opacity, or destination changes; retirement is transactional.
        let mut mask_ids = SceneIdSet::default();
        for command in &frame.commands {
            if let SceneCommand::MaskReveal { id, .. } = command {
                mask_ids.insert(id);
            }
        }
        for (resource_id, _) in resources
            .range::<str, _>((
                std::ops::Bound::Included("roukaku..mask."),
                std::ops::Bound::Unbounded,
            ))
            .take_while(|(id, _)| id.starts_with("roukaku..mask."))
        {
            let (_, draw_id) = resource_id["roukaku..mask.".len()..]
                .split_once('.')
                .expect("mask cache keys include a kind and draw ID");
            if !mask_ids.contains(draw_id) {
                self.render_mutations.insert(resource_id.clone(), None);
                self.committed_mutations.insert(resource_id.clone(), None);
                self.resources_changed = true;
            }
        }
        for command in &frame.commands {
            if !self.apply_control(command, resources, frame, apply_mutations)? {
                self.apply_draw(command, resources, frame)?;
            }
        }
        if !self.pixel_masks.is_empty()
            || !self.clip_stack.is_empty()
            || self.transform_stack.len() != 1
            || self.opacity_stack.len() != 1
        {
            return Err(invalid("GPU scene command stacks are not balanced"));
        }
        Ok(())
    }
}

pub(super) fn mask_reveal_resource_key(id: &str, kind: &str) -> SmallVec<[u8; 512]> {
    // Public IDs reject ".."; inline storage covers every validated draw ID.
    let mut key = SmallVec::new();
    key.extend_from_slice(b"roukaku..mask.");
    key.extend_from_slice(kind.as_bytes());
    key.push(b'.');
    key.extend_from_slice(id.as_bytes());
    key
}

pub(super) fn mask_reveal_resources_changed(
    resources: &BTreeMap<String, AtlasResource>,
    frame: &SceneFrame,
) -> bool {
    let mut live_count = 0;
    for command in &frame.commands {
        let SceneCommand::MaskReveal {
            id, frame, mask, ..
        } = command
        else {
            continue;
        };
        live_count += 2;
        for (kind, frame) in [("image", frame), ("mask", mask)] {
            let key = mask_reveal_resource_key(id, kind);
            let key = std::str::from_utf8(&key).expect("mask resource key is UTF-8");
            match resources.get(key) {
                Some(AtlasResource::Texture(texture))
                    if texture.patches.is_empty() && texture.base.as_ref() == frame => {}
                _ => return true,
            }
        }
    }
    resources
        .range::<str, _>((
            std::ops::Bound::Included("roukaku..mask."),
            std::ops::Bound::Unbounded,
        ))
        .take_while(|(id, _)| id.starts_with("roukaku..mask."))
        .count()
        != live_count
}
