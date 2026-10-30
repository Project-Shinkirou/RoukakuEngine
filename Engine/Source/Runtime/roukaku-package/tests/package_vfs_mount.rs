use roukaku_core::Hash256;
use roukaku_package::{PackageBuildRequest, PackageBuilder, PackageReader, SectionPayload};

#[test]
fn package_vfs_mount_writes_vfs_manifest_and_catalog_without_asset_registry() {
    let cooked = SectionPayload::raw(
        "asset.background.opening",
        "roukaku.cooked_asset.v1",
        b"opening".to_vec(),
    );
    let expected_hash = Hash256::from_sha256(&cooked.payload).to_string();
    let blob = PackageBuilder::build(PackageBuildRequest::fixture(
        "com.example.nativevn",
        "classic",
        vec![cooked],
    ))
    .unwrap();

    let package = PackageReader::open(blob.as_bytes()).unwrap();
    assert!(package.has_section("asset.vfs_manifest"));
    assert!(package.has_section("asset.catalog"));
    assert!(!package.has_section("asset.registry"));

    let manifest: serde_json::Value = serde_json::from_slice(
        &package
            .container()
            .read_section("asset.vfs_manifest")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["schema"], "roukaku.asset_vfs_manifest.v1");
    assert_eq!(manifest["prefixes"][0]["prefix"], "package");
    assert_eq!(
        manifest["prefixes"][0]["provider_id"],
        "roukaku.vfs.package"
    );
    assert_eq!(
        manifest["entries"][0]["vfs_uri"],
        "package:/asset/background/opening"
    );
    assert_eq!(manifest["entries"][0]["hash"], expected_hash);
    assert_eq!(
        manifest["entries"][0]["source"]["section_id"],
        "asset.background.opening"
    );

    let catalog: serde_json::Value =
        serde_json::from_slice(&package.container().read_section("asset.catalog").unwrap())
            .unwrap();
    assert_eq!(catalog["schema"], "roukaku.asset_catalog.v1");
    assert_eq!(
        catalog["assets"][0]["vfs_uri"],
        "package:/asset/background/opening"
    );

    let provider_policy: serde_json::Value =
        serde_json::from_slice(&package.container().read_section("provider.policy").unwrap())
            .unwrap();
    assert!(provider_policy.get("runtime_provider").is_none());

    let registry: serde_json::Value = serde_json::from_slice(
        &package
            .container()
            .read_section("plugin.extension_registry")
            .unwrap(),
    )
    .unwrap();
    assert!(
        registry["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|provider| {
                provider["slot"] == "vfs_provider"
                    && provider["provider_id"] == "roukaku.vfs.package"
                    && provider["packaged"] == true
            })
    );
    assert!(
        registry["bindings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|binding| {
                binding["slot"] == "vfs_provider" && binding["provider_id"] == "roukaku.vfs.package"
            })
    );
}

#[test]
fn mounted_asset_resolver_reads_catalog_assets_with_source_identity() {
    let cooked = SectionPayload::raw(
        "asset.background.opening",
        "roukaku.cooked_asset.v1",
        b"opening revision".to_vec(),
    );
    let package = PackageReader::open(
        PackageBuilder::build(PackageBuildRequest::fixture(
            "com.example.nativevn",
            "classic",
            vec![cooked],
        ))
        .unwrap()
        .as_bytes(),
    )
    .unwrap();
    let resolver = roukaku_package::MountedAssetResolver::new(&package).unwrap();
    let asset = resolver
        .resolve_asset("asset:/background/opening", 1024)
        .unwrap();
    assert_eq!(asset.bytes.as_ref(), b"opening revision");
    assert_eq!(asset.identity.source_mount, "package.base");
    assert_eq!(
        asset.identity.content_revision,
        Hash256::from_sha256(b"opening revision")
    );
}
