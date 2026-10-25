use roukaku_vn_presentation::{VnPresentationProviderManifest, VnWaitKind};

#[test]
fn presentation_provider_manifest_blocks_unknown_and_wrong_command_presets() {
    let manifest = VnPresentationProviderManifest::standard();

    assert_eq!(
        manifest
            .resolve_preset("advanced-vn", "show", "missing")
            .unwrap_err()
            .code,
        "ROUKAKU_VN_PRESENTATION_PRESET_UNDECLARED"
    );
    assert_eq!(
        manifest
            .resolve_preset("advanced-vn", "show", "slow_push")
            .unwrap_err()
            .code,
        "ROUKAKU_VN_PRESENTATION_PRESET_COMMAND"
    );
}

#[test]
fn presentation_provider_manifest_blocks_duplicate_and_over_budget_policy() {
    let mut manifest = VnPresentationProviderManifest::standard();
    manifest.presets.push(manifest.presets[0].clone());
    manifest.presets[0]
        .command_kinds
        .push("background".to_string());
    manifest.profiles[0].max_effect_budget_us = 1;
    manifest.profiles[0]
        .allowed_filters
        .push("roukaku.filter.fade".to_string());

    let report = manifest.validate_standard();

    assert!(!report.passed);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ROUKAKU_VN_PRESENTATION_PRESET_ID")
    );
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ROUKAKU_VN_PRESENTATION_PROFILE_PRESET_BUDGET"
        })
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ROUKAKU_VN_PRESENTATION_PRESET_COMMAND")
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ROUKAKU_VN_PRESENTATION_PROFILE_FILTER")
    );
}

#[test]
fn presentation_provider_manifest_blocks_missing_movie_await_capability() {
    let mut manifest = VnPresentationProviderManifest::standard();
    manifest
        .wait_capabilities
        .retain(|kind| *kind != VnWaitKind::MovieEnd);

    let report = manifest.validate_standard();

    assert!(!report.passed);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ROUKAKU_VN_PRESENTATION_WAIT_CAPABILITY")
    );
}
