use super::*;
use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use roukaku_package::{PackageBuildRequest, PackageBuilder, PackageReader, SectionPayload};

fn png(pixel: [u8; 4]) -> Vec<u8> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(&pixel, 1, 1, ExtendedColorType::Rgba8)
        .unwrap();
    bytes
}

fn package_with_images(layer_id: &str, priority: i32, assets: Vec<(&str, [u8; 4])>) -> Vec<u8> {
    let mut request = PackageBuildRequest::fixture(
        format!("com.example.{layer_id}"),
        "classic",
        assets
            .into_iter()
            .map(|(id, pixel)| SectionPayload::raw(id, "roukaku.cooked_asset.v1", png(pixel)))
            .collect(),
    );
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&request.asset_vfs_manifest).unwrap();
    for entry in manifest["entries"].as_array_mut().unwrap() {
        entry["media_kind"] = "image/png".into();
    }
    let mut catalog: serde_json::Value = serde_json::from_slice(&request.asset_catalog).unwrap();
    for asset in catalog["assets"].as_array_mut().unwrap() {
        asset["media_kind"] = "image/png".into();
    }
    request.asset_catalog = serde_json::to_vec(&catalog).unwrap();
    manifest["layers"][0]["layer_id"] = layer_id.into();
    manifest["layers"][0]["priority"] = priority.into();
    for entry in manifest["entries"].as_array_mut().unwrap() {
        entry["layer_id"] = layer_id.into();
    }
    request.asset_vfs_manifest = serde_json::to_vec(&manifest).unwrap();
    PackageBuilder::build(request).unwrap().as_bytes().to_vec()
}

#[test]
fn mount_revision_invalidates_only_changed_cached_assets_and_unmount_restores_base() {
    let base_bytes = package_with_images(
        "package.base",
        0,
        vec![
            ("asset.background.opening", [10, 20, 30, 255]),
            ("asset.background.neighbor", [40, 50, 60, 255]),
        ],
    );
    let base = PackageReader::open(&base_bytes).unwrap();
    let store = PackageAssetStore::index_with_mounts(&base, 1024 * 1024, &[], &[]).unwrap();
    let changed = "asset:/background/opening";
    let neighbor = "asset:/background/neighbor";
    assert_eq!(
        store.load_image(changed).unwrap().rgba8.as_slice(),
        &[10, 20, 30, 255]
    );
    assert_eq!(
        store.load_image(neighbor).unwrap().rgba8.as_slice(),
        &[40, 50, 60, 255]
    );
    let overlay_bytes = package_with_images(
        "overlay.live",
        10,
        vec![("asset.background.opening", [70, 80, 90, 255])],
    );
    let overlay = roukaku_package::OverlayPack::open(&overlay_bytes).unwrap();
    store.mount_overlay("live", &overlay).unwrap();
    assert!(!store.is_image_cached(changed).unwrap());
    assert!(store.is_image_cached(neighbor).unwrap());
    assert_eq!(
        store.load_image(changed).unwrap().rgba8.as_slice(),
        &[70, 80, 90, 255]
    );
    assert_eq!(
        store.load_image(neighbor).unwrap().rgba8.as_slice(),
        &[40, 50, 60, 255]
    );
    let revised_bytes = package_with_images(
        "overlay.live",
        10,
        vec![("asset.background.opening", [100, 110, 120, 255])],
    );
    let revised = roukaku_package::OverlayPack::open(&revised_bytes).unwrap();
    store.mount_overlay("live", &revised).unwrap();
    assert!(!store.is_image_cached(changed).unwrap());
    assert!(store.is_image_cached(neighbor).unwrap());
    assert_eq!(
        store.load_image(changed).unwrap().rgba8.as_slice(),
        &[100, 110, 120, 255]
    );
    store.unmount_overlay("live").unwrap();
    assert!(!store.is_image_cached(changed).unwrap());
    assert!(store.is_image_cached(neighbor).unwrap());
    assert_eq!(
        store.load_image(changed).unwrap().rgba8.as_slice(),
        &[10, 20, 30, 255]
    );
}

fn media(bytes: &[u8]) -> CachedAsset {
    CachedAsset::Media(bytes.to_vec().into())
}

#[test]
fn decoded_cache_evicts_least_recently_used_entry_within_bound() {
    let mut cache = AssetCache {
        entries: BTreeMap::new(),
        pinned: BTreeSet::new(),
        bytes: 0,
        max_bytes: 6,
        clock: 0,
    };
    drop(cache.insert("a".into(), media(b"aaa")).unwrap());
    drop(cache.insert("b".into(), media(b"bbb")).unwrap());
    assert!(cache.get("a").is_some());
    drop(cache.insert("c".into(), media(b"ccc")).unwrap());
    assert!(cache.entries.contains_key("a"));
    assert!(!cache.entries.contains_key("b"));
    assert!(cache.entries.contains_key("c"));
    assert_eq!(cache.bytes, 6);
}

#[test]
fn decoded_cache_rejects_single_entry_over_budget() {
    let mut cache = AssetCache {
        entries: BTreeMap::new(),
        pinned: BTreeSet::new(),
        bytes: 0,
        max_bytes: 2,
        clock: 0,
    };
    let error = cache.insert("oversized".into(), media(b"abc")).unwrap_err();
    assert!(error.to_string().contains("CACHE_ENTRY_BUDGET"));
    assert!(cache.entries.is_empty());
}

#[test]
fn prewarm_insert_never_evicts_an_authored_prefix() {
    let mut cache = AssetCache {
        entries: BTreeMap::new(),
        pinned: BTreeSet::new(),
        bytes: 0,
        max_bytes: 6,
        clock: 0,
    };
    assert!(
        cache
            .insert_without_eviction("a".into(), media(b"aaaa"))
            .unwrap()
    );
    assert!(
        !cache
            .insert_without_eviction("b".into(), media(b"bbb"))
            .unwrap()
    );
    assert!(cache.entries.contains_key("a"));
    assert!(!cache.entries.contains_key("b"));
    assert_eq!(cache.bytes, 4);
}

#[test]
fn background_admission_accounts_for_replacement_without_evicting_neighbors() {
    let mut cache = AssetCache {
        entries: BTreeMap::new(),
        pinned: BTreeSet::new(),
        bytes: 0,
        max_bytes: 8,
        clock: 0,
    };
    drop(cache.insert("a".into(), media(b"aaaa")).unwrap());
    drop(cache.insert("b".into(), media(b"bbbb")).unwrap());

    assert!(cache.can_insert_without_eviction("a", 4));
    assert!(!cache.can_insert_without_eviction("a", 5));
    assert!(
        !cache
            .insert_without_eviction("a".into(), media(b"aaaaa"))
            .unwrap()
    );
    assert_eq!(cache.bytes, 8);
    assert!(cache.entries.contains_key("a"));
    assert!(cache.entries.contains_key("b"));
}

#[test]
fn decoded_cache_never_evicts_the_explicit_working_set() {
    let mut cache = AssetCache {
        entries: BTreeMap::new(),
        pinned: BTreeSet::new(),
        bytes: 0,
        max_bytes: 6,
        clock: 0,
    };
    drop(cache.insert("a".into(), media(b"aaa")).unwrap());
    drop(cache.insert("b".into(), media(b"bbb")).unwrap());
    cache.pinned.insert("a".into());
    drop(cache.insert("c".into(), media(b"ccc")).unwrap());
    assert!(cache.entries.contains_key("a"));
    assert!(!cache.entries.contains_key("b"));
    assert!(cache.entries.contains_key("c"));
}
