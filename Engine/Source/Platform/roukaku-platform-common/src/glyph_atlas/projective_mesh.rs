use super::*;
use roukaku_media_core::{MeshDepth2D, ProjectiveMeshVertex2D};

/// Clip position (16) + atlas UV (8) + premultiplied color (16) + pixel mask (8).
pub(crate) const PROJECTIVE_VERTEX_STRIDE: wgpu::BufferAddress = 48;

pub(crate) struct ProjectiveMeshRun<'a> {
    pub(super) pixel_mask: u64,
    pub(super) vertices: &'a [ProjectiveMeshVertex2D],
    pub(super) indices: &'a [u32],
    pub(super) texture_id: Option<&'a str>,
    pub(super) texture_filter: TextureFilter2D,
    pub(super) compositing: SceneCompositing2D,
    pub(super) opacity: f32,
    pub(super) blend: BlendMode,
    pub(super) depth: MeshDepth2D,
    pub(super) clip: RectI,
    pub(super) transform: Transform2D,
}

/// Applies the 2D transform to XY with translation scaled by W, then maps
/// logical pixels to clip space without dividing, keeping Z and W intact.
pub(super) fn projective_clip_position(
    position: [f32; 4],
    transform: Transform2D,
    width: u32,
    height: u32,
) -> Result<[f32; 4], PlatformError> {
    let [x, y, z, w] = position;
    let transformed_x = transform.m11 * x + transform.m21 * y + transform.tx * w;
    let transformed_y = transform.m12 * x + transform.m22 * y + transform.ty * w;
    let clip = [
        transformed_x / width as f32 * 2.0 - w,
        w - transformed_y / height as f32 * 2.0,
        z,
        w,
    ];
    if !clip.into_iter().all(f32::is_finite) {
        return Err(invalid("projective mesh transform overflowed"));
    }
    Ok(clip)
}

/// Converts unit-range premultiplied sRGBA into the compositing space and
/// applies opacity.
fn projective_color(rgba: [f32; 4], opacity: f32, compositing: SceneCompositing2D) -> [f32; 4] {
    match compositing {
        SceneCompositing2D::EncodedSrgb => rgba.map(|value| value * opacity),
        SceneCompositing2D::LinearSrgb => {
            let alpha = rgba[3];
            let channel = |value: f32| {
                let encoded = if alpha == 0.0 { 0.0 } else { value / alpha };
                let linear = if encoded <= 0.04045 {
                    encoded / 12.92
                } else {
                    ((encoded + 0.055) / 1.055).powf(2.4)
                };
                linear * alpha * opacity
            };
            [
                channel(rgba[0]),
                channel(rgba[1]),
                channel(rgba[2]),
                alpha * opacity,
            ]
        }
    }
}

/// Writes one 48-byte vertex per index; returns the vertex count and the
/// atlas page the mesh samples from. Untextured meshes sample the white texel
/// of `untextured_page`, which every page carries at its origin.
pub(super) fn push_projective_vertices(
    run: &ProjectiveMeshRun<'_>,
    atlas: &AtlasPlacementView<'_>,
    resources: &AtlasResourceView<'_>,
    frame: &SceneFrame,
    untextured_page: usize,
    bytes: &mut Vec<u8>,
) -> Result<(u32, usize), PlatformError> {
    let texture_mapping = run
        .texture_id
        .map(|resource_id| {
            let resource = resources
                .get(resource_id)
                .ok_or_else(|| invalid("projective mesh texture resource is missing"))?;
            let placement = atlas
                .get(resource_id)
                .ok_or_else(|| invalid("projective mesh texture has no atlas placement"))?;
            Ok::<_, PlatformError>((placement, resource.width(), resource.height()))
        })
        .transpose()?;
    let page = texture_mapping.map_or(untextured_page, |(placement, _, _)| placement.page);
    let atlas_width = atlas.width(page) as f32;
    let atlas_height = atlas.height(page) as f32;
    for index in run.indices {
        let vertex = &run.vertices[*index as usize];
        let (u, v) = match texture_mapping {
            Some((placement, width, height)) => (
                (placement.x as f32 + vertex.uv[0] * width as f32) / atlas_width,
                (placement.y as f32 + vertex.uv[1] * height as f32) / atlas_height,
            ),
            None => (0.5 / atlas_width, 0.5 / atlas_height),
        };
        let position =
            projective_clip_position(vertex.position, run.transform, frame.width, frame.height)?;
        let color = projective_color(vertex.premultiplied_rgba, run.opacity, run.compositing);
        for value in position.into_iter().chain([u, v]).chain(color) {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        bytes.extend_from_slice(&(run.pixel_mask as u32).to_ne_bytes());
        bytes.extend_from_slice(&((run.pixel_mask >> 32) as u32).to_ne_bytes());
    }
    let count =
        u32::try_from(run.indices.len()).map_err(|_| invalid("scene vertex count overflowed"))?;
    Ok((count, page))
}

/// Depth attachment shared by every depth-tested projective mesh of a frame.
pub(crate) struct DepthTarget {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) texture: wgpu::Texture,
}

impl DepthTarget {
    /// Reuses the cached attachment when the frame size is unchanged.
    pub(super) fn view(
        target: &mut Option<Self>,
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> wgpu::TextureView {
        if target
            .as_ref()
            .is_none_or(|depth| depth.width != width || depth.height != height)
        {
            *target = Some(Self {
                width,
                height,
                texture: device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("roukaku-projective-depth"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: DEPTH_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                }),
            });
        }
        target
            .as_ref()
            .expect("depth target was just created")
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    pub(super) fn bytes(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height) * 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_position_scales_translation_by_w_and_keeps_depth() {
        let transform = Transform2D {
            m11: 2.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            tx: 4.0,
            ty: 2.0,
        };
        // x' = 2*6 + 4*2 = 20 -> 20/10*2 - 2 = 2; y' = 3 + 2*2 = 7 -> 2 - 7/10*2 = 0.6.
        let clip = projective_clip_position([6.0, 3.0, 0.5, 2.0], transform, 10, 10).unwrap();
        for (actual, expected) in clip.into_iter().zip([2.0, 0.6, 0.5, 2.0]) {
            assert!((actual - expected).abs() < 1e-6, "{clip:?}");
        }
        assert!(projective_clip_position([f32::MAX, 0.0, 0.0, 1.0], transform, 10, 10).is_err());
    }
}
