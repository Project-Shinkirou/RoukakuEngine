use roukaku_example_progress_meter::{COMMAND, PROVIDER, SCHEMA};
use roukaku_player_core::{PlayerHostCommand, PlayerHostResourceId};
use roukaku_player_vn::{
    NativeVnHostCommandSource, NativeVnHostOpenOptions, NativeVnRuntimeExecution,
    PreparedPolicyActions,
};
use roukaku_plugin::{ExtensionRestoration, WorkExtensionRegistry};
use roukaku_vn_script::{ExtensionCommandDescriptor, ExtensionFieldContract, ExtensionFieldKind};

fn package() -> Vec<u8> {
    roukaku_player_vn::headless_test_fixture::product_package_with_extensions(
        "story main #@id story.main\nstate start #@id state.start\n  scene room #@id scene.room\n    progress_meter completed:3 total:4 #@id meter\n    text key:line #@id line\n",
        &[ExtensionCommandDescriptor {
            command: COMMAND.into(),
            provider_id: PROVIDER.into(),
            schema: SCHEMA.into(),
            fields: ["completed", "total"]
                .into_iter()
                .map(|name| {
                    (
                        name.into(),
                        ExtensionFieldContract {
                            kind: ExtensionFieldKind::Integer,
                            required: true,
                        },
                    )
                })
                .collect(),
        }],
    )
}

fn open(
    bytes: &[u8],
    registry: &WorkExtensionRegistry,
) -> Result<NativeVnHostCommandSource, roukaku_player_vn::NativeVnHostError> {
    let package = roukaku_package::PackageReader::open(bytes).unwrap();
    NativeVnHostCommandSource::from_package_with_extensions(
        &package,
        roukaku_vn_core::VnRunConfig::classic("en"),
        320,
        180,
        PlayerHostResourceId(1),
        NativeVnHostOpenOptions {
            max_asset_cache_bytes: 64 * 1024 * 1024,
            max_glyph_cache_bytes: 1024 * 1024,
            runtime_execution: NativeVnRuntimeExecution::shipping_serial(),
            policy_actions: PreparedPolicyActions::load(
                &package,
                64 * 1024 * 1024,
                &roukaku_runtime::TaskScope::new(),
            )
            .unwrap(),
        },
        registry,
    )
}

#[test]
fn linked_project_player_executes_story_command_into_presentation() {
    let bytes = package();
    let registry = roukaku_example_extension_player::extensions().unwrap();
    let mut player = open(&bytes, &registry).unwrap();
    let batch = player.launch().unwrap();
    let expected = roukaku_media_core::SceneCommand::rect(
        "progress_meter.fill",
        16,
        16,
        180,
        12,
        [32, 192, 128, 255],
    );
    assert!(batch.commands.iter().any(|command| matches!(command, PlayerHostCommand::PresentScene { commands, .. } if commands.contains(&expected))));
    player.release_resources().unwrap();
    player.shutdown().unwrap();
}

#[test]
fn prebuilt_player_rejects_unlinked_project_provider_before_launch() {
    let error = match open(&package(), &WorkExtensionRegistry::default()) {
        Ok(_) => panic!("unlinked provider was accepted"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("ROUKAKU_EXTENSION_PROVIDER_UNREGISTERED")
    );
}

#[test]
fn extension_business_state_restores_and_rejects_invalid_candidate() {
    let mut registry = roukaku_example_extension_player::extensions().unwrap();
    assert!(
        registry
            .register::<roukaku_example_progress_meter::ProgressMeter>()
            .is_err()
    );
    let mut runtime = registry.instantiate();
    let mut command = roukaku_vn_script::ExtensionPresentationCommand {
        command: COMMAND.into(),
        provider_id: PROVIDER.into(),
        schema: SCHEMA.into(),
        fields: [
            (
                "completed".into(),
                roukaku_vn_script::ExtensionValue::Integer(3),
            ),
            (
                "total".into(),
                roukaku_vn_script::ExtensionValue::Integer(4),
            ),
        ]
        .into(),
    };
    roukaku_player_vn::dispatch_work_extension(&mut runtime, &command).unwrap();
    command.schema = "example.progress_meter.v2".into();
    assert!(roukaku_player_vn::dispatch_work_extension(&mut runtime, &command).is_err());
    command.schema = SCHEMA.into();
    command.fields.insert(
        "total".into(),
        roukaku_vn_script::ExtensionValue::Integer(0),
    );
    assert!(roukaku_player_vn::dispatch_work_extension(&mut runtime, &command).is_err());
    let ExtensionRestoration::Restorable(state) = runtime.restoration(PROVIDER).unwrap() else {
        panic!("meter must restore")
    };
    let snapshot = state.capture().unwrap();
    assert_eq!(
        snapshot.value,
        serde_json::json!({"completed": 3, "total": 4})
    );
    let mut invalid = snapshot.clone();
    invalid.value["total"] = serde_json::json!(0);
    assert!(state.restore(&invalid).is_err());
    assert_eq!(state.capture().unwrap(), snapshot);
    let mut restored = registry.instantiate();
    let ExtensionRestoration::Restorable(state) = restored.restoration(PROVIDER).unwrap() else {
        panic!("meter must restore")
    };
    state.restore(&snapshot).unwrap();
    assert_eq!(state.capture().unwrap(), snapshot);
}
