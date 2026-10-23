#![cfg(not(target_arch = "wasm32"))]

use roukaku_platform::{PackageSourcePolicy, PlatformErrorCode};
use roukaku_platform_common::HttpRangeClient;

#[test]
fn https_client_requires_an_explicit_allowlist_policy() {
    let error = HttpRangeClient::from_policies(&[]).err().unwrap();
    assert_eq!(error.code, PlatformErrorCode::PermissionDenied);
}

#[test]
fn https_client_rejects_unapproved_origin_before_network_access() {
    let client = HttpRangeClient::from_policies(&[PackageSourcePolicy::HttpsRange {
        allowed_origins: vec!["https://cdn.example.test".to_string()],
    }])
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let mut cache = roukaku_platform_common::VerifiedPackageCache::open(
        temp.path(),
        roukaku_platform::PackageCachePolicy {
            max_entry_bytes: 64,
            max_total_bytes: 128,
        },
    )
    .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    assert_eq!(
        runtime
            .block_on(client.fetch_into_cache(
                "https://other.example.test/game.roukakupkg",
                "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                &mut cache
            ))
            .unwrap_err()
            .code,
        PlatformErrorCode::PermissionDenied
    );
}
