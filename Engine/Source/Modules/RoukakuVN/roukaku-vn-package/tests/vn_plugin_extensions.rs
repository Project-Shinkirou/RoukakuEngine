use std::{path::Path, process::Command};

use roukaku_plugin::{dylib_path, PluginGate, PluginLoader, PluginRegistrar};
use roukaku_vn_package::{VnExtensionBinding, VnExtensionManifest};
use semver::Version;

#[test]
fn standard_vn_extension_manifest_declares_required_bindings() {
    let manifest = VnExtensionManifest::standard();
    let report = manifest.validate_required();

    assert!(report.passed, "{report:?}");
    assert!(manifest.has_binding("roukaku.vn.policy_bundle_provider"));
    assert!(manifest.has_binding("roukaku.vn.command_provider"));
    assert!(manifest.has_binding("roukaku.vn.presentation_command_provider"));
    assert!(manifest.has_binding("roukaku.vn.editor_metadata_provider"));
    assert!(manifest.has_binding("roukaku.vn.release_check_provider"));
}

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
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "ROUKAKU_VN_EXTENSION_BINDING_DUPLICATE"));
}

#[test]
fn vn_extension_manifest_accepts_real_cdylib_provider_fixture() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap();
    let status = Command::new("cargo")
        .args(["build", "-p", "vn-extension-provider"])
        .current_dir(root)
        .status()
        .unwrap();
    assert!(status.success());

    let mut registrar = PluginRegistrar::default();
    let loader = PluginLoader::new(PluginGate {
        engine_version: Version::parse("0.1.0").unwrap(),
        rustc_fingerprint: "rustc-stable".to_string(),
        feature_fingerprint: "stage3-vn".to_string(),
        abi_fingerprint: "roukaku-plugin-abi-v3".to_string(),
        required_capabilities: vec![
            "roukaku.vn.policy_bundle".to_string(),
            "roukaku.vn.command".to_string(),
            "roukaku.vn.presentation_command".to_string(),
            "roukaku.vn.editor_metadata".to_string(),
            "roukaku.vn.release_check".to_string(),
        ],
        required_permissions: vec!["runtime.vn".to_string()],
    });
    let plugin = loader
        .load(dylib_path(root, "vn_extension_provider"), &mut registrar)
        .unwrap();

    assert_eq!(
        plugin.descriptor().id,
        "roukaku.fixture.vn_extension_provider"
    );
    let snapshot = registrar.extension_registry_snapshot().unwrap();
    let manifest = VnExtensionManifest {
        schema: "roukaku.vn.extension_manifest.v1".to_string(),
        bindings: snapshot
            .providers
            .into_iter()
            .map(|provider| VnExtensionBinding {
                extension_point: provider.slot,
                provider_id: provider.provider_id,
                required_capabilities: vec![provider.capability],
            })
            .collect(),
    };

    let report = manifest.validate_required();
    assert!(report.passed, "{report:?}");

    let unload = plugin.unload_from(&mut registrar).unwrap();
    assert_eq!(unload.status, "unloaded");
    assert!(registrar.extensions.providers().is_empty());
}
