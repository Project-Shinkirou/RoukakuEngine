use roukaku_media_core::{
    BlendMode, CpuRendererProvider, RectI, RenderTargetFormat, Renderer2DProvider,
    RendererCreateRequest, SceneCommand, SceneCompositing2D, TextureFrame,
};
use roukaku_platform::SceneFrame;
use roukaku_platform_common::WgpuOffscreenRenderer;

const BLEND_MODES: [BlendMode; 6] = [
    BlendMode::Alpha,
    BlendMode::Opaque,
    BlendMode::Add,
    BlendMode::Multiply,
    BlendMode::Screen,
    BlendMode::ScreenRgb,
];

fn upload() -> SceneCommand {
    SceneCommand::UploadTexture {
        resource_id: "blend.source".into(),
        frame: TextureFrame::from_vec(1, 1, vec![200, 100, 40, 128]).unwrap(),
    }
}

fn sprite(blend: BlendMode) -> SceneCommand {
    SceneCommand::Sprite {
        id: "blend.sprite".into(),
        texture_id: "blend.source".into(),
        source: None,
        destination: RectI::new(0, 0, 2, 2),
        opacity: 0.75,
        blend,
    }
}

#[tokio::test]
#[ignore = "requires a native hardware GPU runner"]
async fn gpu_blend_modes_match_cpu_reference_for_partial_alpha_on_hardware() {
    let destination = [60, 130, 220, 255];
    let mut gpu =
        WgpuOffscreenRenderer::new(&roukaku_platform_common::native_wgpu_instance().unwrap())
            .await
            .unwrap()
            .with_default_compositing(SceneCompositing2D::EncodedSrgb);
    assert!(matches!(
        gpu.identity().device_type.as_str(),
        "integrated_gpu" | "discrete_gpu"
    ));
    for (index, blend) in BLEND_MODES.into_iter().enumerate() {
        let mut cpu = CpuRendererProvider
            .create(RendererCreateRequest {
                width: 2,
                height: 2,
                format: RenderTargetFormat::Rgba8Srgb,
                profile: "blend-reference".into(),
            })
            .unwrap();
        let expected = cpu
            .capture_frame(&[SceneCommand::clear(destination), upload(), sprite(blend)])
            .unwrap();
        // The GPU renderer retains the texture after the first frame.
        let mut commands = Vec::with_capacity(2);
        if index == 0 {
            commands.push(upload());
        }
        commands.push(sprite(blend));
        let actual = gpu
            .render(&SceneFrame {
                sequence: index as u64 + 1,
                width: 2,
                height: 2,
                clear_rgba: destination,
                commands,
                semantics: None,
            })
            .unwrap();
        for (actual, expected) in actual
            .rgba8
            .as_chunks::<4>()
            .0
            .iter()
            .zip(expected.bytes.as_chunks::<4>().0)
        {
            assert!(
                actual
                    .iter()
                    .zip(expected)
                    .all(|(actual, expected)| actual.abs_diff(*expected) <= 1),
                "{blend:?}: GPU {actual:?} != CPU {expected:?}"
            );
        }
    }
}
