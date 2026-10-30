use std::{collections::BTreeMap, path::Path, sync::Arc};

use roukaku_asset::{
    AssetCatalog, AssetCatalogEntry, LocalMountRootSet, ResolveContext, VfsManifest,
    VfsManifestEntry, VfsSourceRef, VfsUri,
};
use roukaku_core::Hash256;

use crate::{ContainerError, PackageReader};

/// Identity used by decoded caches. A content revision is the resolved VFS content hash.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResolvedAssetIdentity {
    pub asset_id: String,
    pub source_mount: String,
    pub content_revision: Hash256,
}

#[derive(Debug, Clone)]
pub struct ResolvedAssetBytes {
    pub identity: ResolvedAssetIdentity,
    pub uri: VfsUri,
    pub media_kind: String,
    pub codec: Option<String>,
    pub bytes: Arc<[u8]>,
}

/// Package, overlay-package and explicitly authorized local-directory resolver.
/// Mount order is determined solely by the VFS layer priorities in each package manifest.
#[derive(Debug, Clone)]
pub struct MountedAssetResolver {
    package: PackageReader,
    manifest: VfsManifest,
    catalog: AssetCatalog,
    mount_readers: BTreeMap<String, PackageReader>,
    mounted_layers: BTreeMap<String, Vec<String>>,
    local_roots: BTreeMap<String, LocalMountRootSet>,
    context: ResolveContext,
}

impl MountedAssetResolver {
    pub fn new(package: &PackageReader) -> Result<Self, ContainerError> {
        let manifest = read_manifest(package)?;
        let catalog = read_catalog(package)?;
        let context = ResolveContext {
            target: package.product_context().target().to_owned(),
            profile: package.product_context().profile().to_owned(),
            capability: String::new(),
            provider_binding: String::new(),
        };
        let mut result = Self {
            package: package.clone(),
            manifest,
            catalog,
            mount_readers: BTreeMap::new(),
            mounted_layers: BTreeMap::new(),
            local_roots: BTreeMap::new(),
            context,
        };
        result.index_package(package)?;
        if let Some(layer_id) = result
            .manifest
            .layers
            .iter()
            .filter(|layer| layer.prefix == "package")
            .min_by_key(|layer| layer.priority)
            .map(|layer| layer.layer_id.clone())
        {
            result.index_product_sections(package, &layer_id)?;
        }
        Ok(result)
    }

    pub fn catalog_entries(&self) -> &[AssetCatalogEntry] {
        &self.catalog.assets
    }

    /// Add an overlay package. It must declare compatible prefixes and unique layer IDs.
    pub fn mount_overlay(
        &mut self,
        mount_id: &str,
        overlay: &PackageReader,
    ) -> Result<(), ContainerError> {
        if mount_id.trim().is_empty() || self.mounted_layers.contains_key(mount_id) {
            return Err(ContainerError::message("ROUKAKU_VFS_MOUNT_ID_INVALID"));
        }
        let overlay_manifest = read_manifest(overlay)?;
        for prefix in overlay_manifest.prefixes {
            if let Some(existing) = self
                .manifest
                .prefixes
                .iter()
                .find(|item| item.prefix == prefix.prefix)
            {
                if existing.provider_id != prefix.provider_id
                    || existing.capabilities != prefix.capabilities
                {
                    return Err(ContainerError::message("ROUKAKU_VFS_MOUNT_PREFIX_MISMATCH"));
                }
            } else {
                self.manifest.prefixes.push(prefix);
            }
        }
        let mut layer_ids = Vec::with_capacity(overlay_manifest.layers.len());
        for layer in overlay_manifest.layers {
            if self
                .manifest
                .layers
                .iter()
                .any(|existing| existing.layer_id == layer.layer_id)
            {
                return Err(ContainerError::message(format!(
                    "ROUKAKU_VFS_MOUNT_LAYER_DUPLICATE: {}",
                    layer.layer_id
                )));
            }
            layer_ids.push(layer.layer_id.clone());
            self.mount_readers
                .insert(layer.layer_id.clone(), overlay.clone());
            self.manifest.layers.push(layer);
        }
        self.manifest.entries.extend(overlay_manifest.entries);
        self.manifest.whiteouts.extend(overlay_manifest.whiteouts);
        for asset in read_catalog(overlay)?.assets {
            if let Some(existing) = self
                .catalog
                .assets
                .iter()
                .find(|existing| existing.asset_id == asset.asset_id)
            {
                if existing.uri != asset.uri {
                    return Err(ContainerError::message(format!(
                        "ROUKAKU_VFS_ASSET_ID_URI_MISMATCH: {}",
                        asset.asset_id
                    )));
                }
            } else {
                self.catalog.assets.push(asset);
            }
        }
        if let Some(layer_id) = self
            .manifest
            .layers
            .iter()
            .filter(|layer| layer_ids.contains(&layer.layer_id) && layer.prefix == "package")
            .max_by_key(|layer| layer.priority)
            .map(|layer| layer.layer_id.clone())
        {
            self.index_product_sections(overlay, &layer_id)?;
        }
        self.mounted_layers.insert(mount_id.to_owned(), layer_ids);
        Ok(())
    }

    pub fn unmount_overlay(&mut self, mount_id: &str) -> Result<(), ContainerError> {
        let layers = self
            .mounted_layers
            .remove(mount_id)
            .ok_or_else(|| ContainerError::message("ROUKAKU_VFS_MOUNT_MISSING"))?;
        self.manifest
            .layers
            .retain(|layer| !layers.contains(&layer.layer_id));
        self.manifest
            .entries
            .retain(|entry| !layers.contains(&entry.layer_id));
        self.manifest
            .whiteouts
            .retain(|whiteout| !layers.contains(&whiteout.layer_id));
        for layer in layers {
            self.mount_readers.remove(&layer);
        }
        Ok(())
    }

    /// Host authorization is explicit. The alias must match LocalAuthorized source references.
    pub fn authorize_directory(
        &mut self,
        alias: &str,
        root: impl AsRef<Path>,
    ) -> Result<(), ContainerError> {
        let mut roots = LocalMountRootSet::default();
        roots.authorize(alias, root).map_err(|error| {
            ContainerError::message(format!("ROUKAKU_VFS_LOCAL_AUTHORIZATION: {error}"))
        })?;
        self.local_roots.insert(alias.to_owned(), roots);
        Ok(())
    }

    pub fn resolve_asset(
        &self,
        asset_id: &str,
        max_bytes: usize,
    ) -> Result<ResolvedAssetBytes, ContainerError> {
        let matches: Vec<&AssetCatalogEntry> = self
            .catalog
            .assets
            .iter()
            .filter(|asset| asset.asset_id == asset_id)
            .collect();
        let [asset] = matches.as_slice() else {
            return Err(ContainerError::message(format!(
                "ROUKAKU_VFS_ASSET_CATALOG_MAPPING: {asset_id}"
            )));
        };
        self.resolve_uri(&asset.uri, asset_id, max_bytes)
    }

    pub fn resolve_asset_identity(
        &self,
        asset_id: &str,
    ) -> Result<ResolvedAssetIdentity, ContainerError> {
        let mut assets = self
            .catalog
            .assets
            .iter()
            .filter(|asset| asset.asset_id == asset_id);
        let asset = assets.next().ok_or_else(|| {
            ContainerError::message(format!("ROUKAKU_VFS_ASSET_CATALOG_MAPPING: {asset_id}"))
        })?;
        if assets.next().is_some() {
            return Err(ContainerError::message(format!(
                "ROUKAKU_VFS_ASSET_CATALOG_MAPPING: {asset_id}"
            )));
        }
        let prefix = self
            .manifest
            .prefixes
            .iter()
            .find(|prefix| prefix.prefix == asset.uri.prefix())
            .ok_or_else(|| ContainerError::message("ROUKAKU_VFS_PREFIX_MISSING"))?;
        let mut context = self.context.clone();
        context.provider_binding = prefix.provider_id.clone();
        context.capability = prefix.backend.required_provider_capability().to_owned();
        let entry = self
            .manifest
            .resolve(&asset.uri, &context)
            .map_err(|diagnostics| {
                ContainerError::message(
                    diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.code.as_str())
                        .collect::<Vec<_>>()
                        .join(","),
                )
            })?
            .ok_or_else(|| {
                ContainerError::message(format!("ROUKAKU_VFS_ASSET_WHITEOUT: {asset_id}"))
            })?;
        Ok(ResolvedAssetIdentity {
            asset_id: asset_id.to_owned(),
            source_mount: entry.layer_id.clone(),
            content_revision: entry.hash,
        })
    }

    pub fn resolve_section(
        &self,
        section_id: &str,
        max_bytes: usize,
    ) -> Result<ResolvedAssetBytes, ContainerError> {
        let uri = VfsUri::parse(&format!("package:/{}", section_id.replace('.', "/")))
            .map_err(|error| ContainerError::message(error.to_string()))?;
        self.resolve_uri(&uri, section_id, max_bytes)
    }

    pub fn resolve_uri(
        &self,
        uri: &VfsUri,
        asset_id: &str,
        max_bytes: usize,
    ) -> Result<ResolvedAssetBytes, ContainerError> {
        let prefix = self
            .manifest
            .prefixes
            .iter()
            .find(|prefix| prefix.prefix == uri.prefix())
            .ok_or_else(|| ContainerError::message("ROUKAKU_VFS_PREFIX_MISSING"))?;
        let mut context = self.context.clone();
        context.provider_binding = prefix.provider_id.clone();
        context.capability = prefix.backend.required_provider_capability().to_owned();
        let entry = self
            .manifest
            .resolve(uri, &context)
            .map_err(|diagnostics| {
                ContainerError::message(
                    diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.code.as_str())
                        .collect::<Vec<_>>()
                        .join(","),
                )
            })?
            .ok_or_else(|| {
                ContainerError::message(format!("ROUKAKU_VFS_ASSET_WHITEOUT: {asset_id}"))
            })?;
        if entry.size > max_bytes as u64 {
            return Err(ContainerError::message(format!(
                "ROUKAKU_VFS_READ_BOUND: {asset_id}"
            )));
        }
        let source_mount = entry.layer_id.clone();
        let bytes = match &entry.source {
            VfsSourceRef::PackageSection { section_id } => {
                let reader = self
                    .mount_readers
                    .get(&entry.layer_id)
                    .unwrap_or(&self.package);
                let section = reader.container().read_bounded(section_id, max_bytes)?;
                let start = usize::try_from(entry.offset)
                    .map_err(|_| ContainerError::message("ROUKAKU_VFS_RANGE"))?;
                let end = start
                    .checked_add(entry.size as usize)
                    .ok_or_else(|| ContainerError::message("ROUKAKU_VFS_RANGE"))?;
                section
                    .get(start..end)
                    .ok_or_else(|| ContainerError::message("ROUKAKU_VFS_RANGE"))?
                    .to_vec()
            }
            VfsSourceRef::LocalAuthorized { alias } => {
                let roots = self.local_roots.get(alias).ok_or_else(|| {
                    ContainerError::message(format!("ROUKAKU_VFS_LOCAL_UNAUTHORIZED: {alias}"))
                })?;
                let local_uri = VfsUri::parse(&format!("{alias}:/{}", uri.path()))
                    .map_err(|error| ContainerError::message(error.to_string()))?;
                roots
                    .read_bounded(&local_uri, max_bytes, Some(entry.hash))
                    .map_err(|error| {
                        ContainerError::message(format!("ROUKAKU_VFS_LOCAL_READ: {error}"))
                    })?
            }
            _ => return Err(ContainerError::message("ROUKAKU_VFS_SOURCE_UNSUPPORTED")),
        };
        if bytes.len() as u64 != entry.size || Hash256::from_sha256(&bytes) != entry.hash {
            return Err(ContainerError::message(format!(
                "ROUKAKU_VFS_CONTENT_IDENTITY: {asset_id}"
            )));
        }
        Ok(ResolvedAssetBytes {
            identity: ResolvedAssetIdentity {
                asset_id: asset_id.to_owned(),
                source_mount,
                content_revision: entry.hash,
            },
            uri: uri.clone(),
            media_kind: entry.media_kind.clone(),
            codec: entry.codec.clone(),
            bytes: Arc::from(bytes),
        })
    }

    fn index_package(&mut self, package: &PackageReader) -> Result<(), ContainerError> {
        for layer in &self.manifest.layers {
            if matches!(layer.source, VfsSourceRef::PackageSection { .. }) {
                self.mount_readers
                    .entry(layer.layer_id.clone())
                    .or_insert_with(|| package.clone());
            }
        }
        Ok(())
    }
    fn index_product_sections(
        &mut self,
        package: &PackageReader,
        layer_id: &str,
    ) -> Result<(), ContainerError> {
        for section in package.container().entries() {
            let media_kind = if section.id.starts_with("vn.localization.") {
                "text"
            } else if section.id == "media.font_manifest" {
                "font_manifest"
            } else if section.id.starts_with("vn.policy") {
                "script"
            } else if section.id.starts_with("vn.ui_") {
                "ui"
            } else if section.id.starts_with("vn.") {
                "script"
            } else {
                continue;
            };
            let uri = VfsUri::parse(&format!("package:/{}", section.id.replace('.', "/")))
                .map_err(|error| ContainerError::message(error.to_string()))?;
            if self
                .manifest
                .entries
                .iter()
                .any(|entry| entry.uri == uri && entry.layer_id == layer_id)
            {
                continue;
            }
            self.manifest.entries.push(VfsManifestEntry {
                uri,
                layer_id: layer_id.to_owned(),
                source: VfsSourceRef::PackageSection {
                    section_id: section.id.clone(),
                },
                offset: 0,
                size: section.decoded_length,
                hash: section.hash,
                codec: Some(format!("{:?}", section.codec).to_ascii_lowercase()),
                media_kind: media_kind.to_owned(),
                diagnostics: Vec::new(),
            });
        }
        Ok(())
    }
}

fn read_manifest(package: &PackageReader) -> Result<VfsManifest, ContainerError> {
    serde_json::from_slice(
        &package
            .container()
            .read_bounded("asset.vfs_manifest", 32 * 1024 * 1024)?,
    )
    .map_err(|error| ContainerError::message(format!("ROUKAKU_VFS_MANIFEST_DECODE: {error}")))
}
fn read_catalog(package: &PackageReader) -> Result<AssetCatalog, ContainerError> {
    serde_json::from_slice(
        &package
            .container()
            .read_bounded("asset.catalog", 16 * 1024 * 1024)?,
    )
    .map_err(|error| ContainerError::message(format!("ROUKAKU_VFS_CATALOG_DECODE: {error}")))
}
