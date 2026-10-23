#[test]
fn macos_probe_never_claims_runtime_availability_without_conformance() {
    let report = roukaku_platform_macos::probe(Some("nativevn-game"));
    assert!(report.renderer.available.is_empty());
    assert!(report.renderer.selected.is_none());
    #[cfg(target_os = "macos")]
    assert_eq!(
        report.diagnostics[0].code,
        "ROUKAKU_PLATFORM_RUNTIME_PROBE_REQUIRED"
    );
    #[cfg(not(target_os = "macos"))]
    assert_eq!(
        report.diagnostics[0].code,
        "ROUKAKU_PLATFORM_NOT_IMPLEMENTED"
    );
    let _factory = roukaku_platform_macos::factory();
}
