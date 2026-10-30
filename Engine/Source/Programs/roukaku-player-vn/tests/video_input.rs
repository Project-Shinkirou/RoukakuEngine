mod support;

use roukaku_media_core::TextureFrame;
use roukaku_player_vn::NativeVnHostCommandSource;
use roukaku_ui_core::{UiButtonState, UiInputEventKind, UiPoint, UiPointerButton};

fn video_source(skippable: bool, wait: bool) -> NativeVnHostCommandSource {
    let end_policy = if wait {
        "wait"
    } else {
        "continue"
    };
    let wait_call = if wait {
        "    rk.call(\"roukaku.vn.stage\", \"wait\", \"wait.movie\", \"movie.end\")\n"
    } else {
        ""
    };
    support::source_for_video(&format!(
        r#"local paragraphs = {{}}
paragraphs.start = function()
    rk.call("roukaku.vn.stage", "skip_allowed", "skip.policy", {{allowed = {skippable}}})
    rk.call("roukaku.vn.stage", "movie", "movie.intro", {{layer = "video", asset = "asset:/video/intro", loop_mode = "loop", ["end"] = "{end_policy}", fence = "movie.end", fallback = "asset:/video/intro-fallback", interrupt = "reject"}})
    {wait_call}    rk.call("roukaku.vn.dialog", "show", "line.after", {{key = "line.after"}})
end
return {{ paragraphs = paragraphs }}"#
    ))
}

fn press_key(source: &mut NativeVnHostCommandSource, key: &str) {
    for state in [UiButtonState::Pressed, UiButtonState::Released] {
        source
            .prepare_ui_input(UiInputEventKind::Keyboard {
                physical_key: key.into(),
                logical_key: key.into(),
                state,
                repeat: false,
                modifiers: 0,
            })
            .unwrap();
    }
}

#[test]
fn physical_advance_cancels_only_an_allowed_movie_wait_and_rejects_late_frames() {
    for (skippable, wait) in [(true, true), (false, true), (true, false)] {
        let mut source = video_source(skippable, wait);
        source.launch().unwrap();
        source
            .prepare_ui_input(UiInputEventKind::Focus { focused: true })
            .unwrap();
        let request = source.take_video_requests().remove(0);
        press_key(&mut source, "Enter");
        let should_skip = skippable && wait;
        assert_eq!(request.is_cancelled(), should_skip);
        let completions = source.take_stage_completions();
        if should_skip {
            assert_eq!(completions, ["movie.end"]);
            let error = source
                .bind_decoded_video_frame(
                    &request,
                    TextureFrame::from_vec(1, 1, vec![0, 0, 0, 255]).unwrap(),
                    true,
                )
                .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("ROUKAKU_PLAYER_VIDEO_REQUEST_STALE")
            );
            source.complete_wait(completions[0].clone()).unwrap();
            assert_ne!(
                source.pending_wait().map(|wait| wait.fence.as_str()),
                Some("movie.end")
            );
        } else {
            assert!(completions.is_empty());
        }
        source.release_resources().unwrap();
        source.shutdown().unwrap();
    }
}

#[test]
fn system_page_enter_returns_to_the_movie_without_skipping_it() {
    let mut source = video_source(true, true);
    source.launch().unwrap();
    source
        .prepare_ui_input(UiInputEventKind::Focus { focused: true })
        .unwrap();
    let request = source.take_video_requests().remove(0);
    source
        .prepare_ui_input(UiInputEventKind::PointerButton {
            button: UiPointerButton::Secondary,
            state: UiButtonState::Pressed,
            position: UiPoint { x: 12.0, y: 12.0 },
        })
        .unwrap();
    source
        .prepare_ui_input(UiInputEventKind::PointerButton {
            button: UiPointerButton::Secondary,
            state: UiButtonState::Released,
            position: UiPoint { x: 12.0, y: 12.0 },
        })
        .unwrap();
    press_key(&mut source, "Tab");
    assert_eq!(
        source
            .product_observation_evidence()
            .unwrap()
            .active_system_page,
        Some(roukaku_vn_core::SystemPageKind::QuickPanel)
    );
    assert_eq!(
        source
            .product_observation_evidence()
            .unwrap()
            .focused_semantic_id
            .as_deref(),
        Some("root/back")
    );
    press_key(&mut source, "Enter");
    assert_eq!(
        source
            .product_observation_evidence()
            .unwrap()
            .active_system_page,
        None
    );
    assert!(!request.is_cancelled());
    assert!(source.take_stage_completions().is_empty());
    assert_eq!(
        source.pending_wait().map(|wait| wait.fence.as_str()),
        Some("movie.end")
    );
    source.release_resources().unwrap();
    source.shutdown().unwrap();
}
