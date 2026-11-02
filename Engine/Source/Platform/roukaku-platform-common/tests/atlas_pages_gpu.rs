use roukaku_media_core::{BlendMode, RectI, SceneCommand, SceneCompositing2D, TextureFrame};
use roukaku_platform::SceneFrame;
use roukaku_platform_common::WgpuOffscreenRenderer;

const LARGE_WIDTH: u32 = 2730;
const LARGE_HEIGHT: u32 = 1365;
const LARGE_COUNT: usize = 9;
/// Bytes of one 8192x4096 RGBA8 atlas page.
const MAX_PAGE_BYTES: u64 = 8192 * 4096 * 4;

fn color(index: usize) -> [u8; 4] {
    [
        (index * 28) as u8,
        255 - (index * 28) as u8,
        (index * 97 % 256) as u8,
        255,
    ]
}

fn solid_texture(width: u32, height: u32, rgba: [u8; 4]) -> TextureFrame {
    TextureFrame {
        width,
        height,
        rgba8: rgba.repeat((width * height) as usize).into(),
    }
}

fn sprite(index: usize, texture_id: String) -> SceneCommand {
    SceneCommand::Sprite {
        id: format!("pages.sprite.{index}"),
        texture_id,
        source: Some(RectI::new(8, 8, 2, 2)),
        destination: RectI::new(index as i32, 0, 1, 1),
        opacity: 1.0,
        blend: BlendMode::Opaque,
    }
}

fn assert_columns(rgba8: &[u8], expected: &[[u8; 4]], frame: &str) {
    for (index, (pixel, expected)) in rgba8.as_chunks::<4>().0.iter().zip(expected).enumerate() {
        assert_eq!(pixel, expected, "{frame}: column {index}");
    }
}

#[tokio::test]
#[ignore = "requires a native hardware GPU runner"]
async fn texture_set_overflowing_one_atlas_page_samples_every_page() {
    let mut renderer =
        WgpuOffscreenRenderer::new(&roukaku_platform_common::native_wgpu_instance().unwrap())
            .await
            .unwrap()
            .with_default_compositing(SceneCompositing2D::EncodedSrgb);
    assert!(matches!(
        renderer.identity().device_type.as_str(),
        "integrated_gpu" | "discrete_gpu"
    ));

    // Nine 2730x1365 backgrounds (~134 MB) cannot share one 8192x4096 page.
    let mut commands = (0..LARGE_COUNT)
        .map(|index| SceneCommand::UploadTexture {
            resource_id: format!("pages.large.{index}"),
            frame: solid_texture(LARGE_WIDTH, LARGE_HEIGHT, color(index)),
        })
        .collect::<Vec<_>>();
    commands.extend((0..LARGE_COUNT).map(|index| sprite(index, format!("pages.large.{index}"))));
    let width = LARGE_COUNT as u32 + 1;
    let captured = renderer
        .render(&SceneFrame {
            sequence: 1,
            width,
            height: 1,
            clear_rgba: [0, 0, 0, 255],
            commands,
            semantics: None,
        })
        .unwrap();
    let mut expected = (0..LARGE_COUNT).map(color).collect::<Vec<_>>();
    expected.push([0, 0, 0, 255]);
    assert_columns(&captured.rgba8, &expected, "initial frame");
    assert!(renderer.performance_counters().atlas_bytes > MAX_PAGE_BYTES);

    // The retained pages take an incremental upload and keep serving draws
    // in reverse order, interleaving pages within one frame.
    let small = [200, 40, 10, 255];
    let mut commands = vec![SceneCommand::UploadTexture {
        resource_id: "pages.small".into(),
        frame: solid_texture(16, 16, small),
    }];
    commands.extend(
        (0..LARGE_COUNT)
            .rev()
            .enumerate()
            .map(|(column, index)| sprite(column, format!("pages.large.{index}"))),
    );
    commands.push(sprite(LARGE_COUNT, "pages.small".to_owned()));
    let captured = renderer
        .render(&SceneFrame {
            sequence: 2,
            width,
            height: 1,
            clear_rgba: [0, 0, 0, 255],
            commands,
            semantics: None,
        })
        .unwrap();
    let mut expected = (0..LARGE_COUNT).rev().map(color).collect::<Vec<_>>();
    expected.push(small);
    assert_columns(&captured.rgba8, &expected, "retained frame");
}
