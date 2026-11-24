use roukaku_media_core::{
    GlyphBitmap, GlyphBitmapFormat, GlyphInstance, SceneCommand, SceneCompositing2D,
};
use roukaku_platform::SceneFrame;
use roukaku_platform_common::WgpuOffscreenRenderer;
use std::sync::Arc;

fn glyphs(x: i32) -> Arc<[GlyphInstance]> {
    [GlyphInstance {
        resource_id: "edge".into(),
        x,
        y: 0,
        rotation_quadrants: 0,
    }]
    .into()
}

fn rect(id: &str, x: u32, width: u32, rgba: [u8; 4]) -> SceneCommand {
    SceneCommand::Rect {
        id: id.into(),
        x,
        y: 0,
        width,
        height: 1,
        rgba,
    }
}

fn linear(x: i32) -> SceneCommand {
    SceneCommand::LinearGlyphRun {
        id: format!("linear-{x}"),
        glyphs: glyphs(x),
        rgba: [255; 4],
        opacity: 1.0,
    }
}

#[tokio::test]
#[ignore = "requires a native hardware GPU runner"]
async fn linear_glyphs_preserve_encoded_draw_order_and_transparent_alpha() {
    let mut renderer =
        WgpuOffscreenRenderer::new(&roukaku_platform_common::native_wgpu_instance().unwrap())
            .await
            .unwrap()
            .with_default_compositing(SceneCompositing2D::EncodedSrgb);
    let render_scene = |sequence, commands| SceneFrame {
        sequence,
        width: 4,
        height: 1,
        clear_rgba: [0; 4],
        commands,
        semantics: None,
    };
    let mut commands = vec![
        rect("opaque-backdrop", 0, 2, [0, 0, 0, 255]),
        SceneCommand::UploadGlyph {
            resource_id: "edge".into(),
            glyph: GlyphBitmap {
                width: 1,
                height: 1,
                format: GlyphBitmapFormat::Alpha8,
                pixels: vec![128].into(),
            },
        },
        rect("encoded-white", 0, 1, [255, 255, 255, 128]),
        rect("translucent-red", 2, 2, [255, 0, 0, 128]),
        linear(1),
        linear(2),
    ];
    let expected = [
        [128, 128, 128, 255],
        [188, 188, 188, 255],
        [192, 160, 160, 192],
        [128, 0, 0, 128],
    ];
    let first = renderer.render(&render_scene(1, commands.clone())).unwrap();
    commands.retain(|command| !matches!(command, SceneCommand::UploadGlyph { .. }));
    // An encoded draw after linear text must see the already drawn text, not
    // force a text overlay above the next sprite or change the sprite's blend.
    commands.push(rect("foreground-blue", 1, 1, [0, 0, 255, 128]));
    let second = renderer.render(&render_scene(2, commands)).unwrap();
    let mut expected_with_later_draw = expected;
    expected_with_later_draw[1] = [94, 94, 222, 255];
    for (capture, colors) in [(first, expected), (second, expected_with_later_draw)] {
        assert_eq!(capture.rgba8.len(), colors.len() * 4);
        for (index, (actual, expected)) in capture.rgba8.chunks_exact(4).zip(colors).enumerate() {
            assert!(
                actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 1),
                "pixel {index}: {actual:?}, expected {expected:?}"
            );
        }
    }
}
