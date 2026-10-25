use roukaku_platform_android::{
    ANDROID_AGP_VERSION, ANDROID_BUILD_TOOLS, ANDROID_BUNDLE_MANIFEST_SCHEMA, ANDROID_COMPILE_SDK,
    ANDROID_GRADLE_VERSION, ANDROID_MIN_SDK, ANDROID_NDK_VERSION, ANDROID_TARGET_SDK,
    AndroidArtifactIdentity, AndroidBundleManifest, AndroidSigningMode,
};
use roukaku_release::{CheckStatus, ReleaseValidator};

fn hash() -> String {
    format!("sha256:{}", "b".repeat(64))
}

#[test]
fn android_bundle_without_same_run_e3_is_blocking() {
    let manifest = AndroidBundleManifest {
        schema: ANDROID_BUNDLE_MANIFEST_SCHEMA.to_string(),
        target: "nativevn-game".to_string(),
        profile: "android-release".to_string(),
        package_id: "com.roukaku.game".to_string(),
        package_hash: hash(),
        build_fingerprint: hash(),
        min_sdk: ANDROID_MIN_SDK,
        compile_sdk: ANDROID_COMPILE_SDK,
        target_sdk: ANDROID_TARGET_SDK,
        build_tools: ANDROID_BUILD_TOOLS.to_string(),
        ndk_version: ANDROID_NDK_VERSION.to_string(),
        agp_version: ANDROID_AGP_VERSION.to_string(),
        gradle_version: ANDROID_GRADLE_VERSION.to_string(),
        jdk_major: 27,
        jdk_version: "27.0.1".to_string(),
        toolchain: [
            "jdk_runtime",
            "android_build_tools",
            "android_ndk",
            "gradle_wrapper",
        ]
        .into_iter()
        .map(|kind| AndroidArtifactIdentity {
            kind: kind.to_string(),
            file_name: format!("{kind}.bin"),
            sha256: hash(),
        })
        .collect(),
        shipping_abis: vec!["arm64-v8a".to_string()],
        test_abis: vec!["arm64-v8a".to_string(), "x86_64".to_string()],
        native_library: AndroidArtifactIdentity {
            kind: "cdylib".to_string(),
            file_name: "libroukaku_player_android.so".to_string(),
            sha256: hash(),
        },
        artifacts: vec![AndroidArtifactIdentity {
            kind: "aab".to_string(),
            file_name: "player.aab".to_string(),
            sha256: hash(),
        }],
        signing_mode: AndroidSigningMode::Unsigned,
        cargo_features: vec!["interpreter-only".to_string()],
    };
    let check = ReleaseValidator.validate_android_bundle_manifest(
        &manifest,
        &hash(),
        "nativevn-game",
        "android-release",
        None,
        None,
    );
    assert_eq!(check.status, CheckStatus::Blocked);
}
