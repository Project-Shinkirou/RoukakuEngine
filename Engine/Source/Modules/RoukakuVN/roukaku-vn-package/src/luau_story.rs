use std::collections::BTreeMap;

use roukaku_core::Hash256;
use roukaku_package::{ContainerError, PackageReader, SectionPayload};
use roukaku_vn_policy::authoring::ParagraphId;
use serde::{Deserialize, Serialize};

pub const LUAU_STORY_BUNDLE_SCHEMA: &str = "roukaku.vn.luau_story_bundle.v1";
const MAX_MODULE_BYTES: usize = 4 * 1024 * 1024;
const MAX_BUNDLE_BYTES: usize = 32 * 1024 * 1024;

/// Authored Luau modules are the product story source. The digest covers entry
/// identity and every named source module, and fences paragraph saves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LuauStoryBundle {
    pub schema: String,
    pub entry: ParagraphId,
    pub source_revision: Hash256,
    pub modules: BTreeMap<String, String>,
}

impl LuauStoryBundle {
    pub fn new(
        entry: ParagraphId,
        modules: BTreeMap<String, String>,
    ) -> Result<Self, ContainerError> {
        let mut bundle = Self {
            schema: LUAU_STORY_BUNDLE_SCHEMA.into(),
            entry,
            source_revision: Hash256::from_sha256(&[]),
            modules,
        };
        bundle.source_revision = bundle.compute_revision()?;
        bundle.validate()?;
        Ok(bundle)
    }

    pub fn validate(&self) -> Result<(), ContainerError> {
        if self.schema != LUAU_STORY_BUNDLE_SCHEMA
            || self.entry.module.is_empty()
            || self.entry.entry.is_empty()
            || self.modules.is_empty()
            || self.modules.len() > 256
        {
            return Err(ContainerError::message("ROUKAKU_VN_LUAU_STORY_INVALID"));
        }
        let mut total = 0usize;
        for (module, source) in &self.modules {
            if module.is_empty() || source.is_empty() || source.len() > MAX_MODULE_BYTES {
                return Err(ContainerError::message("ROUKAKU_VN_LUAU_MODULE_INVALID"));
            }
            total = total
                .saturating_add(module.len())
                .saturating_add(source.len());
        }
        if total > MAX_BUNDLE_BYTES || !self.modules.contains_key(&self.entry.module) {
            return Err(ContainerError::message("ROUKAKU_VN_LUAU_STORY_LIMIT"));
        }
        if self.compute_revision()? != self.source_revision {
            return Err(ContainerError::message("ROUKAKU_VN_LUAU_STORY_REVISION"));
        }
        Ok(())
    }

    fn compute_revision(&self) -> Result<Hash256, ContainerError> {
        let bytes = postcard::to_allocvec(&(&self.entry, &self.modules))
            .map_err(|error| ContainerError::message(error.to_string()))?;
        Ok(Hash256::from_sha256(&bytes))
    }

    pub fn section(&self) -> Result<SectionPayload, ContainerError> {
        self.validate()?;
        SectionPayload::postcard("vn.luau_story_bundle", LUAU_STORY_BUNDLE_SCHEMA, self)
    }
}

pub fn load_luau_story_bundle(package: &PackageReader) -> Result<LuauStoryBundle, ContainerError> {
    if package.container().section_entry("vn.story").is_some() {
        return Err(ContainerError::message(
            "ROUKAKU_VN_STORY_SOURCE_MIXED: package contains the retired compiled story section",
        ));
    }
    let bundle: LuauStoryBundle = package
        .container()
        .decode_postcard("vn.luau_story_bundle")?;
    bundle.validate()?;
    Ok(bundle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_revision_covers_entry_and_source_modules() {
        let entry = ParagraphId {
            module: "story.main".into(),
            entry: "intro".into(),
        };
        let modules = BTreeMap::from([("story.main".into(), "return {}".into())]);
        let bundle = LuauStoryBundle::new(entry.clone(), modules.clone()).unwrap();
        bundle.validate().unwrap();
        let mut changed = bundle.clone();
        changed
            .modules
            .insert("story.main".into(), "return { changed = true }".into());
        assert!(
            changed
                .validate()
                .unwrap_err()
                .to_string()
                .contains("ROUKAKU_VN_LUAU_STORY_REVISION")
        );
        let mut changed_entry = LuauStoryBundle::new(
            ParagraphId {
                module: "story.main".into(),
                entry: "other".into(),
            },
            modules,
        )
        .unwrap();
        changed_entry.source_revision = bundle.source_revision;
        assert!(changed_entry.validate().is_err());
    }

    #[test]
    fn rejects_packages_that_mix_luau_and_compiled_story_sections() {
        let bundle = LuauStoryBundle::new(
            ParagraphId {
                module: "story.main".into(),
                entry: "intro".into(),
            },
            BTreeMap::from([("story.main".into(), "return {}".into())]),
        )
        .unwrap();
        let legacy = SectionPayload::postcard("vn.story", "roukaku.vn.story", &"legacy").unwrap();
        let bytes = roukaku_package::PackageBuilder::build(
            roukaku_package::PackageBuildRequest::fixture(
                "luau-story-mixed-source",
                "classic",
                vec![bundle.section().unwrap(), legacy],
            ),
        )
        .unwrap();
        let package = PackageReader::open(bytes.as_bytes()).unwrap();

        assert!(
            load_luau_story_bundle(&package)
                .unwrap_err()
                .to_string()
                .contains("ROUKAKU_VN_STORY_SOURCE_MIXED")
        );
    }
}

