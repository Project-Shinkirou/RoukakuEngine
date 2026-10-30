use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

use roukaku_core::Hash256;
use roukaku_media_core::TextureFrame;
use roukaku_package::{MountedAssetResolver, PackageReader, ResolvedAssetIdentity};
use roukaku_ui_core::UiValidationError;
use roukaku_ui_yakui::UiImageResourceProvider;

use crate::NativeVnHostError;

mod prefetch;
pub(crate) use prefetch::PackageImagePrefetcher;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssetKind {
    Image,
    Audio,
    Video,
}

#[derive(Debug, Clone)]
struct AssetDescriptor {
    kind: AssetKind,
}
#[derive(Debug, Clone)]
enum CachedAsset {
    Image(TextureFrame),
    Media(roukaku_byte_source::OwnedByteBuffer),
}

impl CachedAsset {
    fn bytes(&self) -> usize {
        match self {
            Self::Image(frame) => frame.rgba8.len(),
            Self::Media(bytes) => bytes.len(),
        }
    }
}

#[derive(Debug)]
struct CacheEntry {
    value: CachedAsset,
    last_used: u64,
}

#[derive(Debug)]
struct AssetCache {
    entries: BTreeMap<String, CacheEntry>,
    pinned: BTreeSet<String>,
    bytes: usize,
    max_bytes: usize,
    clock: u64,
}

impl AssetCache {
    fn get(&mut self, asset_id: &str) -> Option<CachedAsset> {
        self.clock = self.clock.saturating_add(1);
        let entry = self.entries.get_mut(asset_id)?;
        entry.last_used = self.clock;
        Some(entry.value.clone())
    }

    fn insert(
        &mut self,
        asset_id: String,
        value: CachedAsset,
    ) -> Result<Vec<CachedAsset>, NativeVnHostError> {
        let bytes = value.bytes();
        if bytes == 0 || bytes > self.max_bytes {
            return Err(NativeVnHostError::Asset(
                "ROUKAKU_PLAYER_ASSET_CACHE_ENTRY_BUDGET".into(),
            ));
        }
        self.clock = self.clock.saturating_add(1);
        let mut retired = Vec::new();
        if let Some(previous) = self.entries.remove(&asset_id) {
            self.bytes = self.bytes.saturating_sub(previous.value.bytes());
            retired.push(previous.value);
        }
        self.entries.insert(
            asset_id,
            CacheEntry {
                value,
                last_used: self.clock,
            },
        );
        self.bytes = self.bytes.saturating_add(bytes);
        while self.bytes > self.max_bytes {
            let evict_id = self
                .entries
                .iter()
                .filter(|(id, _)| !self.pinned.contains(*id))
                .min_by_key(|(id, entry)| (entry.last_used, *id))
                .map(|(id, _)| id.clone())
                .ok_or_else(|| {
                    NativeVnHostError::Asset(
                        "ROUKAKU_PLAYER_ASSET_CACHE_PINNED_BUDGET_EXCEEDED".into(),
                    )
                })?;
            let evicted = self.entries.remove(&evict_id).ok_or_else(|| {
                NativeVnHostError::Asset("ROUKAKU_PLAYER_ASSET_CACHE_STATE".into())
            })?;
            self.bytes = self.bytes.saturating_sub(evicted.value.bytes());
            retired.push(evicted.value);
            tracing::debug!(
                event = "player.asset.cache.evicted",
                asset_id = evict_id,
                cache_bytes = self.bytes,
                cache_budget_bytes = self.max_bytes,
                "evicted a package asset from the bounded CPU cache"
            );
        }
        Ok(retired)
    }

    fn insert_without_eviction(
        &mut self,
        asset_id: String,
        value: CachedAsset,
    ) -> Result<bool, NativeVnHostError> {
        let bytes = value.bytes();
        if bytes == 0 || bytes > self.max_bytes {
            return Err(NativeVnHostError::Asset(
                "ROUKAKU_PLAYER_ASSET_CACHE_ENTRY_BUDGET".into(),
            ));
        }
        if !self.can_insert_without_eviction(&asset_id, bytes) {
            return Ok(false);
        }
        self.clock = self.clock.saturating_add(1);
        if let Some(previous) = self.entries.remove(&asset_id) {
            self.bytes = self.bytes.saturating_sub(previous.value.bytes());
        }
        self.entries.insert(
            asset_id,
            CacheEntry {
                value,
                last_used: self.clock,
            },
        );
        self.bytes = self.bytes.saturating_add(bytes);
        Ok(true)
    }

    fn can_insert_without_eviction(&self, asset_id: &str, bytes: usize) -> bool {
        if bytes == 0 || bytes > self.max_bytes {
            return false;
        }
        let previous_bytes = self
            .entries
            .get(asset_id)
            .map_or(0, |entry| entry.value.bytes());
        self.bytes
            .saturating_sub(previous_bytes)
            .saturating_add(bytes)
            <= self.max_bytes
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LoadedMediaAsset {
    pub codec: String,
    pub bytes: roukaku_byte_source::OwnedByteBuffer,
    pub byte_length: u64,
}

#[derive(Debug)]
pub(crate) struct PackageAssetStore {
    resolver: MountedAssetResolver,
    descriptors: BTreeMap<String, AssetDescriptor>,
    revisions: Mutex<BTreeMap<String, ResolvedAssetIdentity>>,
    cache: Mutex<AssetCache>,
}
impl PackageAssetStore {
    pub fn index_with_mounts(
        package: &PackageReader,
        max_cache_bytes: u64,
        overlays: &[(&str, &PackageReader)],
        authorized_directories: &[(&str, &std::path::Path)],
    ) -> Result<Arc<Self>, NativeVnHostError> {
        let max_cache_bytes = usize::try_from(max_cache_bytes).map_err(|_| {
            NativeVnHostError::Asset("ROUKAKU_PLAYER_ASSET_CACHE_BUDGET_RANGE".into())
        })?;
        if max_cache_bytes == 0 {
            return Err(NativeVnHostError::Asset(
                "ROUKAKU_PLAYER_ASSET_CACHE_BUDGET_ZERO".into(),
            ));
        }
        let mut resolver = MountedAssetResolver::new(package)
            .map_err(|error| NativeVnHostError::Asset(error.to_string()))?;
        for (mount_id, overlay) in overlays {
            resolver
                .mount_overlay(mount_id, overlay)
                .map_err(|error| NativeVnHostError::Asset(error.to_string()))?;
        }
        for (alias, directory) in authorized_directories {
            resolver
                .authorize_directory(alias, directory)
                .map_err(|error| {
                    NativeVnHostError::Asset(format!(
                        "ROUKAKU_VFS_LOCAL_UNAUTHORIZED: {alias}: {error}"
                    ))
                })?;
        }
        let mut descriptors = BTreeMap::new();
        for asset in resolver.catalog_entries() {
            let kind = if catalog_asset_has_type(asset, "image.") {
                AssetKind::Image
            } else if catalog_asset_has_type(asset, "video.") {
                AssetKind::Video
            } else if catalog_asset_has_type(asset, "audio.")
                || catalog_asset_has_type(asset, "voice")
            {
                AssetKind::Audio
            } else {
                continue;
            };
            if descriptors
                .insert(asset.asset_id.clone(), AssetDescriptor { kind })
                .is_some()
            {
                return Err(NativeVnHostError::Asset(format!(
                    "ROUKAKU_PLAYER_ASSET_ID_DUPLICATE: {}",
                    asset.asset_id
                )));
            }
        }
        Ok(Arc::new(Self {
            resolver,
            descriptors,
            revisions: Mutex::new(BTreeMap::new()),
            cache: Mutex::new(AssetCache {
                entries: BTreeMap::new(),
                pinned: BTreeSet::new(),
                bytes: 0,
                max_bytes: max_cache_bytes,
                clock: 0,
            }),
        }))
    }

    pub fn resolve_asset_bytes(
        &self,
        asset_id: &str,
        max_bytes: usize,
    ) -> Result<roukaku_package::ResolvedAssetBytes, NativeVnHostError> {
        self.resolver
            .resolve_asset(asset_id, max_bytes)
            .map_err(|error| NativeVnHostError::Asset(error.to_string()))
    }

    pub fn resolve_section(
        &self,
        section_id: &str,
        max_bytes: usize,
    ) -> Result<roukaku_package::ResolvedAssetBytes, NativeVnHostError> {
        self.resolver
            .resolve_section(section_id, max_bytes)
            .map_err(|error| NativeVnHostError::Asset(error.to_string()))
    }

    pub fn load_policy_actions(
        &self,
        package: &PackageReader,
        max_bytes: u64,
        scope: &roukaku_runtime::TaskScope,
    ) -> Result<crate::PreparedPolicyActions, NativeVnHostError> {
        crate::PreparedPolicyActions::load_with_resolver(package, max_bytes, scope, &self.resolver)
    }

    pub fn contains_image(&self, asset_id: &str) -> bool {
        self.descriptors
            .get(asset_id)
            .is_some_and(|descriptor| descriptor.kind == AssetKind::Image)
    }

    pub fn contains_media(&self, asset_id: &str) -> bool {
        self.descriptors.get(asset_id).is_some_and(|descriptor| {
            matches!(descriptor.kind, AssetKind::Audio | AssetKind::Video)
        })
    }

    pub fn contains_audio(&self, asset_id: &str) -> bool {
        self.descriptors
            .get(asset_id)
            .is_some_and(|descriptor| descriptor.kind == AssetKind::Audio)
    }

    pub fn cache_bytes(&self) -> u64 {
        self.cache
            .lock()
            .map(|cache| cache.bytes as u64)
            .unwrap_or(u64::MAX)
    }

    pub fn is_image_cached(&self, asset_id: &str) -> Result<bool, NativeVnHostError> {
        self.cache
            .lock()
            .map_err(|_| NativeVnHostError::Asset("ROUKAKU_PLAYER_ASSET_CACHE_POISONED".into()))
            .map(|cache| {
                matches!(
                    cache.entries.get(asset_id).map(|entry| &entry.value),
                    Some(CachedAsset::Image(_))
                )
            })
    }

    pub fn release_uploaded_image(&self, asset_id: &str) -> Result<bool, NativeVnHostError> {
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| NativeVnHostError::Asset("ROUKAKU_PLAYER_ASSET_CACHE_POISONED".into()))?;
        let is_image = matches!(
            cache.entries.get(asset_id).map(|entry| &entry.value),
            Some(CachedAsset::Image(_))
        );
        if !is_image {
            return Ok(false);
        }
        cache.pinned.remove(asset_id);
        let removed = cache
            .entries
            .remove(asset_id)
            .ok_or_else(|| NativeVnHostError::Asset("ROUKAKU_PLAYER_ASSET_CACHE_STATE".into()))?;
        cache.bytes = cache
            .bytes
            .checked_sub(removed.value.bytes())
            .ok_or_else(|| {
                NativeVnHostError::Asset("ROUKAKU_PLAYER_ASSET_CACHE_BYTES_UNDERFLOW".into())
            })?;
        Ok(true)
    }

    pub fn pin_image_working_set(&self, asset_ids: &[String]) -> Result<(), NativeVnHostError> {
        if let Some(asset_id) = asset_ids
            .iter()
            .find(|asset_id| !self.contains_image(asset_id))
        {
            return Err(NativeVnHostError::Asset(format!(
                "ROUKAKU_PLAYER_IMAGE_PIN_KIND: asset_hash={}",
                Hash256::from_sha256(asset_id.as_bytes())
            )));
        }
        let pinned = asset_ids.iter().cloned().collect::<BTreeSet<_>>();
        if pinned.len() != asset_ids.len() {
            return Err(NativeVnHostError::Asset(
                "ROUKAKU_PLAYER_IMAGE_PIN_DUPLICATE".into(),
            ));
        }
        self.cache
            .lock()
            .map_err(|_| NativeVnHostError::Asset("ROUKAKU_PLAYER_ASSET_CACHE_LOCK".into()))?
            .pinned = pinned;
        Ok(())
    }

    pub fn load_image(&self, asset_id: &str) -> Result<TextureFrame, NativeVnHostError> {
        if let Some(CachedAsset::Image(frame)) = self.cache_get(asset_id)? {
            return Ok(frame);
        }
        let frame = self.decode_image(asset_id)?;
        self.cache_insert(asset_id, CachedAsset::Image(frame.clone()))?;
        Ok(frame)
    }

    pub fn prewarm_image_prefix(&self, asset_ids: &[String]) -> Result<usize, NativeVnHostError> {
        let mut retained = 0usize;
        for asset_id in asset_ids {
            if matches!(self.cache_get(asset_id)?, Some(CachedAsset::Image(_))) {
                retained += 1;
                continue;
            }
            let frame = self.decode_image(asset_id)?;
            let inserted = self
                .cache
                .lock()
                .map_err(|_| NativeVnHostError::Asset("ROUKAKU_PLAYER_ASSET_CACHE_LOCK".into()))?
                .insert_without_eviction(asset_id.clone(), CachedAsset::Image(frame))?;
            if !inserted {
                break;
            }
            retained += 1;
        }
        Ok(retained)
    }

    pub fn load_media(&self, asset_id: &str) -> Result<LoadedMediaAsset, NativeVnHostError> {
        let descriptor = self
            .descriptors
            .get(asset_id)
            .filter(|descriptor| matches!(descriptor.kind, AssetKind::Audio | AssetKind::Video))
            .ok_or_else(|| {
                NativeVnHostError::Asset(format!("ROUKAKU_PLAYER_MEDIA_ASSET_MISSING: {asset_id}"))
            })?;
        let bytes = match self.cache_get(asset_id)? {
            Some(CachedAsset::Media(bytes)) => bytes,
            Some(CachedAsset::Image(_)) => {
                return Err(NativeVnHostError::Asset(
                    "ROUKAKU_PLAYER_ASSET_CACHE_KIND_MISMATCH".into(),
                ));
            }
            None => {
                let bytes = roukaku_byte_source::OwnedByteBuffer::from_vec(
                    self.read_asset(asset_id, descriptor)?,
                );
                self.cache_insert(asset_id, CachedAsset::Media(bytes.clone()))?;
                bytes
            }
        };
        let codec = match descriptor.kind {
            AssetKind::Audio => sniff_audio_codec(&bytes),
            AssetKind::Video => sniff_video_codec(&bytes),
            AssetKind::Image => None,
        }
        .ok_or_else(|| {
            NativeVnHostError::Asset(format!(
                "ROUKAKU_PLAYER_MEDIA_CODEC_UNSUPPORTED: {asset_id}"
            ))
        })?;
        let byte_length = bytes.len() as u64;
        Ok(LoadedMediaAsset {
            codec: codec.into(),
            bytes,
            byte_length,
        })
    }

    fn descriptor(
        &self,
        asset_id: &str,
        expected_kind: AssetKind,
    ) -> Result<&AssetDescriptor, NativeVnHostError> {
        self.descriptors
            .get(asset_id)
            .filter(|descriptor| descriptor.kind == expected_kind)
            .ok_or_else(|| {
                NativeVnHostError::Asset(format!("ROUKAKU_PLAYER_ASSET_MISSING: {asset_id}"))
            })
    }

    fn decode_image(&self, asset_id: &str) -> Result<TextureFrame, NativeVnHostError> {
        let descriptor = self.descriptor(asset_id, AssetKind::Image)?;
        let encoded = self.read_asset(asset_id, descriptor)?;
        let decoded = image::load_from_memory(&encoded)
            .map_err(|error| {
                NativeVnHostError::Asset(format!("ROUKAKU_PLAYER_ASSET_DECODE: {error}"))
            })?
            .into_rgba8();
        let (width, height) = decoded.dimensions();
        TextureFrame::from_vec(width, height, decoded.into_raw()).map_err(|error| {
            NativeVnHostError::Asset(format!("ROUKAKU_PLAYER_ASSET_TEXTURE: {error}"))
        })
    }

    fn read_asset(
        &self,
        asset_id: &str,
        _descriptor: &AssetDescriptor,
    ) -> Result<Vec<u8>, NativeVnHostError> {
        let resolved = self
            .resolver
            .resolve_asset(asset_id, usize::MAX)
            .map_err(|error| NativeVnHostError::Asset(error.to_string()))?;
        Ok(resolved.bytes.to_vec())
    }

    fn cache_get(&self, asset_id: &str) -> Result<Option<CachedAsset>, NativeVnHostError> {
        let current = self
            .resolver
            .resolve_asset_identity(asset_id)
            .map_err(|error| NativeVnHostError::Asset(error.to_string()))?;
        let changed = {
            let mut revisions = self.revisions.lock().map_err(|_| {
                NativeVnHostError::Asset("ROUKAKU_PLAYER_ASSET_REVISION_LOCK".into())
            })?;
            match revisions.insert(asset_id.to_owned(), current.clone()) {
                Some(previous) => previous != current,
                None => false,
            }
        };
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| NativeVnHostError::Asset("ROUKAKU_PLAYER_ASSET_CACHE_POISONED".into()))?;
        if changed {
            cache.pinned.remove(asset_id);
            if let Some(removed) = cache.entries.remove(asset_id) {
                cache.bytes = cache.bytes.saturating_sub(removed.value.bytes());
            }
            return Ok(None);
        }
        Ok(cache.get(asset_id))
    }

    fn cache_insert(&self, asset_id: &str, value: CachedAsset) -> Result<(), NativeVnHostError> {
        let retired = {
            let mut cache = self.cache.lock().map_err(|_| {
                NativeVnHostError::Asset("ROUKAKU_PLAYER_ASSET_CACHE_POISONED".into())
            })?;
            cache.insert(asset_id.into(), value)?
        };
        drop(retired);
        Ok(())
    }
}

impl UiImageResourceProvider for PackageAssetStore {
    fn load_image(&self, asset: &str) -> Result<TextureFrame, UiValidationError> {
        PackageAssetStore::load_image(self, asset).map_err(|error| {
            UiValidationError::invalid("ROUKAKU_UI_IMAGE_RESOURCE_LOAD", error.to_string())
        })
    }
}

fn catalog_asset_has_type(asset: &roukaku_asset::AssetCatalogEntry, prefix: &str) -> bool {
    let mime_prefix = prefix.strip_suffix('.').map(|value| format!("{value}/"));
    asset.media_kind.starts_with(prefix)
        || mime_prefix
            .as_deref()
            .is_some_and(|prefix| asset.media_kind.starts_with(prefix))
        || asset.tags.iter().any(|tag| {
            tag.starts_with(prefix)
                || mime_prefix
                    .as_deref()
                    .is_some_and(|prefix| tag.starts_with(prefix))
        })
}

fn sniff_audio_codec(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"ID3")
        || bytes
            .get(..2)
            .is_some_and(|prefix| prefix[0] == 0xff && prefix[1] & 0xe0 == 0xe0)
    {
        Some("mp3")
    } else if bytes.starts_with(b"OggS") {
        Some("ogg")
    } else if bytes.starts_with(b"fLaC") {
        Some("flac")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") {
        Some("wav")
    } else {
        None
    }
}

fn sniff_video_codec(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        Some("webm")
    } else if bytes.get(4..8) == Some(b"ftyp") {
        Some("mp4")
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
