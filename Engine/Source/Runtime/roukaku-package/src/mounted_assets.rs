use std::{collections::BTreeMap, path::Path, sync::Arc};

use roukaku_asset::{
    AssetCatalog, AssetCatalogEntry, LocalMountRootSet, ResolveContext, VfsManifest,
    VfsManifestEntry, VfsSourceRef, VfsUri,
};
use roukaku_core::Hash256;

use crate::{ContainerError, ContainerKind, PackageReader, RoukakuContainerReader};

/// A standalone overlay is a real package-kind container with bounded VFS metadata.
/// It does not need a launch manifest, provider registry, or an executable story.
#[derive(Debug, Clone)]
pub struct OverlayPack {
    container: RoukakuContainerReader,
    manifest: VfsManifest,
    catalog: AssetCatalog,
}

impl OverlayPack {
    pub fn open(bytes: &[u8]) -> Result<Self, ContainerError> {
        Self::from_container(RoukakuContainerReader::new(bytes)?)
    }

    pub fn from_container(container: RoukakuContainerReader) -> Result<Self, ContainerError> {
        if container.kind() != ContainerKind::Package {
            return Err(ContainerError::message(
                "ROUKAKU_VFS_OVERLAY_CONTAINER_KIND",
            ));
        }
        let manifest = read_manifest(&container)?;
        let catalog = read_catalog(&container)?;
        Ok(Self {
            container,
            manifest,
            catalog,
        })
    }
}

/// Identity of a decoded asset, independent of unrelated mount revisions.
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

/// Package, overlay-pack and explicitly authorized local-directory resolution.
#[derive(Debug, Clone)]
pub struct MountedAssetResolver {
    package: PackageReader,
    manifest: VfsManifest,
    catalog: AssetCatalog,
    mount_readers: BTreeMap<String, RoukakuContainerReader>,
    overlays: BTreeMap<String, OverlayPack>,
    local_roots: BTreeMap<String, LocalMountRootSet>,
    context: ResolveContext,
}

impl MountedAssetResolver {
    pub fn new(package: &PackageReader) -> Result<Self, ContainerError> {
        let manifest = read_manifest(package.container())?;
        let catalog = read_catalog(package.container())?;
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
            overlays: BTreeMap::new(),
            local_roots: BTreeMap::new(),
            context,
        };
        if let Some(layer_id) = result
            .manifest
            .layers
            .iter()
            .filter(|layer| layer.prefix == "package")
            .min_by_key(|layer| layer.priority)
            .map(|layer| layer.layer_id.clone())
        {
            result.index_product_sections(package.container(), &layer_id)?;
        }
        result.validate()?;
        Ok(result)
    }

    pub fn catalog_entries(&self) -> &[AssetCatalogEntry] {
        &self.catalog.assets
    }

    /// Add a new mount; failed validation leaves every existing mount unchanged.
    pub fn mount_overlay(
        &mut self,
        mount_id: &str,
        overlay: &OverlayPack,
    ) -> Result<(), ContainerError> {
        if mount_id.trim().is_empty() || self.overlays.contains_key(mount_id) {
            return Err(ContainerError::message("ROUKAKU_VFS_MOUNT_ID_INVALID"));
        }
        self.replace_overlay(mount_id, overlay)
    }

    /// Add or replace a mount atomically, rebuilding its metadata and removing old entries.
    pub fn replace_overlay(
        &mut self,
        mount_id: &str,
        overlay: &OverlayPack,
    ) -> Result<(), ContainerError> {
        if mount_id.trim().is_empty() {
            return Err(ContainerError::message("ROUKAKU_VFS_MOUNT_ID_INVALID"));
        }
        let mut overlays = self.overlays.clone();
        overlays.insert(mount_id.to_owned(), overlay.clone());
        self.rebuild(overlays)
    }

    pub fn unmount_overlay(&mut self, mount_id: &str) -> Result<(), ContainerError> {
        let mut overlays = self.overlays.clone();
        if overlays.remove(mount_id).is_none() {
            return Err(ContainerError::message("ROUKAKU_VFS_MOUNT_MISSING"));
        }
        self.rebuild(overlays)
    }

    fn rebuild(&mut self, overlays: BTreeMap<String, OverlayPack>) -> Result<(), ContainerError> {
        let mut next = Self::new(&self.package)?;
        next.local_roots = self.local_roots.clone();
        for overlay in overlays.values() {
            next.merge_overlay(overlay)?;
        }
        next.validate()?;
        next.overlays = overlays;
        *self = next;
        Ok(())
    }

    fn merge_overlay(&mut self, overlay: &OverlayPack) -> Result<(), ContainerError> {
        for prefix in &overlay.manifest.prefixes {
            if let Some(existing) = self
                .manifest
                .prefixes
                .iter()
                .find(|item| item.prefix == prefix.prefix)
            {
                if existing != prefix {
                    return Err(ContainerError::message("ROUKAKU_VFS_MOUNT_PREFIX_MISMATCH"));
                }
            } else {
                self.manifest.prefixes.push(prefix.clone());
            }
        }
        for layer in &overlay.manifest.layers {
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
            self.mount_readers
                .insert(layer.layer_id.clone(), overlay.container.clone());
            self.manifest.layers.push(layer.clone());
        }
        self.manifest
            .entries
            .extend(overlay.manifest.entries.iter().cloned());
        self.manifest
            .whiteouts
            .extend(overlay.manifest.whiteouts.iter().cloned());
        for asset in &overlay.catalog.assets {
            if let Some(existing) = self
                .catalog
                .assets
                .iter()
                .find(|existing| existing.asset_id == asset.asset_id)
            {
                if existing.uri != asset.uri
                    || existing.media_kind != asset.media_kind
                    || existing.tags != asset.tags
                {
                    return Err(ContainerError::message(format!(
                        "ROUKAKU_VFS_ASSET_ID_URI_MISMATCH: {}",
                        asset.asset_id
                    )));
                }
            } else {
                self.catalog.assets.push(asset.clone());
            }
        }
        if let Some(layer_id) = overlay
            .manifest
            .layers
            .iter()
            .filter(|layer| layer.prefix == "package")
            .max_by_key(|layer| layer.priority)
            .map(|layer| layer.layer_id.clone())
        {
            self.index_product_sections(&overlay.container, &layer_id)?;
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), ContainerError> {
        let diagnostics = self.manifest.validate();
        if !diagnostics.is_empty() {
            return Err(ContainerError::message(
                diagnostics
                    .iter()
                    .map(|d| d.code.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
            ));
        }
        let mut ids = BTreeMap::new();
        for asset in &self.catalog.assets {
            if ids.insert(&asset.asset_id, &asset.uri).is_some() {
                return Err(ContainerError::message(format!(
                    "ROUKAKU_VFS_ASSET_CATALOG_MAPPING: {}",
                    asset.asset_id
                )));
            }
        }
        Ok(())
    }

    /// Authorization grants a root to manifest LocalAuthorized references, never to arbitrary paths.
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

    fn asset_uri(&self, asset_id: &str) -> Result<&VfsUri, ContainerError> {
        self.catalog
            .assets
            .iter()
            .find(|asset| asset.asset_id == asset_id)
            .map(|asset| &asset.uri)
            .ok_or_else(|| {
                ContainerError::message(format!("ROUKAKU_VFS_ASSET_CATALOG_MAPPING: {asset_id}"))
            })
    }

    fn entry(&self, uri: &VfsUri, asset_id: &str) -> Result<&VfsManifestEntry, ContainerError> {
        let prefix = self
            .manifest
            .prefixes
            .iter()
            .find(|prefix| prefix.prefix == uri.prefix())
            .ok_or_else(|| ContainerError::message("ROUKAKU_VFS_PREFIX_MISSING"))?;
        let context = ResolveContext {
            provider_binding: prefix.provider_id.clone(),
            capability: prefix.backend.required_provider_capability().to_owned(),
            ..self.context.clone()
        };
        self.manifest
            .resolve(uri, &context)
            .map_err(|diagnostics| {
                ContainerError::message(
                    diagnostics
                        .iter()
                        .map(|d| d.code.as_str())
                        .collect::<Vec<_>>()
                        .join(","),
                )
            })?
            .ok_or_else(|| {
                ContainerError::message(format!("ROUKAKU_VFS_ASSET_WHITEOUT: {asset_id}"))
            })
    }

    pub fn resolve_asset(
        &self,
        asset_id: &str,
        max_bytes: usize,
    ) -> Result<ResolvedAssetBytes, ContainerError> {
        self.resolve_uri(self.asset_uri(asset_id)?, asset_id, max_bytes)
    }

    pub fn resolve_asset_identity(
        &self,
        asset_id: &str,
    ) -> Result<ResolvedAssetIdentity, ContainerError> {
        let entry = self.entry(self.asset_uri(asset_id)?, asset_id)?;
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
        let entry = self.entry(uri, asset_id)?;
        if entry.size > max_bytes as u64 {
            return Err(ContainerError::message(format!(
                "ROUKAKU_VFS_READ_BOUND: {asset_id}"
            )));
        }
        let bytes = match &entry.source {
            VfsSourceRef::PackageSection { section_id } => {
                let reader = self
                    .mount_readers
                    .get(&entry.layer_id)
                    .unwrap_or(self.package.container());
                let section = reader.read_bounded(section_id, max_bytes)?;
                let start = usize::try_from(entry.offset)
                    .map_err(|_| ContainerError::message("ROUKAKU_VFS_RANGE"))?;
                let size = usize::try_from(entry.size)
                    .map_err(|_| ContainerError::message("ROUKAKU_VFS_RANGE"))?;
                let end = start
                    .checked_add(size)
                    .ok_or_else(|| ContainerError::message("ROUKAKU_VFS_RANGE"))?;
                section
                    .get(start..end)
                    .ok_or_else(|| ContainerError::message("ROUKAKU_VFS_RANGE"))?
                    .to_vec()
            }
            VfsSourceRef::LocalAuthorized { alias } => {
                if entry.offset != 0 {
                    return Err(ContainerError::message("ROUKAKU_VFS_LOCAL_RANGE"));
                }
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
                source_mount: entry.layer_id.clone(),
                content_revision: entry.hash,
            },
            uri: uri.clone(),
            media_kind: entry.media_kind.clone(),
            codec: entry.codec.clone(),
            bytes: Arc::from(bytes),
        })
    }

    fn index_product_sections(
        &mut self,
        container: &RoukakuContainerReader,
        layer_id: &str,
    ) -> Result<(), ContainerError> {
        for section in container.entries() {
            let media_kind = if section.id.starts_with("vn.localization.") {
                "text"
            } else if section.id == "media.font_manifest" {
                "font_manifest"
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

fn read_manifest(container: &RoukakuContainerReader) -> Result<VfsManifest, ContainerError> {
    let manifest: VfsManifest =
        serde_json::from_slice(&container.read_bounded("asset.vfs_manifest", 16 * 1024 * 1024)?)
            .map_err(|error| {
                ContainerError::message(format!("ROUKAKU_VFS_MANIFEST_DECODE: {error}"))
            })?;
    if manifest.schema != "roukaku.asset_vfs_manifest.v1" {
        return Err(ContainerError::message("ROUKAKU_VFS_SCHEMA"));
    }
    Ok(manifest)
}

fn read_catalog(container: &RoukakuContainerReader) -> Result<AssetCatalog, ContainerError> {
    let catalog: AssetCatalog =
        serde_json::from_slice(&container.read_bounded("asset.catalog", 16 * 1024 * 1024)?)
            .map_err(|error| {
                ContainerError::message(format!("ROUKAKU_VFS_CATALOG_DECODE: {error}"))
            })?;
    if catalog.schema != "roukaku.asset_catalog.v1" {
        return Err(ContainerError::message("ROUKAKU_VFS_CATALOG_SCHEMA"));
    }
    Ok(catalog)
}
