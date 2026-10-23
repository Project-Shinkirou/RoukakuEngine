use abi_stable::{
    prefix_type::PrefixTypeTrait,
    std_types::{RString, RVec},
};
use roukaku_plugin_abi::{
    FfiPluginRegistration, FfiPluginShutdown, FfiProviderRegistration, RoukakuPluginModule,
    RoukakuPluginModuleRef,
};

extern "C" fn descriptor_yaml() -> RString {
    r#"
id: roukaku.fixture.vn_extension_provider
version: 0.1.0
engine_version: 0.1.0
rustc_fingerprint: rustc-stable
feature_fingerprint: stage3-vn
abi_fingerprint: roukaku-plugin-abi-v3
abi_style: abi_stable_rust
capabilities:
  - roukaku.vn.policy_bundle
  - roukaku.vn.command
  - roukaku.vn.presentation_command
  - roukaku.vn.editor_metadata
  - roukaku.vn.release_check
permissions:
  - runtime.vn
packaged: true
"#
    .into()
}

extern "C" fn register() -> FfiPluginRegistration {
    tracing::info!(
        event = "fixture.vn_extension.register",
        provider_count = 5,
        "VN extension fixture registered"
    );
    FfiPluginRegistration {
        providers: RVec::from(vec![
            provider(
                "roukaku.vn.policy_bundle_provider",
                "roukaku.fixture.vn.policy_bundle",
                "roukaku.vn.policy_bundle",
            ),
            provider(
                "roukaku.vn.command_provider",
                "roukaku.fixture.vn.command",
                "roukaku.vn.command",
            ),
            provider(
                "roukaku.vn.presentation_command_provider",
                "roukaku.fixture.vn.presentation_command",
                "roukaku.vn.presentation_command",
            ),
            provider(
                "roukaku.vn.editor_metadata_provider",
                "roukaku.fixture.vn.editor_metadata",
                "roukaku.vn.editor_metadata",
            ),
            provider(
                "roukaku.vn.release_check_provider",
                "roukaku.fixture.vn.release_check",
                "roukaku.vn.release_check",
            ),
        ]),
        callbacks: 0,
    }
}

extern "C" fn shutdown() -> FfiPluginShutdown {
    tracing::info!(
        event = "fixture.vn_extension.shutdown",
        "VN extension fixture shut down"
    );
    FfiPluginShutdown {
        callbacks_released: true,
    }
}

fn provider(slot: &str, provider_id: &str, capability: &str) -> FfiProviderRegistration {
    FfiProviderRegistration {
        slot: slot.into(),
        provider_id: provider_id.into(),
        capability: capability.into(),
        phase: "runtime".into(),
        packaged: true,
    }
}

#[abi_stable::export_root_module]
pub fn roukaku_plugin_root_module() -> RoukakuPluginModuleRef {
    RoukakuPluginModule {
        descriptor_yaml,
        register,
        shutdown,
    }
    .leak_into_prefix()
}
