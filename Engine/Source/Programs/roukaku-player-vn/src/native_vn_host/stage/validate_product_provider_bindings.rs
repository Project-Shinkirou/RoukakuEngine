use super::*;

pub(in super::super) fn validate_product_provider_bindings(
    package: &roukaku_package::PackageReader,
) -> Result<(), NativeVnHostError> {
    let policy = read_package_json(package, "provider.policy")?;
    let registry = read_package_json(package, "plugin.extension_registry")?;
    let headless_selected = policy.get("renderer").and_then(serde_json::Value::as_str)
        == Some("headless")
        || policy
            .get("bindings")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .any(|binding| {
                binding
                    .get("provider_id")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|provider| provider.contains("headless"))
            });
    if headless_selected {
        return Err(NativeVnHostError::ProviderBinding(
            "ROUKAKU_PLAYER_PRESENTATION_PROVIDER_INELIGIBLE: headless presentation is not packaged Player eligible"
                .to_string(),
        ));
    }
    {
        let (slot, provider_id, capability) =
            ("presentation", "roukaku.renderer.wgpu", "renderer2d.wgpu");
        let policy_bound = policy
            .get("bindings")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|bindings| {
                bindings.iter().any(|binding| {
                    binding.get("slot").and_then(serde_json::Value::as_str) == Some(slot)
                        && binding
                            .get("provider_id")
                            .and_then(serde_json::Value::as_str)
                            == Some(provider_id)
                })
            });
        let registry_bound = registry
            .get("bindings")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|bindings| {
                bindings.iter().any(|binding| {
                    binding.get("slot").and_then(serde_json::Value::as_str) == Some(slot)
                        && binding
                            .get("provider_id")
                            .and_then(serde_json::Value::as_str)
                            == Some(provider_id)
                })
            });
        let registered = registry
            .get("providers")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|providers| {
                providers.iter().any(|provider| {
                    provider.get("slot").and_then(serde_json::Value::as_str) == Some(slot)
                        && provider
                            .get("provider_id")
                            .and_then(serde_json::Value::as_str)
                            == Some(provider_id)
                        && provider
                            .get("capability")
                            .and_then(serde_json::Value::as_str)
                            == Some(capability)
                        && provider
                            .get("packaged")
                            .and_then(serde_json::Value::as_bool)
                            == Some(true)
                })
            });
        if !policy_bound || !registry_bound || !registered {
            return Err(NativeVnHostError::ProviderBinding(format!(
                "ROUKAKU_PLAYER_PROVIDER_BINDING_INVALID: {slot} must bind packaged provider {provider_id}"
            )));
        }
    }
    let presentation: roukaku_vn_presentation::VnPresentationProviderManifest = package
        .container()
        .decode_postcard("vn.presentation_provider_manifest")
        .map_err(|error| NativeVnHostError::ProviderBinding(error.to_string()))?;
    if !presentation.validate_standard().passed
        || presentation.renderer_provider != "roukaku.renderer2d.wgpu"
    {
        return Err(NativeVnHostError::ProviderBinding(
            "ROUKAKU_PLAYER_PRESENTATION_PROVIDER_INELIGIBLE: package presentation provider is not the product wgpu path"
                .to_string(),
        ));
    }
    Ok(())
}

pub(in super::super) fn unique_string_set(
    values: Vec<String>,
    code: &'static str,
) -> Result<BTreeSet<String>, NativeVnHostError> {
    let count = values.len();
    let values = values.into_iter().collect::<BTreeSet<_>>();
    if values.len() != count {
        return Err(typed_vn_state_error(code));
    }
    Ok(values)
}

pub(in super::super) fn typed_vn_state_error(code: &'static str) -> NativeVnHostError {
    NativeVnHostError::RuntimeEvidence(format!(
        "{code}: typed VN state contains duplicate identity"
    ))
}

pub(in super::super) fn player_timeline_task(
    task: roukaku_vn_core::VnTimelineTask,
) -> Result<PlayerTimelineTask, NativeVnHostError> {
    let (task_id, target, action, duration_ms, fence) = match task.command {
        TimelineCommand::Start(spec) => {
            let duration_ms = spec
                .tracks
                .iter()
                .flat_map(|track| track.keyframes.iter())
                .map(|keyframe| u64::from(keyframe.time_ms))
                .max();
            let target = spec.tracks.first().map(|track| track.target.clone());
            (
                spec.id,
                target,
                PlayerTimelineTaskAction::Start,
                duration_ms,
                spec.fence,
            )
        }
        TimelineCommand::Cancel { id, .. } => {
            (id, None, PlayerTimelineTaskAction::Cancel, None, None)
        }
    };
    Ok(PlayerTimelineTask {
        schema: "roukaku.player_timeline_task.v1".to_string(),
        task_id,
        target,
        action,
        duration_ms,
        fence,
    })
}

pub(in super::super) fn read_package_json(
    package: &roukaku_package::PackageReader,
    section: &str,
) -> Result<serde_json::Value, NativeVnHostError> {
    let bytes = package
        .container()
        .read_bounded(section, 256 * 1024)
        .map_err(|error| NativeVnHostError::ProviderBinding(error.to_string()))?;
    serde_json::from_slice(&bytes).map_err(|error| {
        NativeVnHostError::ProviderBinding(format!(
            "ROUKAKU_PLAYER_PROVIDER_SECTION_INVALID: {section}: {error}"
        ))
    })
}

pub(in super::super) fn cleanup_runtime_host(
    host: &mut NativeVnRuntimeHost,
    error: impl std::fmt::Display,
) -> NativeVnHostError {
    match host.cleanup_after_failure() {
        Ok(_) => NativeVnHostError::Package(error.to_string()),
        Err(cleanup_error) => NativeVnHostError::Package(format!(
            "ROUKAKU_PLAYER_RUNTIME_CLEANUP_FAILED: {error}; cleanup failed: {cleanup_error}"
        )),
    }
}

pub(in super::super) fn current_image_working_set(state: &ProductStageState) -> Vec<String> {
    let mut working_set = state
        .entities
        .values()
        .filter(|entity| entity.visible)
        .map(|entity| entity.asset.clone())
        .collect::<BTreeSet<_>>();
    working_set.extend(
        state
            .movies
            .values()
            .filter_map(|movie| movie.fallback.clone()),
    );
    working_set.into_iter().collect()
}
