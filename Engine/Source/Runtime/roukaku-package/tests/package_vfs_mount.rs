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

fn package_with_overlay_layer(
    layer_id: &str,
    priority: i32,
    bytes: &[u8],
) -> roukaku_package::OverlayPack {
    let section = SectionPayload::raw(
        "asset.background.opening",
        "roukaku.cooked_asset.v1",
        bytes.to_vec(),
    );
    let mut request = PackageBuildRequest::fixture(
        format!("com.example.overlay.{}", layer_id.replace('.', "-")),
        "classic",
        vec![section],
    );
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&request.asset_vfs_manifest).unwrap();
    manifest["layers"][0]["layer_id"] = layer_id.into();
    manifest["layers"][0]["priority"] = priority.into();
    manifest["entries"][0]["layer_id"] = layer_id.into();
    request.asset_vfs_manifest = serde_json::to_vec(&manifest).unwrap();
    roukaku_package::OverlayPack::open(PackageBuilder::build(request).unwrap().as_bytes()).unwrap()
}

fn mark_font_asset(request: &mut PackageBuildRequest) {
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&request.asset_vfs_manifest).unwrap();
    for entry in manifest["entries"].as_array_mut().unwrap() {
        if entry["uri"] == "package:/asset/font/primary" {
            entry["media_kind"] = "font/ttf".into();
        }
    }
    request.asset_vfs_manifest = serde_json::to_vec(&manifest).unwrap();
    let mut catalog: serde_json::Value = serde_json::from_slice(&request.asset_catalog).unwrap();
    for asset in catalog["assets"].as_array_mut().unwrap() {
        if asset["asset_id"] == "asset:/font/primary" {
            asset["media_kind"] = "font/ttf".into();
        }
    }
    request.asset_catalog = serde_json::to_vec(&catalog).unwrap();
}

#[test]
fn overlay_replaces_ui_localization_and_font_manifest_sections() {
    let mut request = PackageBuildRequest::fixture(
        "com.example.nativevn",
        "classic",
        vec![
            SectionPayload::raw(
                "vn.ui_blueprint",
                "roukaku.ui_blueprint_bundle.v2",
                b"base-ui".to_vec(),
            ),
            SectionPayload::raw(
                "vn.localization.en",
                "roukaku.vn.localization_table.v1",
                b"base-locale".to_vec(),
            ),
            SectionPayload::raw(
                "media.font_manifest",
                "roukaku.font_manifest.v1",
                b"base-font".to_vec(),
            ),
            SectionPayload::raw(
                "asset.font.primary",
                "roukaku.cooked_asset.v1",
                b"base-font-file".to_vec(),
            ),
        ],
    );
    mark_font_asset(&mut request);
    let base = PackageReader::open(PackageBuilder::build(request).unwrap().as_bytes()).unwrap();
    let mut overlay_request = PackageBuildRequest::fixture(
        "com.example.overlay.presentation",
        "classic",
        vec![
            SectionPayload::raw(
                "vn.ui_blueprint",
                "roukaku.ui_blueprint_bundle.v2",
                b"overlay-ui".to_vec(),
            ),
            SectionPayload::raw(
                "vn.localization.en",
                "roukaku.vn.localization_table.v1",
                b"overlay-locale".to_vec(),
            ),
            SectionPayload::raw(
                "media.font_manifest",
                "roukaku.font_manifest.v1",
                b"overlay-font".to_vec(),
            ),
            SectionPayload::raw(
                "asset.font.primary",
                "roukaku.cooked_asset.v1",
                b"overlay-font-file".to_vec(),
            ),
        ],
    );
    mark_font_asset(&mut overlay_request);
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&overlay_request.asset_vfs_manifest).unwrap();
    manifest["layers"][0]["layer_id"] = "overlay.presentation".into();
    manifest["layers"][0]["priority"] = 10.into();
    for entry in manifest["entries"].as_array_mut().unwrap() {
        entry["layer_id"] = "overlay.presentation".into();
    }
    overlay_request.asset_vfs_manifest = serde_json::to_vec(&manifest).unwrap();
    let overlay = roukaku_package::OverlayPack::open(
        PackageBuilder::build(overlay_request).unwrap().as_bytes(),
    )
    .unwrap();
    let mut resolver = roukaku_package::MountedAssetResolver::new(&base).unwrap();
    resolver.mount_overlay("presentation", &overlay).unwrap();
    for (section, expected) in [
        ("vn.ui_blueprint", b"overlay-ui".as_slice()),
        ("vn.localization.en", b"overlay-locale".as_slice()),
        ("media.font_manifest", b"overlay-font".as_slice()),
    ] {
        let resolved = resolver.resolve_section(section, 1024).unwrap();
        assert_eq!(resolved.bytes.as_ref(), expected);
        assert_eq!(resolved.identity.source_mount, "overlay.presentation");
    }
    let font = resolver.resolve_asset("asset:/font/primary", 1024).unwrap();
    assert_eq!(font.bytes.as_ref(), b"overlay-font-file");
    assert_eq!(font.identity.source_mount, "overlay.presentation");
}

#[test]
fn standalone_overlay_pack_is_a_valid_package_container() {
    let overlay = package_with_overlay_layer("overlay.standalone", 10, b"standalone");
    let base = base_package();
    let mut resolver = roukaku_package::MountedAssetResolver::new(&base).unwrap();
    resolver.mount_overlay("standalone", &overlay).unwrap();
    let asset = resolver
        .resolve_asset("asset:/background/opening", 1024)
        .unwrap();
    assert_eq!(asset.bytes.as_ref(), b"standalone");
    assert_eq!(asset.identity.source_mount, "overlay.standalone");
}

fn base_package() -> PackageReader {
    let section = SectionPayload::raw(
        "asset.background.opening",
        "roukaku.cooked_asset.v1",
        b"base".to_vec(),
    );
    PackageReader::open(
        PackageBuilder::build(PackageBuildRequest::fixture(
            "com.example.nativevn",
            "classic",
            vec![section],
        ))
        .unwrap()
        .as_bytes(),
    )
    .unwrap()
}

#[test]
fn mounted_overlay_overrides_and_removal_restores_base() {
    let base = base_package();
    let overlay = package_with_overlay_layer("overlay.high", 10, b"overlay");
    let mut resolver = roukaku_package::MountedAssetResolver::new(&base).unwrap();
    resolver.mount_overlay("high", &overlay).unwrap();
    let replaced = resolver
        .resolve_asset("asset:/background/opening", 1024)
        .unwrap();
    assert_eq!(replaced.bytes.as_ref(), b"overlay");
    assert_eq!(replaced.identity.source_mount, "overlay.high");
    resolver.unmount_overlay("high").unwrap();
    let restored = resolver
        .resolve_asset("asset:/background/opening", 1024)
        .unwrap();
    assert_eq!(restored.bytes.as_ref(), b"base");
    assert_eq!(restored.identity.source_mount, "package.base");
}

#[test]
fn same_priority_overlay_is_reported_as_ambiguous() {
    let base = base_package();
    let overlay = package_with_overlay_layer("overlay.equal", 0, b"overlay");
    let mut resolver = roukaku_package::MountedAssetResolver::new(&base).unwrap();
    resolver.mount_overlay("equal", &overlay).unwrap();
    let error = resolver
        .resolve_asset("asset:/background/opening", 1024)
        .unwrap_err();
    assert!(error.to_string().contains("ROUKAKU_VFS_RESOLVE_AMBIGUOUS"));
}

#[test]
fn overlay_whiteout_hides_base_asset() {
    let base = base_package();
    let mut overlay =
        PackageBuildRequest::fixture("com.example.overlay.whiteout", "classic", vec![]);
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&overlay.asset_vfs_manifest).unwrap();
    manifest["layers"][0]["layer_id"] = "overlay.whiteout".into();
    manifest["layers"][0]["priority"] = 10.into();
    manifest["whiteouts"] = serde_json::json!([{
        "vfs_uri": "package:/asset/background/opening",
        "layer_id": "overlay.whiteout",
        "base_hash": Hash256::from_sha256(b"base").to_string(),
        "allowlist_id": "test.allowlist",
        "reason": "test removal",
        "targets": [],
        "profiles": []
    }]);
    overlay.asset_vfs_manifest = serde_json::to_vec(&manifest).unwrap();
    let overlay =
        roukaku_package::OverlayPack::open(PackageBuilder::build(overlay).unwrap().as_bytes())
            .unwrap();
    let mut resolver = roukaku_package::MountedAssetResolver::new(&base).unwrap();
    resolver.mount_overlay("whiteout", &overlay).unwrap();
    let error = resolver
        .resolve_asset("asset:/background/opening", 1024)
        .unwrap_err();
    assert!(error.to_string().contains("ROUKAKU_VFS_ASSET_WHITEOUT"));
}

#[test]
fn local_catalog_source_requires_authorized_alias_and_bounded_content() {
    let payload = b"authorized local asset";
    let mut request = PackageBuildRequest::fixture(
        "com.example.localmount",
        "classic",
        vec![SectionPayload::raw(
            "asset.background.opening",
            "roukaku.cooked_asset.v1",
            payload.to_vec(),
        )],
    );
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&request.asset_vfs_manifest).unwrap();
    manifest["layers"][0]["source"] =
        serde_json::json!({"kind":"local_authorized", "alias":"project"});
    manifest["entries"][0]["source"] =
        serde_json::json!({"kind":"local_authorized", "alias":"project"});
    request.asset_vfs_manifest = serde_json::to_vec(&manifest).unwrap();
    let package = PackageReader::open(PackageBuilder::build(request).unwrap().as_bytes()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(directory.path().join("asset/background")).unwrap();
    std::fs::write(directory.path().join("asset/background/opening"), payload).unwrap();
    let mut resolver = roukaku_package::MountedAssetResolver::new(&package).unwrap();
    let denied = resolver
        .resolve_asset("asset:/background/opening", 1024)
        .unwrap_err();
    assert!(
        denied
            .to_string()
            .contains("ROUKAKU_VFS_LOCAL_UNAUTHORIZED")
    );
    resolver
        .authorize_directory("project", directory.path())
        .unwrap();
    let resolved = resolver
        .resolve_asset("asset:/background/opening", 1024)
        .unwrap();
    assert_eq!(resolved.bytes.as_ref(), payload);
    assert_eq!(resolved.identity.source_mount, "package.base");
}
