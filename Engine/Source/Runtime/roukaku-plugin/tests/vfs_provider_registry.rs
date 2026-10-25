use roukaku_plugin::{
    EngineModuleSlot, LoadPhase, PluginRegistrar, ProviderBindingContext, RegisteredProvider,
    provider_binding_context_from_runtime_package,
};

fn vfs_provider(provider_id: &str, capability: &str) -> RegisteredProvider {
    RegisteredProvider {
        slot: EngineModuleSlot("vfs_provider".to_string()),
        provider_id: provider_id.to_string(),
        capability: capability.to_string(),
        phase: LoadPhase::Runtime,
        packaged: true,
        engine_version: "0.1.0".to_string(),
        rustc_fingerprint: "rustc-stable".to_string(),
        feature_fingerprint: "runtime-typed-v3".to_string(),
        abi_fingerprint: "roukaku-plugin-abi-v3".to_string(),
    }
}

fn context(capability: &str) -> ProviderBindingContext {
    provider_binding_context_from_runtime_package(
        &roukaku_runtime::PackageHandle::default(),
        capability,
    )
}

#[test]
fn vfs_provider_registry_allows_multiple_providers_on_single_slot() {
    let mut registrar = PluginRegistrar::default();
    registrar
        .register_provider(vfs_provider("roukaku.vfs.package", "vfs.backend.package"))
        .unwrap();
    registrar
        .register_provider(vfs_provider("roukaku.vfs.local", "vfs.backend.local"))
        .unwrap();
    registrar
        .register_provider(vfs_provider(
            "roukaku.vfs.fvp",
            "vfs.backend.legacy_pack.fvp",
        ))
        .unwrap();

    let snapshot = registrar.extension_registry_snapshot().unwrap();
    assert_eq!(snapshot.providers.len(), 3);
    assert!(
        snapshot
            .providers
            .iter()
            .all(|provider| provider.slot == "vfs_provider")
    );
    assert!(snapshot.bindings.is_empty());
    assert!(snapshot.conflicts.is_empty());
}

#[test]
fn renderer_registry_keeps_explicit_single_binding_conflicts() {
    let mut registrar = PluginRegistrar::default();
    let slot = EngineModuleSlot("presentation".to_string());
    registrar
        .register_provider(RegisteredProvider {
            slot: slot.clone(),
            provider_id: "roukaku.renderer.wgpu".to_string(),
            capability: "renderer2d.wgpu".to_string(),
            phase: LoadPhase::Runtime,
            packaged: true,
            engine_version: "0.1.0".to_string(),
            rustc_fingerprint: "rustc-stable".to_string(),
            feature_fingerprint: "runtime-typed-v3".to_string(),
            abi_fingerprint: "roukaku-plugin-abi-v3".to_string(),
        })
        .unwrap();
    registrar
        .bind_provider(&slot, "roukaku.renderer.wgpu", context("renderer2d.wgpu"))
        .unwrap();
    registrar
        .register_provider(RegisteredProvider {
            slot: slot.clone(),
            provider_id: "roukaku.runtime.shinki_emu".to_string(),
            capability: "runtime.shinki_emu".to_string(),
            phase: LoadPhase::Runtime,
            packaged: true,
            engine_version: "0.1.0".to_string(),
            rustc_fingerprint: "rustc-stable".to_string(),
            feature_fingerprint: "runtime-typed-v3".to_string(),
            abi_fingerprint: "roukaku-plugin-abi-v3".to_string(),
        })
        .unwrap();
    assert!(
        registrar
            .bind_provider(
                &slot,
                "roukaku.runtime.shinki_emu",
                context("runtime.shinki_emu"),
            )
            .unwrap_err()
            .contains("ROUKAKU_PLUGIN_BINDING_CONFLICT")
    );

    let snapshot = registrar.extension_registry_snapshot().unwrap();
    assert_eq!(snapshot.conflicts.len(), 1);
    assert_eq!(snapshot.conflicts[0].slot, "presentation");
}
