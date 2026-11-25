use super::*;
use crate::scene_filters::SceneFilters;

/// A caller-owned render target for same-device scene presentation. Its base
/// `Rgba8Unorm` format stores encoded sRGB RGB channels with opaque alpha after
/// rendering.
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

    /// The rendered texture itself. The default view samples encoded RGB with
    /// alpha set to one.
    pub fn texture(&self) -> &wgpu::Texture {
        &self.output.texture
    }
}

/// A session-local atlas compositor on a host-owned WGPU device. The default
/// blend space is encoded sRGB; `LinearGlyphRun` still blends in linear light.
/// Device loss and target pooling are managed by the host.
pub struct WgpuSharedSceneRenderer {
    renderer: WgpuGlyphAtlasRenderer,
    opaque_alpha: wgpu::RenderPipeline,
}

impl WgpuSharedSceneRenderer {
    pub fn new(device: &wgpu::Device) -> Self {
        let mut renderer = WgpuGlyphAtlasRenderer::new(device);
        renderer.set_default_compositing(SceneCompositing2D::EncodedSrgb);
        Self {
            renderer,
            opaque_alpha: opaque_alpha_pipeline(device),
        }
    }

    /// Submit an opaque scene directly to `target` without mapping or copying it.
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
        let (mutations, atlas_update, _, mut encoder) =
            self.renderer
                .render_internal(device, queue, frame, true, Some(&mut *target), None)?;
        // Keep intermediate alpha for blending and color-space transfers,
        // then make the same texture opaque for the presentation consumer.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("roukaku-shared-scene-opaque-alpha"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.output.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.opaque_alpha);
            pass.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
        self.renderer.commit_resources(mutations, atlas_update);
        Ok(())
    }
}

fn opaque_alpha_pipeline(device: &wgpu::Device) -> wgpu::RenderPipeline {
    const LABEL: &str = "roukaku-shared-scene-opaque-alpha";
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(LABEL),
        source: wgpu::ShaderSource::Wgsl(
            r#"
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = array(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4(p[i], 0.0, 1.0);
}

@fragment fn fs() -> @location(0) vec4<f32> {
    return vec4(0.0, 0.0, 0.0, 1.0);
}
"#
            .into(),
        ),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(LABEL),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: wgpu::ColorWrites::ALPHA,
            })],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}
