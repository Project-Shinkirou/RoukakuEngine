use super::*;

/// Two retained textures blended in premultiplied space across one quad.
/// The previous texture binds as group 0 and the current texture as group 1,
/// so the two resources may live in different atlas pages.
pub(crate) struct CrossfadeRun<'a> {
    pub(super) pixel_mask: u64,
    pub(super) previous_resource: &'a str,
    pub(super) current_resource: &'a str,
    /// Shared source rectangle; `None` maps each texture fully.
    pub(super) source: Option<RectI>,
    /// Logical corners: top-left, top-right, bottom-left, bottom-right.
    pub(super) positions: [[f32; 2]; 4],
    pub(super) progress: f32,
    pub(super) opacity: f32,
    pub(super) compositing: SceneCompositing2D,
    pub(super) clip: RectI,
    pub(super) transform: Transform2D,
}

/// Corner triangles split along the top-right/bottom-left diagonal, matching
/// the CPU rasterizer.
const CORNERS: [usize; 6] = [0, 1, 2, 2, 1, 3];

/// Writes six vertices in the shared 52-byte layout: `uv` addresses the
/// previous texture, `mask_uv` the current texture, `phase` carries progress
/// and the color is opacity-scaled premultiplied white. Returns the atlas
/// pages of the previous and current textures.
pub(super) fn push_crossfade_vertices(
    run: &CrossfadeRun<'_>,
    atlas: &AtlasPlacementView<'_>,
    frame: &SceneFrame,
    bytes: &mut Vec<u8>,
) -> Result<[usize; 2], PlatformError> {
    let (previous_page, previous) = texture_corners(atlas, run.previous_resource, run.source)?;
    let (current_page, current) = texture_corners(atlas, run.current_resource, run.source)?;
    let color = [run.opacity; 4];
    for corner in CORNERS {
        let [x, y] = run.positions[corner];
        let (x, y) = transform_point(run.transform, x, y);
        if !x.is_finite() || !y.is_finite() {
            return Err(invalid("transformed crossfade quad is not finite"));
        }
        let [u, v] = previous[corner];
        push_vertex(
            bytes,
            x,
            y,
            u,
            v,
            color,
            frame.width,
            frame.height,
            run.pixel_mask,
            current[corner],
            run.progress,
        );
    }
    Ok([previous_page, current_page])
}

fn texture_corners(
    atlas: &AtlasPlacementView<'_>,
    resource_id: &str,
    source: Option<RectI>,
) -> Result<(usize, [[f32; 2]; 4]), PlatformError> {
    let placement = atlas
        .get(resource_id)
        .ok_or_else(|| invalid("crossfade texture has no atlas placement"))?;
    let source = source.unwrap_or(RectI::new(0, 0, placement.width, placement.height));
    let width = atlas.width(placement.page) as f32;
    let height = atlas.height(placement.page) as f32;
    let left = (placement.x + source.x as u32) as f32 / width;
    let top = (placement.y + source.y as u32) as f32 / height;
    let right = (placement.x + source.x as u32 + source.width) as f32 / width;
    let bottom = (placement.y + source.y as u32 + source.height) as f32 / height;
    Ok((
        placement.page,
        [[left, top], [right, top], [left, bottom], [right, bottom]],
    ))
}
