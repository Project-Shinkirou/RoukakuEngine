use super::*;

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
