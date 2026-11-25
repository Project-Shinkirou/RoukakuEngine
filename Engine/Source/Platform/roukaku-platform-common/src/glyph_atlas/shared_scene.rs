use super::*;
use crate::scene_filters::SceneFilters;

/// A caller-owned render target for same-device scene presentation. Its base
/// `Rgba8Unorm` format stores encoded sRGB RGB channels and the scene's alpha.
/// Return a target to the render pool only after its consumer has submitted
/// the last sampling work on the same queue.
pub struct WgpuSceneTarget {
    pub(super) output: CachedOutput,
    // A wholly linear scene can use hardware sRGB writes to the same texture.
    pub(super) linear_view: Option<wgpu::TextureView>,
    pub(super) mixed_output: Option<MixedOutput>,
}

impl WgpuSceneTarget {
    /// Allocate one sampled color attachment on the host device.
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Result<Self, PlatformError> {
        let max = device.limits().max_texture_dimension_2d;
        if width == 0 || height == 0 || width > max || height > max {
            return Err(invalid("shared scene target extent is outside the device limits")
                .with_field("width", width.to_string())
                .with_field("height", height.to_string())
                .with_field("max_texture_dimension_2d", max.to_string()));
        }
        Ok(Self {
            output: CachedOutput::with_view_formats(
                device,
                width,
                height,
                SceneCompositing2D::EncodedSrgb,
                &[wgpu::TextureFormat::Rgba8UnormSrgb],
            ),
            linear_view: None,
            mixed_output: None,
        })
    }

    /// The rendered texture itself. The default view samples encoded RGB bytes.
    pub fn texture(&self) -> &wgpu::Texture {
        &self.output.texture
    }
}

/// A session-local atlas compositor on a host-owned WGPU device. The default
/// blend space is encoded sRGB; `LinearGlyphRun` still blends in linear light.
/// Device loss and target pooling are managed by the host.
pub struct WgpuSharedSceneRenderer {
    renderer: WgpuGlyphAtlasRenderer,
}

impl WgpuSharedSceneRenderer {
    pub fn new(device: &wgpu::Device) -> Self {
        let mut renderer = WgpuGlyphAtlasRenderer::new(device);
        renderer.set_default_compositing(SceneCompositing2D::EncodedSrgb);
        Self { renderer }
    }

    /// Submit a scene directly to `target` without mapping or copying the output.
    /// Use the same device and queue for all rendering and sampling. The target
    /// must match the frame's raster extent and be free of consumer leases.
    /// Post-processing filter graphs are not supported by this path.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &SceneFrame,
        target: &mut WgpuSceneTarget,
    ) -> Result<(), PlatformError> {
        if target.output.width != frame.width || target.output.height != frame.height {
            return Err(invalid("shared scene target extent does not match the frame"));
        }
        if SceneFilters::validate(frame)? != 0 {
            return Err(invalid("shared scene targets do not support post-processing filters"));
        }
        let (mutations, atlas_update, _, encoder) =
            self.renderer
                .render_internal(device, queue, frame, true, Some(target), None)?;
        queue.submit([encoder.finish()]);
        self.renderer.commit_resources(mutations, atlas_update);
        Ok(())
    }
}
