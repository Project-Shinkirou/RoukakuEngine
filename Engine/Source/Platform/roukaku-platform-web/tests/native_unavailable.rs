#![cfg(not(target_arch = "wasm32"))]

use roukaku_platform::{PlatformHostFactory, PlatformHostProfile};

#[tokio::test]
async fn native_process_cannot_construct_a_fake_web_host() {
    let profile = PlatformHostProfile::web_release("nativevn-web", "com.example.game");
    let error = roukaku_platform_web::factory()
        .start(roukaku_platform::HostLaunchProfile::platform(profile))
        .await
        .err()
        .expect("native Web host must be unavailable");
    assert_eq!(
        error.code,
        roukaku_platform::PlatformErrorCode::UnsupportedPlatform
    );
    let report = roukaku_platform_web::probe(Some("nativevn-web"));
    assert!(report.renderer.available.is_empty());
}
