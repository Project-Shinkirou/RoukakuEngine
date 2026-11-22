use roukaku_media_core::{
    BlendMode, FilterGraph, FilterNode, FilterTarget, RectI, SceneCommand, TextureFrame,
};
use roukaku_platform::SceneFrame;
use roukaku_platform_common::{WgpuOffscreenRenderer, native_wgpu_instance};

fn scene(sequence: u64, width: u32, height: u32, filtered: bool) -> SceneFrame {
    let mut commands = vec![SceneCommand::rect(
        "background",
        0,
        0,
        width,
        height,
        [26, 31, 40, 255],
    )];
    for index in 0..6u8 {
        commands.push(SceneCommand::Texture {
            id: format!("layer-{index}"),
            frame: TextureFrame {
                width: 2,
                height: 2,
                rgba8: [index * 30, 90, 190, 126].repeat(4).into(),
            },
            destination: RectI::new(i32::from(index), 0, width, height),
            opacity: 0.52,
            blend: BlendMode::Alpha,
        });
    }
    if filtered {
        let nodes = [0.8, 0.65]
            .into_iter()
            .enumerate()
            .map(|(index, amount)| FilterNode {
                id: format!("fade-{index}"),
                kind: "roukaku.filter.fade".into(),
                input: FilterTarget::Final,
                output: FilterTarget::Final,
                params: [("amount".into(), amount.into())].into(),
                deterministic: true,
                allow_cpu_fallback: false,
            })
            .collect();
        commands.push(SceneCommand::FilterGraph {
            graph: FilterGraph {
                schema: "roukaku.filter_graph.v1".into(),
                nodes,
            },
        });
    }
    SceneFrame {
        sequence,
        width,
        height,
        clear_rgba: [0, 0, 0, 255],
        commands,
        semantics: None,
    }
}

#[tokio::test]
#[ignore = "requires a native hardware GPU runner"]
async fn combined_capture_matches_explicit_capture_and_submits_once() {
    let instance = native_wgpu_instance().unwrap();
    let mut renderer = WgpuOffscreenRenderer::new(&instance).await.unwrap();
    assert_ne!(renderer.identity().device_type, "cpu");

    for (sequence, (width, filtered)) in [(128, false), (131, true)].into_iter().enumerate() {
        let mut frame = scene(sequence as u64 * 2 + 1, width, 65, filtered);
        let combined = renderer.render(&frame).unwrap();
        assert_eq!(renderer.performance_counters().queue_submissions, 1);
        assert_eq!(
            renderer.performance_counters().readback_bytes,
            u64::from(width) * 65 * 4
        );

        frame.sequence += 1;
        renderer.submit_frame(&frame).unwrap();
        let separate = renderer.capture_checkpoint().unwrap();
        assert_eq!(
            renderer.performance_counters().queue_submissions,
            if filtered { 3 } else { 2 }
        );
        assert_eq!((combined.width, combined.height), (width, 65));
        assert_eq!(combined.rgba8, separate.rgba8);
    }
}
