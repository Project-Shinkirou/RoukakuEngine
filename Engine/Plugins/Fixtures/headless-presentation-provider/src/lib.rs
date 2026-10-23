use abi_stable::{
    prefix_type::PrefixTypeTrait,
    std_types::{RString, RVec},
};
use roukaku_plugin_abi::{
    RoukakuPluginModule, RoukakuPluginModuleRef, FfiPluginRegistration, FfiPluginShutdown,
    FfiProviderRegistration,
};
use tracing::debug;

extern "C" fn descriptor_yaml() -> RString {
    r#"
id: roukaku.fixture.headless_presentation
version: 0.1.0
engine_version: 0.1.0
rustc_fingerprint: rustc-stable
feature_fingerprint: runtime-typed-v3
abi_fingerprint: roukaku-plugin-abi-v3
abi_style: abi_stable_rust
capabilities:
  - presentation.headless
permissions:
  - runtime.presentation
packaged: true
"#
    .into()
}

extern "C" fn register() -> FfiPluginRegistration {
    debug!("fixture.register");
    FfiPluginRegistration {
        providers: RVec::from(vec![FfiProviderRegistration {
            slot: "presentation".into(),
            provider_id: "roukaku.fixture.headless_presentation".into(),
            capability: "presentation.headless".into(),
            phase: "runtime".into(),
            packaged: true,
        }]),
        callbacks: 0,
    }
}

extern "C" fn shutdown() -> FfiPluginShutdown {
    debug!("fixture.shutdown");
    FfiPluginShutdown {
        callbacks_released: true,
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
