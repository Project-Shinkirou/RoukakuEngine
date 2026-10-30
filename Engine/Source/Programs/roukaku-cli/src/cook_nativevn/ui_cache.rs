use std::{collections::BTreeMap, path::Path};

use roukaku_cook::{CookArtifact, FileCookCache, cook_cache_key};
use roukaku_core::Hash256;
use roukaku_vn_script::{
    CompileRoukakuProjectOptions, CompiledVnProject, RoukakuSource, compile_roukaku_project,
};

use crate::{
    CliError, collect_asset_sidecars, nativevn_asset_roots, normalize_relative_path,
    validate_project_relative_path,
};

const PROCESSOR: &str = "roukaku.cook.ui-jsonc";
const VERSION: &str = "1";

/// Cache the compiled project with the existing Cook artifact store. The source identity
/// includes paths as well as bytes because implicit widget identities contain source paths.
pub(super) fn compile(
    project: &serde_yaml::Value,
    root: &Path,
    target_profile: &str,
    sources: Vec<RoukakuSource>,
    options: CompileRoukakuProjectOptions,
    mut inputs: BTreeMap<String, Hash256>,
) -> Result<CompiledVnProject, CliError> {
    for source in &sources {
        inputs.insert(
            source.path.clone(),
            Hash256::from_sha256(source.text.as_bytes()),
        );
    }
    let source_hash = Hash256::from_sha256(&serde_json::to_vec(&inputs)?);
    let mut sidecars = BTreeMap::new();
    for source in inputs.keys() {
        let path = root.join(format!("{source}.roukaku-asset.yaml"));
        match std::fs::read(&path) {
            Ok(bytes) => {
                sidecars.insert(
                    normalize_relative_path(path.strip_prefix(root)?),
                    Hash256::from_sha256(&bytes),
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    // Asset metadata and cook settings remain part of the project dependency identity.
    let mut paths = Vec::new();
    for directory in nativevn_asset_roots(project) {
        let directory = root.join(validate_project_relative_path(&directory)?);
        if directory.is_dir() {
            collect_asset_sidecars(root, &directory, &mut paths)?;
        }
    }
    for path in paths {
        sidecars.insert(
            normalize_relative_path(&path),
            Hash256::from_sha256(&std::fs::read(root.join(path))?),
        );
    }
    let sidecar_hash = Hash256::from_sha256(&serde_json::to_vec(&(project, sidecars))?);
    let dependencies = BTreeMap::from([
        (
            "ui-screen.v1.json".to_string(),
            Hash256::from_sha256(roukaku_vn_script::UI_SCREEN_SCHEMA.as_bytes()),
        ),
        (
            "ui-style.v1.json".to_string(),
            Hash256::from_sha256(roukaku_vn_script::UI_STYLE_SCHEMA.as_bytes()),
        ),
    ]);
    let key = cook_cache_key(
        &source_hash,
        &sidecar_hash,
        PROCESSOR,
        VERSION,
        target_profile,
        &dependencies,
    );
    let cache = FileCookCache::new(root.join(".roukaku-cache/cook"));
    if let Some(artifact) = cache.load(&key)? {
        if artifact.source_hash != source_hash
            || artifact.sidecar_hash != sidecar_hash
            || artifact.processor_id != PROCESSOR
            || artifact.processor_version != VERSION
            || artifact.target_profile != target_profile
            || artifact.asset_id != "asset:/ui/project"
            || artifact.section_id != "vn.compiled_project"
        {
            return Err("ROUKAKU_COOK_CACHE_CORRUPT: JSONC UI artifact identity mismatch".into());
        }
        return Ok(serde_json::from_slice(&artifact.payload)?);
    }
    let compiled = compile_roukaku_project(sources, options)?;
    let payload = serde_json::to_vec(&compiled)?;
    cache.store(&CookArtifact {
        schema: "roukaku.cook_artifact.v1".to_string(),
        asset_id: "asset:/ui/project".to_string(),
        section_id: "vn.compiled_project".to_string(),
        target_profile: target_profile.to_string(),
        processor_id: PROCESSOR.to_string(),
        processor_version: VERSION.to_string(),
        source_hash,
        sidecar_hash,
        cache_key: key,
        payload_hash: Hash256::from_sha256(&payload),
        payload,
    })?;
    Ok(compiled)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sources(text: &str) -> Vec<RoukakuSource> {
        vec![
            RoukakuSource::story(
                "story.roukaku",
                "story main #@id story.main\nstate start #@id state.start\n  scene room #@id scene.room\n    text key:hello #@id line.hello\n",
            ),
            RoukakuSource::ui("test.screen.jsonc", text),
        ]
    }

    fn count_cache(root: &Path) -> usize {
        std::fs::read_dir(root.join(".roukaku-cache/cook"))
            .unwrap()
            .count()
    }

    #[test]
    fn cached_blueprints_preserve_identity_and_invalidate_on_sources_sidecars_and_target() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        let project = serde_yaml::from_str("nativevn: {asset_roots: []}").unwrap();
        let screen = r#"{"$schema":"ui-screen.v1.json"}"#;
        let cook = |target, text: &str| {
            compile(
                &project,
                root,
                target,
                sources(text),
                Default::default(),
                BTreeMap::new(),
            )
            .unwrap()
        };
        let first = cook("windows/classic", screen);
        let second = cook("windows/classic", screen);
        assert_eq!(first.ui_blueprints, second.ui_blueprints);
        assert_eq!(first.ui_source_map, second.ui_source_map);
        assert_eq!(count_cache(root), 1);
        cook(
            "windows/classic",
            &format!("// moved source span\n{screen}"),
        );
        assert_eq!(count_cache(root), 2);
        std::fs::write(
            root.join("test.screen.jsonc.roukaku-asset.yaml"),
            "license: MIT\n",
        )
        .unwrap();
        cook("windows/classic", screen);
        assert_eq!(count_cache(root), 3);
        std::fs::write(
            root.join("test.screen.jsonc.roukaku-asset.yaml"),
            "license: Apache-2.0\n",
        )
        .unwrap();
        cook("windows/classic", screen);
        assert_eq!(count_cache(root), 4);
        cook("windows/modern", screen);
        assert_eq!(count_cache(root), 5);
        assert!(
            compile(
                &project,
                root,
                "windows/classic",
                sources("{\"$schema\":\"ui-screen.v1.json\",\"views\":[{}]}"),
                Default::default(),
                BTreeMap::new()
            )
            .is_err()
        );
        assert_eq!(count_cache(root), 5);
    }
}
