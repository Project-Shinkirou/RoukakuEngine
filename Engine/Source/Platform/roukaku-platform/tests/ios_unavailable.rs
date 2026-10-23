#[test]
fn ios_probe_is_explicitly_unavailable_until_stage6() {
    let report = roukaku_platform::probe_ios(Some("nativevn-game"));
    assert!(report.renderer.available.is_empty());
    assert!(report.renderer.selected.is_none());
    assert_eq!(report.diagnostics[0].code, "ROUKAKU_PLATFORM_NOT_IMPLEMENTED");
    let _factory = roukaku_platform::ios_factory();
}
