use super::*;

pub(super) struct CompositingTransfer {
    layout: wgpu::BindGroupLayout,
    to_linear: wgpu::RenderPipeline,
    to_encoded: wgpu::RenderPipeline,
}

pub(super) struct MixedOutput {
    pub(super) alternate: CachedOutput,
    primary_input: wgpu::BindGroup,
    alternate_input: wgpu::BindGroup,
}

impl MixedOutput {
    pub(super) fn new(
        device: &wgpu::Device,
        transfer: &CompositingTransfer,
        primary: &CachedOutput,
    ) -> Self {
        let alternate = CachedOutput::new(
            device,
            primary.width,
            primary.height,
            match primary.compositing {
                SceneCompositing2D::EncodedSrgb => SceneCompositing2D::LinearSrgb,
                SceneCompositing2D::LinearSrgb => SceneCompositing2D::EncodedSrgb,
            },
        );
        let input = |source: &CachedOutput| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("roukaku-compositing-transfer-input"),
                layout: &transfer.layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&source.view),
                }],
            })
        };
        Self {
            primary_input: input(primary),
            alternate_input: input(&alternate),
            alternate,
        }
    }

    pub(super) fn transfer_to(
        &self,
        transfer: &CompositingTransfer,
        encoder: &mut wgpu::CommandEncoder,
        primary: &CachedOutput,
        mode: SceneCompositing2D,
        timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'_>>,
    ) {
        let (source, target) = if mode == primary.compositing {
            (&self.alternate_input, primary)
        } else {
            (&self.primary_input, &self.alternate)
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("roukaku-compositing-transfer"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(match mode {
            SceneCompositing2D::LinearSrgb => &transfer.to_linear,
            SceneCompositing2D::EncodedSrgb => &transfer.to_encoded,
        });
        pass.set_bind_group(0, source, &[]);
        pass.draw(0..3, 0..1);
    }
}

impl CompositingTransfer {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("roukaku-compositing-transfer-input"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("roukaku-compositing-transfer-layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("roukaku-compositing-transfer"),
            source: wgpu::ShaderSource::Wgsl(TRANSFER.into()),
        });
        let pipeline = |entry, mode| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("roukaku-compositing-transfer"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: scene_target_format(mode),
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            to_linear: pipeline("to_linear", SceneCompositing2D::LinearSrgb),
            to_encoded: pipeline("to_encoded", SceneCompositing2D::EncodedSrgb),
            layout,
        }
    }
}

// sRGB views transfer the stored RGB channels, not straight paint. Undo and
// redo premultiplication across the transfer so changing passes leaves alpha
// untouched and subsequent commands see the same underlying backdrop.
const TRANSFER: &str = r#"
@group(0) @binding(0) var input: texture_2d<f32>;

@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = array(vec2(-1.0,-1.0), vec2(3.0,-1.0), vec2(-1.0,3.0));
    return vec4(p[i], 0.0, 1.0);
}

fn decode(x: vec3<f32>) -> vec3<f32> {
    return select(pow((x + vec3(0.055)) / 1.055, vec3(2.4)), x / 12.92, x <= vec3(0.04045));
}

fn encode(x: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(x, vec3(1.0 / 2.4)) - vec3(0.055), x * 12.92, x <= vec3(0.0031308));
}

@fragment fn to_linear(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let src = textureLoad(input, vec2<i32>(p.xy), 0);
    if src.a == 0.0 || src.a == 1.0 {
        return vec4(decode(src.rgb), src.a);
    }
    return vec4(decode(src.rgb / src.a) * src.a, src.a);
}

@fragment fn to_encoded(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let src = textureLoad(input, vec2<i32>(p.xy), 0);
    if src.a == 0.0 || src.a == 1.0 {
        return vec4(encode(src.rgb), src.a);
    }
    return vec4(encode(src.rgb / src.a) * src.a, src.a);
}
"#;
