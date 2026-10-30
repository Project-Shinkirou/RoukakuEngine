use std::collections::BTreeMap;

use roukaku_core::{Hash256, SourceRef};
use roukaku_package::{ContainerError, PackageReader, SectionPayload};
use roukaku_ui_core::{UiBindingManifest, UiBlueprintBundle};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    CompiledVnUi, LuauStoryBundle, VN_PRESENTATION_PROVIDER_MANIFEST_SCHEMA, VnExtensionManifest,
    VnPolicyBundleManifest, VnPolicyBundleSourceCache, VnPresentationProviderManifest,
    VnProfileManifest, VnStandardCommandManifest, VnSystemUiProfileManifest,
    load_luau_story_bundle,
};

pub const VN_PROJECT_SCHEMA: &str = "roukaku.vn.project.v6";
pub const VN_PROJECT_ROOT_SCHEMA: &str = "roukaku.vn.project_root.v5";

/// A packaged NativeVN product: authored Luau story modules plus compiled UI.
#[derive(Debug, Clone, PartialEq)]
pub struct VnProject {
    pub schema: String,
    pub project_hash: Hash256,
    pub story: LuauStoryBundle,
    pub ui: CompiledVnUi,
}

impl VnProject {
    pub fn new(story: LuauStoryBundle, ui: CompiledVnUi) -> Result<Self, ContainerError> {
        story.validate()?;
        let project_hash = project_hash(&story, &ui)?;
        Ok(Self {
            schema: VN_PROJECT_SCHEMA.to_string(),
            project_hash,
            story,
            ui,
        })
    }
}

fn project_hash(story: &LuauStoryBundle, ui: &CompiledVnUi) -> Result<Hash256, ContainerError> {
    let bytes = postcard::to_allocvec(&(
        story.source_revision,
        ui.ui_blueprints.hash,
        ui.ui_bindings.hash,
        &ui.themes,
        &ui.controller_sources,
        &ui.system_ui_profiles,
    ))
    .map_err(|error| ContainerError::message(format!("ROUKAKU_VN_PROJECT_ENCODE: {error}")))?;
    Ok(Hash256::from_sha256(&bytes))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VnProjectRoot {
    pub schema: String,
    pub project_schema: String,
    pub project_hash: Hash256,
    pub story_revision: Hash256,
    pub ui_blueprint_hash: Hash256,
    pub ui_binding_hash: Hash256,
    pub ui_provider: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VnUiControllerBundle {
    pub schema: String,
    pub sources: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VnUiBackendManifest {
    pub schema: String,
    pub provider_id: String,
    pub input_protocol: String,
    pub render_protocol: String,
}

pub fn package_sections_for_project(
    project: &VnProject,
    profiles: &[String],
    target: &str,
) -> Result<Vec<SectionPayload>, ContainerError> {
    tracing::info!(
        event = "vn.package.build.start",
        profile_count = profiles.len(),
        story_module_count = project.story.modules.len(),
        target,
        "RoukakuVN package section build started"
    );
    let manifest = VnProfileManifest {
        schema: "roukaku.vn.profile_manifest.v1".to_string(),
        target: target.to_string(),
        profiles: profiles.to_vec(),
    };
    let root = VnProjectRoot {
        schema: VN_PROJECT_ROOT_SCHEMA.to_string(),
        project_schema: project.schema.clone(),
        project_hash: project.project_hash,
        story_revision: project.story.source_revision,
        ui_blueprint_hash: project.ui.ui_blueprints.hash,
        ui_binding_hash: project.ui.ui_bindings.hash,
        ui_provider: "roukaku.ui.yakui".to_string(),
    };
    let sections = vec![
        SectionPayload::postcard("vn.project", VN_PROJECT_ROOT_SCHEMA, &root)?,
        project.story.section()?,
        SectionPayload::postcard(
            "vn.ui_blueprint_bundle",
            "roukaku.ui_blueprint_bundle.v2",
            &project.ui.ui_blueprints,
        )?,
        SectionPayload::postcard(
            "vn.ui_binding_manifest",
            "roukaku.ui_binding_manifest.v1",
            &project.ui.ui_bindings,
        )?,
        SectionPayload::postcard(
            "vn.ui_source_map",
            "roukaku.vn.ui_source_map.v1",
            &project.ui.ui_source_map,
        )?,
        SectionPayload::postcard(
            "vn.ui_controller_manifest",
            "roukaku.vn.ui_controller_manifest.v1",
            &VnUiControllerBundle {
                schema: "roukaku.vn.ui_controller_manifest.v1".to_string(),
                sources: project.ui.controller_sources.clone(),
            },
        )?,
        SectionPayload::postcard(
            "vn.ui_theme_manifest",
            "roukaku.ui_theme_bundle.v1",
            &project.ui.themes,
        )?,
        SectionPayload::postcard(
            "vn.ui_backend_manifest",
            "roukaku.vn.ui_backend_manifest.v1",
            &VnUiBackendManifest {
                schema: "roukaku.vn.ui_backend_manifest.v1".to_string(),
                provider_id: "roukaku.ui.yakui".to_string(),
                input_protocol: "roukaku.ui_input_frame.v1".to_string(),
                render_protocol: "roukaku.ui_render_frame.v1".to_string(),
            },
        )?,
        SectionPayload::postcard(
            "vn.profile_manifest",
            "roukaku.vn.profile_manifest.v1",
            &manifest,
        )?,
        SectionPayload::postcard(
            "vn.policy_bundle_manifest",
            "roukaku.policy_bundle.v1",
            &VnPolicyBundleManifest::standard(),
        )?,
        SectionPayload::postcard(
            "vn.policy_bundle_source_cache",
            "roukaku.vn.policy_bundle_source_cache.v1",
            &VnPolicyBundleSourceCache::standard(),
        )?,
        SectionPayload::postcard(
            "vn.extension_manifest",
            "roukaku.vn.extension_manifest.v1",
            &VnExtensionManifest::standard(),
        )?,
        SectionPayload::postcard(
            "vn.standard_command_manifest",
            "roukaku.vn.standard_command_manifest.v2",
            &VnStandardCommandManifest::standard(),
        )?,
        SectionPayload::postcard(
            "vn.presentation_provider_manifest",
            VN_PRESENTATION_PROVIDER_MANIFEST_SCHEMA,
            &VnPresentationProviderManifest::standard(),
        )?,
        SectionPayload::postcard(
            "vn.system_ui_profile_manifest",
            "roukaku.vn.system_ui_profile_manifest.v4",
            &VnSystemUiProfileManifest::from_ui(&project.ui, vec!["zh-Hans".to_string()]),
        )?,
    ];
    tracing::info!(
        event = "vn.package.build.complete",
        section_count = sections.len(),
        target,
        "RoukakuVN package section build completed"
    );
    Ok(sections)
}

pub fn load_presentation_provider_manifest(
    package: &PackageReader,
    profile: &str,
) -> Result<VnPresentationProviderManifest, ContainerError> {
    let entry = package
        .container()
        .section_entry("vn.presentation_provider_manifest")
        .ok_or_else(|| {
            ContainerError::message(
                "ROUKAKU_VN_PRESENTATION_PROVIDER_MANIFEST: package section is missing",
            )
        })?;
    if entry.schema != VN_PRESENTATION_PROVIDER_MANIFEST_SCHEMA {
        return Err(ContainerError::message(format!(
            "ROUKAKU_VN_PRESENTATION_PROVIDER_SCHEMA: unsupported schema {}; package must be re-cooked",
            entry.schema
        )));
    }
    let manifest: VnPresentationProviderManifest = package
        .container()
        .decode_postcard("vn.presentation_provider_manifest")
        .map_err(|error| {
            ContainerError::message(format!(
                "ROUKAKU_VN_PRESENTATION_PROVIDER_MANIFEST: {error}"
            ))
        })?;
    let report = manifest.validate_standard();
    if let Some(diagnostic) = report.diagnostics.first() {
        return Err(ContainerError::message(format!(
            "{}: {}",
            diagnostic.code, diagnostic.message
        )));
    }
    manifest.profile(profile).map_err(|diagnostic| {
        ContainerError::message(format!("{}: {}", diagnostic.code, diagnostic.message))
    })?;
    Ok(manifest)
}

pub fn decode_vn_project(package: &PackageReader) -> Result<VnProject, ContainerError> {
    let root: VnProjectRoot = package.container().decode_postcard("vn.project")?;
    if root.schema != VN_PROJECT_ROOT_SCHEMA
        || root.project_schema != VN_PROJECT_SCHEMA
        || root.ui_provider != "roukaku.ui.yakui"
    {
        return Err(ContainerError::message(
            "ROUKAKU_VN_PROJECT_ROOT: invalid project root or UI provider",
        ));
    }
    let story = load_luau_story_bundle(package)?;
    let ui_blueprints: UiBlueprintBundle = package
        .container()
        .decode_postcard("vn.ui_blueprint_bundle")?;
    let ui_bindings: UiBindingManifest = package
        .container()
        .decode_postcard("vn.ui_binding_manifest")?;
    let ui_source_map: BTreeMap<String, SourceRef> =
        package.container().decode_postcard("vn.ui_source_map")?;
    let controllers: VnUiControllerBundle = package
        .container()
        .decode_postcard("vn.ui_controller_manifest")?;
    let themes: BTreeMap<String, roukaku_ui_core::UiThemeManifest> = package
        .container()
        .decode_postcard("vn.ui_theme_manifest")?;
    let system_ui: VnSystemUiProfileManifest = package
        .container()
        .decode_postcard("vn.system_ui_profile_manifest")?;
    if system_ui.schema != "roukaku.vn.system_ui_profile_manifest.v4" {
        return Err(ContainerError::message(
            "ROUKAKU_VN_SYSTEM_UI_PROFILE_SCHEMA: packaged system UI policy is not v4",
        ));
    }
    if story.source_revision != root.story_revision
        || ui_blueprints.hash != root.ui_blueprint_hash
        || ui_bindings.hash != root.ui_binding_hash
    {
        return Err(ContainerError::message(
            "ROUKAKU_VN_PROJECT_HASH: project section hashes do not match the root",
        ));
    }
    let project = VnProject::new(
        story,
        CompiledVnUi {
            ui_blueprints,
            ui_bindings,
            ui_source_map,
            controller_ids: controllers.sources.keys().cloned().collect(),
            controller_sources: controllers.sources,
            theme_ids: themes.keys().cloned().collect(),
            themes,
            system_ui_profiles: system_ui.profiles,
        },
    )?;
    if project.project_hash != root.project_hash {
        return Err(ContainerError::message(
            "ROUKAKU_VN_PROJECT_HASH: project root hash does not cover the packaged sections",
        ));
    }
    Ok(project)
}
