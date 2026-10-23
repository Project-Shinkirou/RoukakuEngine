#![cfg(all(target_os = "windows", feature = "platform-test-driver"))]

use roukaku_platform::{PlatformErrorCode, PlatformHostFactory, PlatformHostProfile};

#[tokio::test(flavor = "current_thread")]
async fn same_game_profile_rejects_a_second_live_host() {
    let temp = tempfile::tempdir().unwrap();
    let profile = PlatformHostProfile::windows_release("nativevn-game", "com.example.single");
    let factory = roukaku_platform_windows::factory_with_test_roots(temp.path(), temp.path());
    let first = factory
        .start(roukaku_platform::HostLaunchProfile::platform(
            profile.clone(),
        ))
        .await
        .unwrap();
    let error = match factory
        .start(roukaku_platform::HostLaunchProfile::platform(profile))
        .await
    {
        Ok(_) => panic!("second host unexpectedly started"),
        Err(error) => error,
    };
    assert_eq!(error.code, PlatformErrorCode::AlreadyInUse);
    first.client.shutdown().await.unwrap();
}
