use super::*;
use roukaku_vn_policy::{
    LuauPolicy, POLICY_IO_MAX_BYTES, PolicyActionBinding, PolicyExecutionBudget,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionEntry {
    entry: String,
    #[serde(default)]
    read_text: Vec<VfsUri>,
}

pub(super) fn load(
    project: &serde_yaml::Value,
    root: &Path,
    cancellation: &CookCancellationToken,
) -> Result<Vec<PolicyActionBinding>, CliError> {
    let Some(entries) = project
        .get("nativevn")
        .and_then(|vn| vn.get("policy_actions"))
    else {
        return Ok(Vec::new());
    };
    let entries: Vec<ActionEntry> = serde_yaml::from_value(entries.clone())?;
    if entries.len() > 64 {
        return Err("ROUKAKU_VN_POLICY_ACTION_LIMIT".into());
    }
    let root = root.canonicalize()?;
    let mut bindings = Vec::new();
    let mut commands = std::collections::BTreeSet::new();
    let mut paths = std::collections::BTreeSet::new();
    for entry in entries {
        cancellation.check_cancelled()?;
        let relative = validate_project_relative_path(&entry.entry)?;
        if !paths.insert(normalize_relative_path(&relative)) {
            return Err("ROUKAKU_VN_POLICY_ACTION_DUPLICATE_ENTRY".into());
        }
        let path = root.join(&relative).canonicalize()?;
        if !path.starts_with(&root) || path.metadata()?.len() > POLICY_IO_MAX_BYTES as u64 {
            return Err("ROUKAKU_VN_POLICY_ACTION_ENTRY_INVALID".into());
        }
        let source = fs::read_to_string(path)?;
        cancellation.check_cancelled()?;
        let descriptors =
            LuauPolicy::action_descriptors(&source, PolicyExecutionBudget::default())?;
        for descriptor in descriptors {
            if !commands.insert(descriptor.command.clone()) {
                return Err("ROUKAKU_VN_POLICY_ACTION_DUPLICATE_COMMAND".into());
            }
            let binding = PolicyActionBinding {
                descriptor,
                entry: normalize_relative_path(&relative),
                source_hash: Hash256::from_sha256(source.as_bytes()).to_string(),
                source: source.clone(),
                read_text: entry.read_text.clone(),
            };
            binding.validate()?;
            bindings.push(binding);
        }
    }
    Ok(bindings)
}

/// Validate the exact asset IDs used by the catalog before committing Cook output.
pub(super) fn validate_resources(
    bindings: &[PolicyActionBinding],
    artifacts: &[CookedArtifactRef],
    out: &Path,
    cancellation: &CookCancellationToken,
) -> Result<(), CliError> {
    let resources: std::collections::BTreeSet<_> =
        bindings.iter().flat_map(|b| &b.read_text).collect();
    roukaku_vn_policy::PolicyIoCapabilities::read_text(
        resources.iter().map(|r| (*r).clone()),
        POLICY_IO_MAX_BYTES,
    )?;
    for resource in resources {
        cancellation.check_cancelled()?;
        let matches: Vec<_> = artifacts
            .iter()
            .filter(|a| a.asset_id.as_deref() == Some(resource.as_str()))
            .collect();
        let [artifact] = matches.as_slice() else {
            return Err("ROUKAKU_VN_POLICY_TEXT_ASSET_MAPPING".into());
        };
        if resource.prefix() != "asset" || artifact_media_kind(artifact) != "text" {
            return Err("ROUKAKU_VN_POLICY_TEXT_ASSET_TYPE".into());
        }
        let path = out.join(&artifact.path);
        if path.metadata()?.len() > POLICY_IO_MAX_BYTES as u64 {
            return Err("ROUKAKU_VN_POLICY_TEXT_LIMIT".into());
        }
        let text = fs::read(path)?;
        cancellation.check_cancelled()?;
        std::str::from_utf8(&text).map_err(|_| "ROUKAKU_VN_POLICY_TEXT_UTF8")?;
        artifact_vfs_uri(artifact)?;
    }
    Ok(())
}

pub(super) fn validate_commands(
    project: &roukaku_vn_script::CompiledVnProject,
    bindings: &[PolicyActionBinding],
) -> Result<(), CliError> {
    use roukaku_vn_script::{CompiledCommand, POLICY_ACTION_PROVIDER, PresentationCommand};
    for command in project
        .story
        .states
        .values()
        .flat_map(|state| &state.scenes)
        .flat_map(|scene| &scene.commands)
    {
        if let CompiledCommand::Presentation {
            command: PresentationCommand::Extension(extension),
            ..
        } = command
            && extension.provider_id == POLICY_ACTION_PROVIDER
        {
            bindings
                .iter()
                .find(|b| b.descriptor.command == extension.command)
                .ok_or("ROUKAKU_VN_POLICY_ACTION_BINDING_MISSING")?
                .validate_command(extension)?;
        }
    }
    Ok(())
}
