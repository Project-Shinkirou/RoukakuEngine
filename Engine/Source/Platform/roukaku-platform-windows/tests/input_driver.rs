#![cfg(all(target_os = "windows", feature = "platform-test-driver"))]

use std::time::Duration;

use roukaku_platform::{
    InputState, PlatformEventKind, PlatformHostFactory, PlatformHostProfile, WindowRequest,
};

#[tokio::test]
#[ignore = "requires a hardware GPU adapter"]
async fn sendinput_driver_reaches_the_platform_host_event_stream() {
    let profile = PlatformHostProfile::windows_release("nativevn-game", "com.example.test");
    let mut session = roukaku_platform_windows::factory()
        .start(roukaku_platform::HostLaunchProfile::platform(profile))
        .await
        .unwrap();
    let window = session
        .client
        .create_window(WindowRequest {
            title: "Roukaku Platform Input Driver Test".to_string(),
            width: 320,
            height: 180,
            visible: true,
        })
        .await
        .unwrap();
    let native = roukaku_platform_windows::WindowsTestDriver::wait_for_window(
        std::process::id(),
        "Roukaku Platform Input Driver Test",
        Duration::from_secs(2),
    )
    .unwrap();
    native.focus().unwrap();
    let captured = native.capture_rgba().unwrap();
    assert_eq!((captured.width, captured.height), (320, 180));
    assert_eq!(captured.rgba8.len(), 320 * 180 * 4);
    native.send_key(0x20).unwrap();

    let event = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let event = session.events.recv().await.unwrap();
            if matches!(
                event.kind,
                PlatformEventKind::Keyboard {
                    state: InputState::Pressed,
                    ..
                }
            ) {
                break event;
            }
        }
    })
    .await
    .expect("SendInput event reached host");
    assert!(matches!(event.kind, PlatformEventKind::Keyboard { .. }));

    session.client.destroy_window(window).await.unwrap();
    session.client.shutdown().await.unwrap();
}
