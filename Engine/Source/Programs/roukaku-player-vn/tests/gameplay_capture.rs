#[path = "support/native_package.rs"]
mod native_package;
use native_package::*;
use roukaku_package::PackageReader;
use roukaku_player_core::{PlayerHostCommand, PlayerHostCommandBatch, PlayerHostResourceId};
use roukaku_player_vn::NativeVnHostCommandSource;
use roukaku_ui_core::{UiButtonState, UiInputEventKind, UiPoint, UiPointerButton};
use roukaku_vn_core::VnRunConfig;

#[test]
fn system_activation_captures_before_presentation_and_not_inside_system_ui() {
    let story = r#"local paragraphs = {}
paragraphs.start = function()
    rk.call("roukaku.vn.dialog", "show", "line.one", {key = "line.one", speaker = "hero", window = "message"})
end
return { paragraphs = paragraphs }"#;
    let ui = TEST_UI
        .replace("vn.request_exit", "vn.open_system page:save")
        .replace("quick_slot:\"\"", "quick_slot:\"slot.01\"");
    let bytes = product_package_with_ui_and_request(story, &ui, test_compile_options(), |_| {});
    let package = PackageReader::open(&bytes).unwrap();
    for event in [
        UiInputEventKind::AccessibilityAction {
            semantic_id: "root/exit".into(),
            action: "activate".into(),
            value: None,
        },
        UiInputEventKind::PointerButton {
            button: UiPointerButton::Secondary,
            state: UiButtonState::Pressed,
            position: UiPoint { x: 12.0, y: 12.0 },
        },
        UiInputEventKind::Keyboard {
            physical_key: "F5".into(),
            logical_key: "F5".into(),
            state: UiButtonState::Pressed,
            repeat: false,
            modifiers: 0,
        },
    ] {
        let mut source = NativeVnHostCommandSource::from_package(
            &package,
            VnRunConfig::classic("en"),
            320,
            180,
            PlayerHostResourceId(1),
        )
        .unwrap();
        source.launch().unwrap();
        let batch = source.prepare_ui_input(event).unwrap();
        assert!(matches!(
            batch.commands.first(),
            Some(PlayerHostCommand::CaptureSurface { .. })
        ));
        assert!(batch.commands.len() > 1);
        PlayerHostCommandBatch::new(batch.commands).unwrap();
        source.take_ui_host_request();
        let inside = source
            .prepare_ui_input(UiInputEventKind::PointerMove {
                position: UiPoint { x: 20.0, y: 20.0 },
            })
            .unwrap();
        assert!(
            !inside
                .commands
                .iter()
                .any(|command| matches!(command, PlayerHostCommand::CaptureSurface { .. }))
        );
        source.release_resources().unwrap();
        source.shutdown().unwrap();
    }
}

#[test]
fn ordinary_gameplay_input_does_not_read_back_the_gpu() {
    let bytes = product_package_with_request(
        r#"local paragraphs = {}
paragraphs.start = function()
    rk.call("roukaku.vn.dialog", "show", "line.one", {key = "line.one", speaker = "hero", window = "message"})
end
return { paragraphs = paragraphs }"#,
        |_| {},
    );
    let package = PackageReader::open(&bytes).unwrap();
    let mut source = NativeVnHostCommandSource::from_package(
        &package,
        VnRunConfig::classic("en"),
        320,
        180,
        PlayerHostResourceId(1),
    )
    .unwrap();
    source.launch().unwrap();
    for event in [
        UiInputEventKind::PointerMove {
            position: UiPoint { x: 20.0, y: 20.0 },
        },
        UiInputEventKind::Keyboard {
            physical_key: "Tab".into(),
            logical_key: "Tab".into(),
            state: UiButtonState::Pressed,
            repeat: false,
            modifiers: 0,
        },
    ] {
        let batch = source.prepare_ui_input(event).unwrap();
        assert!(
            !batch
                .commands
                .iter()
                .any(|command| matches!(command, PlayerHostCommand::CaptureSurface { .. }))
        );
    }
    source.release_resources().unwrap();
    source.shutdown().unwrap();
}
