use super::*;
use roukaku_media_core::{MeshDepth2D, SceneCompositing2D};
use roukaku_platform::{PlatformError, PlatformErrorCode};

pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4, 3 => Uint32x2, 4 => Float32x2, 5 => Float32];

const PROJECTIVE_VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 4] =
    wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x2, 2 => Float32x4, 3 => Uint32x2];

const BLEND_MODES: [BlendMode; 6] = [
    BlendMode::Alpha,
    BlendMode::Opaque,
    BlendMode::Add,
    BlendMode::Multiply,
    BlendMode::Screen,
    BlendMode::ScreenRgb,
];

pub(crate) fn create_pipelines(
    device: &wgpu::Device,
) -> (
    wgpu::BindGroupLayout,
    wgpu::Sampler,
    wgpu::Sampler,
    BlendPipelines,
) {
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("roukaku-glyph-atlas-layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    });
    let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("roukaku-glyph-atlas-sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("roukaku-glyph-atlas-nearest-sampler"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("roukaku-glyph-atlas-shader"),
        source: wgpu::ShaderSource::Wgsl(GLYPH_SHADER.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("roukaku-glyph-atlas-pipeline-layout"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    // Crossfade binds the previous texture's page as group 0 and the current
    // texture's page as group 1, so both may live on different atlas pages.
    let crossfade_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("roukaku-glyph-atlas-crossfade-pipeline-layout"),
        bind_group_layouts: &[Some(&layout), Some(&layout)],
        immediate_size: 0,
    });
    let pipelines = BlendPipelines {
        linear: create_blend_pipeline_set(
            device,
            &pipeline_layout,
            &crossfade_layout,
            &shader,
            SceneCompositing2D::LinearSrgb,
        ),
        encoded: create_blend_pipeline_set(
            device,
            &pipeline_layout,
            &crossfade_layout,
            &shader,
            SceneCompositing2D::EncodedSrgb,
        ),
        projective: ProjectivePipelines {
            // Without depth-clip control the pipelines keep the default
            // near/far clipping instead of failing projective draws.
            unclipped_depth: device
                .features()
                .contains(wgpu::Features::DEPTH_CLIP_CONTROL),
            pipelines: std::array::from_fn(|_| None),
        },
        layout: pipeline_layout,
        shader,
    };
    (layout, linear_sampler, nearest_sampler, pipelines)
}

/// Lazily built projective pipelines, keyed by compositing, blend and depth.
pub(crate) struct ProjectivePipelines {
    pub(super) unclipped_depth: bool,
    pub(super) pipelines: [Option<wgpu::RenderPipeline>; 24],
}

impl ProjectivePipelines {
    fn index(compositing: SceneCompositing2D, blend: BlendMode, depth: MeshDepth2D) -> usize {
        let compositing = match compositing {
            SceneCompositing2D::LinearSrgb => 0,
            SceneCompositing2D::EncodedSrgb => 1,
        };
        let blend = BLEND_MODES
            .iter()
            .position(|mode| *mode == blend)
            .expect("every blend mode has a projective pipeline slot");
        let depth = usize::from(depth == MeshDepth2D::ReadWriteLess);
        (compositing * BLEND_MODES.len() + blend) * 2 + depth
    }
}

impl BlendPipelines {
    /// Builds the projective pipeline for this combination on first use.
    pub(super) fn prepare_projective(
        &mut self,
        device: &wgpu::Device,
        compositing: SceneCompositing2D,
        blend: BlendMode,
        depth: MeshDepth2D,
    ) {
        let index = ProjectivePipelines::index(compositing, blend, depth);
        if self.projective.pipelines[index].is_none() {
            let (state, fragment) = blend_state(blend);
            self.projective.pipelines[index] = Some(create_pipeline(
                device,
                PipelineSpec {
                    label: &format!(
                        "roukaku-glyph-atlas-{}-{blend:?}-{depth:?}-projective-pipeline",
                        compositing_name(compositing)
                    ),
                    layout: &self.layout,
                    shader: &self.shader,
                    vertex_entry: "vs_projective",
                    vertex_stride: PROJECTIVE_VERTEX_STRIDE,
                    vertex_attributes: &PROJECTIVE_VERTEX_ATTRIBUTES,
                    fragment_entry: fragment,
                    target_format: scene_target_format(compositing),
                    blend: state,
                    depth,
                    unclipped_depth: self.projective.unclipped_depth,
                },
            ));
        }
    }

    pub(super) fn select_projective(
        &self,
        compositing: SceneCompositing2D,
        blend: BlendMode,
        depth: MeshDepth2D,
    ) -> &wgpu::RenderPipeline {
        self.projective.pipelines[ProjectivePipelines::index(compositing, blend, depth)]
            .as_ref()
            .expect("projective pipeline was prepared before encoding")
    }
}

struct PipelineSpec<'a> {
    label: &'a str,
    layout: &'a wgpu::PipelineLayout,
    shader: &'a wgpu::ShaderModule,
    vertex_entry: &'a str,
    vertex_stride: wgpu::BufferAddress,
    vertex_attributes: &'a [wgpu::VertexAttribute],
    fragment_entry: &'a str,
    target_format: wgpu::TextureFormat,
    blend: wgpu::BlendState,
    depth: MeshDepth2D,
    unclipped_depth: bool,
}

fn create_pipeline(device: &wgpu::Device, spec: PipelineSpec<'_>) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(spec.label),
        layout: Some(spec.layout),
        vertex: wgpu::VertexState {
            module: spec.shader,
            entry_point: Some(spec.vertex_entry),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: spec.vertex_stride,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: spec.vertex_attributes,
            })],
        },
        fragment: Some(wgpu::FragmentState {
            module: spec.shader,
            entry_point: Some(spec.fragment_entry),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: spec.target_format,
                blend: Some(spec.blend),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            unclipped_depth: spec.unclipped_depth,
            ..Default::default()
        },
        depth_stencil: (spec.depth == MeshDepth2D::ReadWriteLess).then_some(
            wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            },
        ),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// Fixed-function blend and fragment entry point for each scene blend mode.
fn blend_state(blend: BlendMode) -> (wgpu::BlendState, &'static str) {
    let color_only = |color: wgpu::BlendComponent| wgpu::BlendState {
        color,
        alpha: wgpu::BlendComponent::OVER,
    };
    match blend {
        BlendMode::Alpha => (wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING, "fs_main"),
        BlendMode::Opaque => (wgpu::BlendState::REPLACE, "fs_opaque"),
        BlendMode::Add => (
            color_only(wgpu::BlendComponent {
                // fs_main already outputs premultiplied color, as the CPU renderer adds.
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            }),
            "fs_main",
        ),
        // dst * (1 - srcA) + dst * premultiplied_src, the CPU coverage-weighted multiply.
        BlendMode::Multiply => (
            color_only(wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Dst,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            }),
            "fs_main",
        ),
        BlendMode::Screen => (
            color_only(wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrc,
                operation: wgpu::BlendOperation::Add,
            }),
            "fs_main",
        ),
        // Screens the tinted RGB regardless of coverage and keeps destination alpha.
        BlendMode::ScreenRgb => (
            wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrc,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Zero,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            },
            "fs_screen_rgb",
        ),
    }
}

fn compositing_name(compositing: SceneCompositing2D) -> &'static str {
    match compositing {
        SceneCompositing2D::LinearSrgb => "linear",
        SceneCompositing2D::EncodedSrgb => "encoded",
    }
}

pub(crate) fn create_blend_pipeline_set(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    crossfade_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    compositing: SceneCompositing2D,
) -> BlendPipelineSet {
    let target_format = scene_target_format(compositing);
    let pipeline = |layout: &wgpu::PipelineLayout,
                    blend: wgpu::BlendState,
                    blend_name: &str,
                    fragment: &str| {
        create_pipeline(
            device,
            PipelineSpec {
                label: &format!(
                    "roukaku-glyph-atlas-{}-{blend_name}-pipeline",
                    compositing_name(compositing)
                ),
                layout,
                shader,
                vertex_entry: "vs_main",
                vertex_stride: VERTEX_STRIDE,
                vertex_attributes: &VERTEX_ATTRIBUTES,
                fragment_entry: fragment,
                target_format,
                blend,
                depth: MeshDepth2D::Disabled,
                unclipped_depth: false,
            },
        )
    };
    let blend = |mode: BlendMode, blend_name: &str| {
        let (state, fragment) = blend_state(mode);
        pipeline(layout, state, blend_name, fragment)
    };
    BlendPipelineSet {
        alpha: blend(BlendMode::Alpha, "alpha"),
        opaque: blend(BlendMode::Opaque, "opaque"),
        add: blend(BlendMode::Add, "add"),
        multiply: blend(BlendMode::Multiply, "multiply"),
        screen: blend(BlendMode::Screen, "screen"),
        screen_rgb: blend(BlendMode::ScreenRgb, "screen-rgb"),
        mask_gradient: pipeline(
            layout,
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "mask-gradient",
            "fs_mask_gradient",
        ),
        mask_threshold: pipeline(
            layout,
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "mask-threshold",
            "fs_mask_threshold",
        ),
        crossfade: pipeline(
            crossfade_layout,
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "crossfade",
            "fs_crossfade",
        ),
    }
}

pub(crate) fn scene_target_format(compositing: SceneCompositing2D) -> wgpu::TextureFormat {
    match compositing {
        SceneCompositing2D::LinearSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
        SceneCompositing2D::EncodedSrgb => wgpu::TextureFormat::Rgba8Unorm,
    }
}

pub(crate) fn invalid(message: &'static str) -> PlatformError {
    PlatformError::new(
        PlatformErrorCode::InvalidState,
        "surface.present_scene",
        message,
    )
}

pub(crate) const GLYPH_SHADER: &str = r#"
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) pixel_mask: vec2<u32>,
    // Second atlas sampling coordinate for mask-reveal and crossfade draws;
    // unused (zero) for regular quads and meshes.
    @location(4) mask_uv: vec2<f32>,
    @location(5) phase: f32,
};
// Homogeneous clip-space position; the rasterizer divides by w, so UV and
// color interpolate perspective-correctly.
struct ProjectiveVertexInput {
    @location(0) position: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) pixel_mask: vec2<u32>,
};
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) pixel_mask: vec2<u32>,
    @location(3) mask_uv: vec2<f32>,
    @location(4) @interpolate(flat) phase: f32,
};
@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;
// Masks are byte-domain coverage, even when the image uses an sRGB view.
@group(0) @binding(2) var mask_atlas: texture_2d<f32>;
// Crossfade only: the page holding the current texture.
@group(1) @binding(0) var current_atlas: texture_2d<f32>;
@group(1) @binding(1) var current_sampler: sampler;
@vertex fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.uv = input.uv;
    output.color = input.color;
    output.pixel_mask = input.pixel_mask;
    output.mask_uv = input.mask_uv;
    output.phase = input.phase;
    return output;
}
@vertex fn vs_projective(input: ProjectiveVertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = input.position;
    output.uv = input.uv;
    output.color = input.color;
    output.pixel_mask = input.pixel_mask;
    output.mask_uv = vec2<f32>(0.0);
    output.phase = 0.0;
    return output;
}
fn masked_sample(input: VertexOutput) -> vec4<f32> {
    let sample = textureSample(atlas, atlas_sampler, input.uv);
    let pixel = vec2<u32>(input.position.xy) % vec2<u32>(8u);
    let bit = 63u - (pixel.y * 8u + pixel.x);
    if ((input.pixel_mask[bit / 32u] >> (bit % 32u)) & 1u) == 0u {
        discard;
    }
    return sample;
}
@fragment fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let sample = masked_sample(input);
    let alpha = sample.a * input.color.a;
    return vec4<f32>(sample.rgb * input.color.rgb * sample.a, alpha);
}
@fragment fn fs_opaque(input: VertexOutput) -> @location(0) vec4<f32> {
    let sample = masked_sample(input);
    return vec4<f32>(sample.rgb * input.color.rgb, sample.a * input.color.a);
}
// Vertex color is premultiplied by opacity, so this is `rgb * tint * opacity`.
@fragment fn fs_screen_rgb(input: VertexOutput) -> @location(0) vec4<f32> {
    let sample = masked_sample(input);
    return vec4<f32>(sample.rgb * input.color.rgb, 0.0);
}
// `uv` addresses the previous texture, `mask_uv` the current one and `phase`
// carries progress; both samples are lerped in premultiplied space and scaled
// by the opacity stored in the vertex color.
@fragment fn fs_crossfade(input: VertexOutput) -> @location(0) vec4<f32> {
    let current = textureSample(current_atlas, current_sampler, input.mask_uv);
    let previous = masked_sample(input);
    let mixed = mix(
        vec4<f32>(previous.rgb * previous.a, previous.a),
        vec4<f32>(current.rgb * current.a, current.a),
        input.phase,
    );
    return mixed * input.color.a;
}
// The mask texture is inverted by the native loader before submission.
fn mask_reveal_alpha(input: VertexOutput, gradient: bool) -> f32 {
    let mask = textureSample(mask_atlas, atlas_sampler, input.mask_uv);
    let luminance = dot(mask.rgb, vec3<f32>(0.299, 0.587, 0.114)) * 255.0;
    let phase = input.phase;
    if (gradient) {
        return clamp((luminance + phase - 256.0) / 255.0, 0.0, 1.0);
    }
    // Absorb sub-byte sampling error without quantizing interpolated masks.
    return select(0.0, 1.0, luminance <= phase + 0.0001);
}
@fragment fn fs_mask_gradient(input: VertexOutput) -> @location(0) vec4<f32> {
    let reveal = mask_reveal_alpha(input, true);
    let sample = masked_sample(input);
    let alpha = sample.a * input.color.a * reveal;
    return vec4<f32>(sample.rgb * input.color.rgb * sample.a * reveal, alpha);
}
@fragment fn fs_mask_threshold(input: VertexOutput) -> @location(0) vec4<f32> {
    let reveal = mask_reveal_alpha(input, false);
    let sample = masked_sample(input);
    let alpha = sample.a * input.color.a * reveal;
    return vec4<f32>(sample.rgb * input.color.rgb * sample.a * reveal, alpha);
}
"#;
