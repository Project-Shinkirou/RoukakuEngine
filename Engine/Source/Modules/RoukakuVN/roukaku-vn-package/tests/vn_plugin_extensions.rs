use roukaku_vn_package::{VnExtensionBinding, VnExtensionManifest};

#[test]
fn missing_vn_extension_binding_blocks_validation() {
    let mut manifest = VnExtensionManifest::standard();
    manifest
        .bindings
        .retain(|binding| binding.extension_point != "roukaku.vn.presentation_command_provider");

    let report = manifest.validate_required();

    assert!(!report.passed);
    assert_eq!(
        report.diagnostics[0].code,
        "ROUKAKU_VN_EXTENSION_BINDING_MISSING"
    );
}

#[test]
fn duplicate_vn_extension_binding_blocks_validation() {
    let mut manifest = VnExtensionManifest::standard();
    manifest.bindings.push(VnExtensionBinding {
        extension_point: "roukaku.vn.command_provider".to_string(),
        provider_id: "roukaku.vn.standard.alt".to_string(),
        required_capabilities: vec!["roukaku.vn.command".to_string()],
    });

    let report = manifest.validate_required();

    assert!(!report.passed);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ROUKAKU_VN_EXTENSION_BINDING_DUPLICATE")
    );
}
