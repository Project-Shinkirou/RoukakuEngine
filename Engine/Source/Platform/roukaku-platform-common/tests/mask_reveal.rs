use roukaku_media_core::{
    MaskRevealMode, RectI, SceneCommand, SceneCompositing2D, TextureFrame, Transform2D,
};
use roukaku_platform::{CapturedFrame, SceneFrame};
use roukaku_platform_common::WgpuOffscreenRenderer;

async fn renderer(compositing: SceneCompositing2D) -> WgpuOffscreenRenderer {
    WgpuOffscreenRenderer::new(&roukaku_platform_common::native_wgpu_instance().unwrap())
        .await
        .unwrap()
        .with_default_compositing(compositing)
}

fn solid(width: u32, height: u32, rgba: [u8; 4]) -> TextureFrame {
    TextureFrame::from_vec(width, height, rgba.repeat((width * height) as usize)).unwrap()
}

fn reveal(
    image: &TextureFrame,
    mask: &TextureFrame,
    mode: MaskRevealMode,
    phase: f32,
) -> SceneCommand {
    SceneCommand::MaskReveal {
        id: "reveal".into(),
        frame: image.clone(),
        mask: mask.clone(),
        destination: RectI::new(0, 0, image.width, image.height),
        mode,
        phase,
        opacity: 1.0,
    }
}

fn scene(sequence: u64, width: u32, height: u32, commands: Vec<SceneCommand>) -> SceneFrame {
    SceneFrame {
        sequence,
        width,
        height,
        clear_rgba: [0, 0, 255, 255],
        commands,
        semantics: None,
    }
}

fn pixel(captured: &CapturedFrame, index: usize) -> [u8; 4] {
    captured.rgba8[index * 4..index * 4 + 4].try_into().unwrap()
}

fn assert_pixel(captured: &CapturedFrame, index: usize, expected: [u8; 4]) {
    let actual = pixel(captured, index);
    for (actual_channel, expected_channel) in actual.into_iter().zip(expected) {
        assert!(
            actual_channel.abs_diff(expected_channel) <= 1,
            "pixel {index}: {actual:?}, expected {expected:?} (one-byte GPU rounding tolerance)"
        );
    }
}

#[tokio::test]
#[ignore = "requires a native hardware GPU runner"]
async fn mask_reveal_gradient_and_threshold_follow_native_phase() {
    let mut renderer = renderer(SceneCompositing2D::EncodedSrgb).await;
    let image = solid(4, 1, [255, 0, 0, 255]);
    // Native inverted luminance, including intermediate byte thresholds.
    let mask = TextureFrame::from_vec(
        4,
        1,
        vec![
            255, 255, 255, 255, 128, 128, 128, 255, 64, 64, 64, 255, 0, 0, 0, 255,
        ],
    )
    .unwrap();
    let cases = [
        (MaskRevealMode::Gradient, 0.0, [0, 0, 0, 0]),
        (MaskRevealMode::Gradient, 256.0, [255, 128, 64, 0]),
        (MaskRevealMode::Gradient, 384.0, [255, 255, 192, 128]),
        (MaskRevealMode::Gradient, 512.0, [255; 4]),
        (MaskRevealMode::Threshold, 0.0, [0, 0, 0, 255]),
        (MaskRevealMode::Threshold, 63.0, [0, 0, 0, 255]),
        (MaskRevealMode::Threshold, 64.0, [0, 0, 255, 255]),
        (MaskRevealMode::Threshold, 127.0, [0, 0, 255, 255]),
        (MaskRevealMode::Threshold, 128.0, [0, 255, 255, 255]),
        (MaskRevealMode::Threshold, 256.0, [255; 4]),
    ];
    for (index, (mode, phase, alphas)) in cases.into_iter().enumerate() {
        let frame = scene(
            index as u64 + 1,
            4,
            1,
            vec![reveal(&image, &mask, mode, phase)],
        );
        let captured = renderer.render(&frame).unwrap();
        for (index, alpha) in alphas.into_iter().enumerate() {
            assert_pixel(&captured, index, [alpha, 0, 255 - alpha, 255]);
        }
        if index > 0 {
            // Phase changes upload six vertices, never image or mask pixels.
            assert_eq!(renderer.performance_counters().upload_bytes, 6 * 52);
        }
        renderer.render(&frame).unwrap();
        assert_eq!(renderer.performance_counters().upload_bytes, 0);
    }

    // Same draw ID with new pixels must invalidate the resident image.
    let green = solid(4, 1, [0, 255, 0, 255]);
    let captured = renderer
        .render(&scene(
            20,
            4,
            1,
            vec![reveal(&green, &mask, MaskRevealMode::Gradient, 512.0)],
        ))
        .unwrap();
    for index in 0..4 {
        assert_pixel(&captured, index, [0, 255, 0, 255]);
    }
    // New mask dimensions and pixels must invalidate only the mask placement.
    let bright = solid(1, 1, [255; 4]);
    let captured = renderer
        .render(&scene(
            21,
            4,
            1,
            vec![reveal(&green, &bright, MaskRevealMode::Threshold, 0.0)],
        ))
        .unwrap();
    for index in 0..4 {
        assert_pixel(&captured, index, [0, 0, 255, 255]);
    }
    // Grow both slots, then shrink the image again under the same draw ID.
    let grown_image = solid(8, 2, [0, 255, 0, 255]);
    let grown_mask = solid(8, 2, [0, 0, 0, 255]);
    let captured = renderer
        .render(&scene(
            22,
            8,
            2,
            vec![reveal(
                &grown_image,
                &grown_mask,
                MaskRevealMode::Threshold,
                0.0,
            )],
        ))
        .unwrap();
    for index in 0..16 {
        assert_pixel(&captured, index, [0, 255, 0, 255]);
    }
    let captured = renderer
        .render(&scene(
            23,
            4,
            1,
            vec![reveal(&image, &mask, MaskRevealMode::Threshold, 256.0)],
        ))
        .unwrap();
    for index in 0..4 {
        assert_pixel(&captured, index, [255, 0, 0, 255]);
    }
    renderer.render(&scene(24, 4, 1, vec![])).unwrap();
    let captured = renderer
        .render(&scene(
            25,
            4,
            1,
            vec![reveal(&image, &mask, MaskRevealMode::Threshold, 256.0)],
        ))
        .unwrap();
    for index in 0..4 {
        assert_pixel(&captured, index, [255, 0, 0, 255]);
    }
}

#[tokio::test]
#[ignore = "requires a native hardware GPU runner"]
async fn mask_luminance_stays_in_byte_domain_for_both_compositors() {
    let image = solid(1, 1, [255, 0, 0, 255]);
    let mask = solid(1, 1, [128, 128, 128, 255]);
    for compositing in [
        SceneCompositing2D::EncodedSrgb,
        SceneCompositing2D::LinearSrgb,
    ] {
        let mut renderer = renderer(compositing).await;
        for (sequence, phase, expected) in
            [(1, 127.0, [0, 0, 255, 255]), (2, 128.0, [255, 0, 0, 255])]
        {
            let captured = renderer
                .render(&scene(
                    sequence,
                    1,
                    1,
                    vec![reveal(&image, &mask, MaskRevealMode::Threshold, phase)],
                ))
                .unwrap();
            assert_pixel(&captured, 0, expected);
        }
        let captured = renderer
            .render(&scene(
                3,
                1,
                1,
                vec![reveal(&image, &mask, MaskRevealMode::Gradient, 256.0)],
            ))
            .unwrap();
        let expected = match compositing {
            SceneCompositing2D::EncodedSrgb => [128, 0, 127, 255],
            SceneCompositing2D::LinearSrgb => [188, 0, 187, 255],
        };
        assert_pixel(&captured, 0, expected);
    }
}

#[tokio::test]
#[ignore = "requires a native hardware GPU runner"]
async fn mask_reveal_preserves_alpha_opacity_clip_transform_and_pixel_mask() {
    let mut renderer = renderer(SceneCompositing2D::EncodedSrgb).await;
    let image = solid(2, 1, [255, 0, 0, 128]);
    let mask = solid(1, 1, [255; 4]);
    let mut command = reveal(&image, &mask, MaskRevealMode::Gradient, 512.0);
    let SceneCommand::MaskReveal { opacity, .. } = &mut command else {
        unreachable!()
    };
    *opacity = 0.5;
    let captured = renderer
        .render(&scene(
            1,
            8,
            2,
            vec![
                SceneCommand::PushClip {
                    rect: RectI::new(2, 1, 1, 1),
                },
                SceneCommand::PushTransform {
                    transform: Transform2D::translation(2.0, 1.0),
                },
                SceneCommand::PushOpacity { opacity: 0.5 },
                SceneCommand::PushPixelMask { bits: 1u64 << 53 },
                command.clone(),
                SceneCommand::PopPixelMask,
                SceneCommand::PopOpacity,
                SceneCommand::PopTransform,
                SceneCommand::PopClip,
            ],
        ))
        .unwrap();
    for index in 0..16 {
        assert_pixel(
            &captured,
            index,
            if index == 10 {
                [32, 0, 223, 255]
            } else {
                [0, 0, 255, 255]
            },
        );
    }
    let captured = renderer
        .render(&scene(
            2,
            8,
            2,
            vec![
                SceneCommand::PushPixelMask { bits: 0 },
                command,
                SceneCommand::PopPixelMask,
            ],
        ))
        .unwrap();
    for index in 0..16 {
        assert_pixel(&captured, index, [0, 0, 255, 255]);
    }
}

#[tokio::test]
#[ignore = "requires a native hardware GPU runner"]
async fn mask_reveal_maps_image_and_mask_dimensions_independently() {
    let mut renderer = renderer(SceneCompositing2D::EncodedSrgb).await;
    let image = TextureFrame::from_vec(2, 1, vec![255, 0, 0, 255, 0, 255, 0, 255]).unwrap();
    let mask = TextureFrame::from_vec(1, 2, vec![255, 255, 255, 255, 0, 0, 0, 255]).unwrap();
    let mut command = reveal(&image, &mask, MaskRevealMode::Gradient, 256.0);
    let SceneCommand::MaskReveal { destination, .. } = &mut command else {
        unreachable!()
    };
    *destination = RectI::new(0, 0, 4, 2);
    let captured = renderer.render(&scene(1, 4, 2, vec![command])).unwrap();
    assert_pixel(&captured, 0, [255, 0, 0, 255]);
    assert_pixel(&captured, 3, [0, 255, 0, 255]);
    for index in 4..8 {
        assert_pixel(&captured, index, [0, 0, 255, 255]);
    }
}

#[tokio::test]
#[ignore = "requires a native hardware GPU runner"]
async fn mask_reveal_rejects_invalid_payload_and_mode_phase_without_poisoning_cache() {
    let mut renderer = renderer(SceneCompositing2D::EncodedSrgb).await;
    let image = solid(1, 1, [255, 0, 0, 255]);
    let mask = solid(1, 1, [255; 4]);
    for (index, (mode, phase)) in [
        (MaskRevealMode::Threshold, 256.5),
        (MaskRevealMode::Gradient, 512.5),
        (MaskRevealMode::Gradient, -1.0),
        (MaskRevealMode::Threshold, f32::NAN),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(
            renderer
                .render(&scene(
                    index as u64 + 1,
                    1,
                    1,
                    vec![reveal(&image, &mask, mode, phase)]
                ))
                .is_err()
        );
    }
    let mut malformed = mask.clone();
    malformed.width = 2;
    assert!(
        renderer
            .render(&scene(
                5,
                1,
                1,
                vec![reveal(&image, &malformed, MaskRevealMode::Gradient, 512.0)]
            ))
            .is_err()
    );
    let captured = renderer
        .render(&scene(
            6,
            1,
            1,
            vec![reveal(&image, &mask, MaskRevealMode::Gradient, 512.0)],
        ))
        .unwrap();
    assert_pixel(&captured, 0, [255, 0, 0, 255]);
}
