use roukaku_media_core::{
    BlendMode, MeshDepth2D, MeshMaterial2D, ProjectiveMeshVertex2D, RectI, SceneCommand,
    SceneCompositing2D, TextureFilter2D, TextureFrame,
};
use roukaku_platform::SceneFrame;
use roukaku_platform_common::WgpuOffscreenRenderer;

async fn hardware_renderer() -> WgpuOffscreenRenderer {
    let renderer =
        WgpuOffscreenRenderer::new(&roukaku_platform_common::native_wgpu_instance().unwrap())
            .await
            .unwrap()
            .with_default_compositing(SceneCompositing2D::EncodedSrgb);
    assert!(matches!(
        renderer.identity().device_type.as_str(),
        "integrated_gpu" | "discrete_gpu"
    ));
    renderer
}

fn solid_texture(resource_id: &str, width: u32, height: u32, rgba: [u8; 4]) -> SceneCommand {
    SceneCommand::UploadTexture {
        resource_id: resource_id.into(),
        frame: TextureFrame {
            width,
            height,
            rgba8: rgba.repeat((width * height) as usize).into(),
        },
    }
}

fn pixel(rgba8: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * width + x) * 4) as usize;
    rgba8[offset..offset + 4].try_into().unwrap()
}

fn assert_near(actual: [u8; 4], expected: [u8; 4], context: &str) {
    assert!(
        actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual.abs_diff(expected) <= 1),
        "{context}: {actual:?} != {expected:?}"
    );
}

#[tokio::test]
#[ignore = "requires a native hardware GPU runner"]
async fn crossfade_lerps_premultiplied_textures_on_hardware() {
    let mut renderer = hardware_renderer().await;
    let captured = renderer
        .render(&SceneFrame {
            sequence: 1,
            width: 4,
            height: 2,
            clear_rgba: [0, 0, 0, 255],
            commands: vec![
                solid_texture("crossfade.previous", 2, 2, [255, 0, 0, 255]),
                // Half-covered current texture: premultiplied lerp keeps the
                // previous colour dominant where the current one is sparse.
                solid_texture("crossfade.current", 2, 2, [0, 0, 255, 128]),
                SceneCommand::CrossfadeSprite {
                    id: "crossfade.sprite".into(),
                    previous_texture_id: "crossfade.previous".into(),
                    current_texture_id: "crossfade.current".into(),
                    source: None,
                    destination: RectI::new(0, 0, 2, 2),
                    progress: 0.5,
                    opacity: 1.0,
                },
                SceneCommand::CrossfadeQuad {
                    id: "crossfade.quad".into(),
                    previous_texture_id: "crossfade.previous".into(),
                    current_texture_id: "crossfade.current".into(),
                    source: Some(RectI::new(0, 0, 1, 1)),
                    positions: [[2.0, 0.0], [4.0, 0.0], [2.0, 2.0], [4.0, 2.0]],
                    progress: 1.0,
                    opacity: 0.5,
                },
            ],
            semantics: None,
        })
        .unwrap();
    // alpha = 0.5*1 + 0.5*0.502 = 0.751; red = 0.5, blue = 0.251 over black.
    for (x, y) in [(0, 0), (1, 1)] {
        assert_near(pixel(&captured.rgba8, 4, x, y), [128, 0, 64, 255], "sprite");
    }
    // Fully current at half opacity: blue = 0.502 * 0.5 over black.
    for (x, y) in [(2, 0), (3, 1)] {
        assert_near(pixel(&captured.rgba8, 4, x, y), [0, 0, 64, 255], "quad");
    }
}

#[tokio::test]
#[ignore = "requires a native hardware GPU runner"]
async fn crossfade_samples_textures_on_separate_atlas_pages_on_hardware() {
    let mut renderer = hardware_renderer().await;
    let large = |resource_id: &str, rgba| solid_texture(resource_id, 4100, 2100, rgba);
    let captured = renderer
        .render(&SceneFrame {
            sequence: 1,
            width: 2,
            height: 1,
            clear_rgba: [0, 0, 0, 255],
            commands: vec![
                // Two 4100x2100 backgrounds overflow an 8192x4096 page on both
                // axes, so each lands on its own page.
                large("crossfade.background.previous", [255, 0, 0, 255]),
                large("crossfade.background.current", [0, 0, 255, 255]),
                SceneCommand::CrossfadeSprite {
                    id: "crossfade.backgrounds".into(),
                    previous_texture_id: "crossfade.background.previous".into(),
                    current_texture_id: "crossfade.background.current".into(),
                    source: None,
                    destination: RectI::new(0, 0, 2, 1),
                    progress: 0.25,
                    opacity: 1.0,
                },
            ],
            semantics: None,
        })
        .unwrap();
    for x in 0..2 {
        assert_near(
            pixel(&captured.rgba8, 2, x, 0),
            [191, 0, 64, 255],
            "cross-page crossfade",
        );
    }
}

fn projective_quad(
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    z: f32,
    w: f32,
    rgba: [f32; 4],
) -> Vec<ProjectiveMeshVertex2D> {
    [[left, top], [right, top], [left, bottom], [right, bottom]]
        .map(|[x, y]| ProjectiveMeshVertex2D {
            position: [x * w, y * w, z, w],
            uv: [0.0, 0.0],
            premultiplied_rgba: rgba,
        })
        .to_vec()
}

#[tokio::test]
#[ignore = "requires a native hardware GPU runner"]
async fn projective_mesh_depth_test_keeps_nearer_surface_on_hardware() {
    let mut renderer = hardware_renderer().await;
    let mesh = |id: &str, vertices: Vec<ProjectiveMeshVertex2D>, depth: MeshDepth2D| {
        SceneCommand::ProjectiveMesh2D {
            id: id.into(),
            vertices: vertices.into(),
            indices: vec![0, 1, 2, 2, 1, 3].into(),
            material: MeshMaterial2D::Solid,
            texture_id: None,
            texture_filter: TextureFilter2D::Nearest,
            opacity: 1.0,
            blend: BlendMode::Alpha,
            depth,
        }
    };
    let captured = renderer
        .render(&SceneFrame {
            sequence: 1,
            width: 4,
            height: 4,
            clear_rgba: [0, 0, 0, 255],
            commands: vec![
                // Near red square in homogeneous form (w = 2, z/w = 0.2).
                mesh(
                    "projective.near",
                    projective_quad(0.0, 0.0, 2.0, 2.0, 0.4, 2.0, [1.0, 0.0, 0.0, 1.0]),
                    MeshDepth2D::ReadWriteLess,
                ),
                // Far blue square drawn later over the whole target.
                mesh(
                    "projective.far",
                    projective_quad(0.0, 0.0, 4.0, 4.0, 0.8, 1.0, [0.0, 0.0, 1.0, 1.0]),
                    MeshDepth2D::ReadWriteLess,
                ),
                // Depth-disabled green bottom row ignores the depth buffer.
                mesh(
                    "projective.overlay",
                    projective_quad(0.0, 3.0, 4.0, 4.0, 0.0, 1.0, [0.0, 1.0, 0.0, 1.0]),
                    MeshDepth2D::Disabled,
                ),
            ],
            semantics: None,
        })
        .unwrap();
    assert_eq!(pixel(&captured.rgba8, 4, 0, 0), [255, 0, 0, 255]);
    assert_eq!(pixel(&captured.rgba8, 4, 1, 1), [255, 0, 0, 255]);
    assert_eq!(pixel(&captured.rgba8, 4, 3, 0), [0, 0, 255, 255]);
    assert_eq!(pixel(&captured.rgba8, 4, 2, 2), [0, 0, 255, 255]);
    assert_eq!(pixel(&captured.rgba8, 4, 0, 3), [0, 255, 0, 255]);
}
