use roukaku_asset::VfsUri;
use roukaku_core::Hash256;
use roukaku_media::{
    FONT_PACKAGE_MANIFEST_SCHEMA, FontPackageEntry, FontPackageManifest, UnicodeRange,
};
use roukaku_package::{PackageBuildRequest, PackageBuilder, PackageReader, SectionPayload};
use roukaku_player_core::PlayerHostResourceId;
use roukaku_player_vn::NativeVnHostCommandSource;
use roukaku_plugin_abi::{
    LoadPhase, PLUGIN_EXTENSION_REGISTRY_SCHEMA, PROVIDER_POLICY_SCHEMA,
    PluginExtensionRegistrySnapshot, ProviderBinding, ProviderBindingContext,
    ProviderExtensionRecord, ProviderPolicy,
};
use roukaku_vn_core::{
    CompileRoukakuProjectOptions, RoukakuSource, VnRunConfig, compile_roukaku_project,
};
use roukaku_vn_package::{PLAYER_LOCALE_CONFIG_SCHEMA, package_sections_for_project};

const TEST_UI: &str = include_str!("../fixtures/shared.screen.jsonc");

#[allow(dead_code)]
pub fn source_for(story: &str) -> NativeVnHostCommandSource {
    source_from_package(product_package_with_video(story, None, false))
}

#[allow(dead_code)]
pub fn source_for_video(story: &str) -> NativeVnHostCommandSource {
    let bytes =
        include_bytes!("../../../../Runtime/roukaku-media/tests/fixtures/silent.mp4").to_vec();
    source_from_package(product_package_with_video(story, Some(bytes), false))
}

fn source_from_package(package_bytes: Vec<u8>) -> NativeVnHostCommandSource {
    let package = PackageReader::open(&package_bytes).unwrap();
    NativeVnHostCommandSource::from_package(
        &package,
        VnRunConfig::classic("en"),
        320,
        180,
        PlayerHostResourceId(1),
    )
    .unwrap()
}

#[allow(dead_code)]
pub fn product_package(story: &str) -> Vec<u8> {
    product_package_with_video(story, None, false)
}

#[allow(dead_code)]
pub fn source_for_audio(story: &str) -> NativeVnHostCommandSource {
    source_from_package(product_package_with_video(story, None, true))
}

fn product_package_with_video(story: &str, video: Option<Vec<u8>>, audio: bool) -> Vec<u8> {
    let compiled = compile_roukaku_project(
        [
            RoukakuSource::story("main.roukaku", story),
            RoukakuSource::ui("test-ui.screen.jsonc", TEST_UI),
        ],
        test_compile_options(),
    )
    .unwrap();
    let mut sections =
        package_sections_for_project(&compiled, &["classic".to_string()], "nativevn-game").unwrap();
    let font =
        include_bytes!("../../../../../../Examples/NativeVN/Assets/Fonts/Poppins-Regular.ttf")
            .to_vec();
    let font_hash = Hash256::from_sha256(&font);
    sections.push(SectionPayload::raw(
        "asset.font.ui",
        "roukaku.cooked_asset.v1",
        font.clone(),
    ));
    let video_hash = video.as_ref().map(|bytes| Hash256::from_sha256(bytes));
    let video_fallback = include_bytes!(
        "../../../../../../Examples/NativeVN/Visual/Backgrounds/apartment-studio.png"
    );
    let video_fallback_hash = Hash256::from_sha256(video_fallback);
    if let Some(bytes) = &video {
        sections.push(SectionPayload::raw(
            "asset.video.intro",
            "roukaku.cooked_asset.v1",
            bytes.clone(),
        ));
        sections.push(SectionPayload::raw(
            "asset.video.intro_fallback",
            "roukaku.cooked_asset.v1",
            video_fallback.to_vec(),
        ));
    }
    sections.push(SectionPayload::raw(
        "media.font_manifest",
        FONT_PACKAGE_MANIFEST_SCHEMA,
        serde_json::to_vec(&FontPackageManifest {
            schema: FONT_PACKAGE_MANIFEST_SCHEMA.to_string(),
            target: "nativevn-game".to_string(),
            profile: "classic".to_string(),
            provider_binding: "roukaku.vfs.package".to_string(),
            fonts: vec![FontPackageEntry {
                asset_id: "asset:/font/ui".to_string(),
                uri: VfsUri::parse("package:/fonts/ui.ttf").unwrap(),
                family: "Poppins".to_string(),
                face_index: 0,
                hash: font_hash,
                license_id: "OFL-1.1".to_string(),
                subset: Some("latin-basic".to_string()),
                coverage: vec![UnicodeRange {
                    start: 32,
                    end: 126,
                }],
                targets: vec!["nativevn-game".to_string()],
                profiles: vec!["classic".to_string()],
            }],
        })
        .unwrap(),
    ));
    sections.push(SectionPayload::raw(
        "vn.localization.en",
        "roukaku.vn.localization_table.v1",
        br#"{
            "schema":"roukaku.vn.localization_table.v1",
            "locale":"en",
            "strings":{
                "line":"Production line.",
                "line.after":"Line after timeline.",
                "speaker.hero":"Hero",
                "system.back":"Back",
                "system.quick_panel":"Quick Panel"
            }
        }"#
        .to_vec(),
    ));
    sections.push(SectionPayload::raw(
        "player.locale_config",
        PLAYER_LOCALE_CONFIG_SCHEMA,
        br#"{
            "schema":"roukaku.player_locale_config.v1",
            "default_locale":"en",
            "available_locales":["en"]
        }"#
        .to_vec(),
    ));

    let audio_wave = if audio {
        let samples = vec![0_u8; 128];
        let mut wave = b"RIFF".to_vec();
        wave.extend_from_slice(&(36_u32 + samples.len() as u32).to_le_bytes());
        wave.extend_from_slice(b"WAVEfmt ");
        wave.extend_from_slice(&16_u32.to_le_bytes());
        wave.extend_from_slice(&1_u16.to_le_bytes());
        wave.extend_from_slice(&2_u16.to_le_bytes());
        wave.extend_from_slice(&48_000_u32.to_le_bytes());
        wave.extend_from_slice(&192_000_u32.to_le_bytes());
        wave.extend_from_slice(&4_u16.to_le_bytes());
        wave.extend_from_slice(&16_u16.to_le_bytes());
        wave.extend_from_slice(b"data");
        wave.extend_from_slice(&(samples.len() as u32).to_le_bytes());
        wave.extend_from_slice(&samples);
        sections.push(SectionPayload::raw(
            "asset.audio.restore",
            "roukaku.cooked_asset.v1",
            wave.clone(),
        ));
        Some(wave)
    } else {
        None
    };
    let mut request = PackageBuildRequest::fixture("com.example.player.audio", "classic", sections);
    bind_product_provider_authority(&mut request);
    let mut vfs_entries = vec![serde_json::json!({
        "vfs_uri": "package:/fonts/ui.ttf",
        "layer_id": "package.base",
        "source": {"kind": "package_section", "section_id": "asset.font.ui"},
        "offset": 0,
        "size": font.len(),
        "hash": font_hash,
        "codec": "raw",
        "media_kind": "font",
        "diagnostics": []
    })];
    if let (Some(bytes), Some(hash)) = (&video, video_hash) {
        vfs_entries.push(serde_json::json!({
            "vfs_uri": "package:/video/intro.mp4",
            "layer_id": "package.base",
            "source": {"kind": "package_section", "section_id": "asset.video.intro"},
            "offset": 0,
            "size": bytes.len(),
            "hash": hash,
            "codec": "raw",
            "media_kind": "video/mp4",
            "diagnostics": []
        }));
        vfs_entries.push(serde_json::json!({
            "vfs_uri": "package:/video/intro-fallback.png",
            "layer_id": "package.base",
            "source": {"kind": "package_section", "section_id": "asset.video.intro_fallback"},
            "offset": 0,
            "size": video_fallback.len(),
            "hash": video_fallback_hash,
            "codec": "raw",
            "media_kind": "image/png",
            "diagnostics": []
        }));
    }
    request.asset_vfs_manifest = serde_json::to_vec(&serde_json::json!({
        "schema": "roukaku.asset_vfs_manifest.v1",
        "prefixes": [{
            "prefix": "package",
            "provider_id": "roukaku.vfs.package",
            "backend": "package",
            "case_policy": "case_sensitive",
            "mode": "read_only",
            "redaction": "shipping",
            "capabilities": ["vfs.backend.package"]
        }],
        "layers": [{
            "layer_id": "package.base",
            "prefix": "package",
            "priority": 0,
            "source": {"kind": "package_section", "section_id": "package.manifest"},
            "targets": ["nativevn-game"],
            "profiles": ["classic"]
        }],
        "entries": vfs_entries,
        "whiteouts": []
    }))
    .unwrap();
    let mut catalog_assets = vec![serde_json::json!({
        "asset_id": "asset:/font/ui",
        "vfs_uri": "package:/fonts/ui.ttf",
        "media_kind": "font",
        "tags": ["ui"],
        "bundle_id": "classic",
        "chunk_id": "base",
        "profiles": ["classic"]
    })];
    if video.is_some() {
        catalog_assets.push(serde_json::json!({
            "asset_id": "asset:/video/intro",
            "vfs_uri": "package:/video/intro.mp4",
            "media_kind": "video/mp4",
            "tags": ["video.mp4"],
            "bundle_id": "classic",
            "chunk_id": "base",
            "profiles": ["classic"]
        }));
        catalog_assets.push(serde_json::json!({
            "asset_id": "asset:/video/intro-fallback",
            "vfs_uri": "package:/video/intro-fallback.png",
            "media_kind": "image/png",
            "tags": ["fallback.frame"],
            "bundle_id": "classic",
            "chunk_id": "base",
            "profiles": ["classic"]
        }));
    }
    request.asset_catalog = serde_json::to_vec(&serde_json::json!({
        "schema": "roukaku.asset_catalog.v1",
        "assets": catalog_assets
    }))
    .unwrap();
    if let Some(wave) = audio_wave {
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&request.asset_vfs_manifest).unwrap();
        manifest["entries"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "vfs_uri": "package:/audio/restore.wav", "layer_id": "package.base",
                "source": {"kind":"package_section", "section_id":"asset.audio.restore"},
                "offset":0, "size":wave.len(), "hash":Hash256::from_sha256(&wave),
                "codec":"raw", "media_kind":"audio/wav", "diagnostics":[]
            }));
        request.asset_vfs_manifest = serde_json::to_vec(&manifest).unwrap();
        let mut catalog: serde_json::Value =
            serde_json::from_slice(&request.asset_catalog).unwrap();
        catalog["assets"].as_array_mut().unwrap().push(serde_json::json!({
            "asset_id":"asset:/audio/restore", "vfs_uri":"package:/audio/restore.wav",
            "media_kind":"audio/wav", "tags":[], "bundle_id":"classic", "chunk_id":"base", "profiles":["classic"]
        }));
        request.asset_catalog = serde_json::to_vec(&catalog).unwrap();
    }
    PackageBuilder::build(request).unwrap().into_bytes()
}

fn test_compile_options() -> CompileRoukakuProjectOptions {
    let theme = roukaku_ui_core::UiThemeManifest {
        schema: "roukaku.ui_theme_manifest.v1".into(),
        id: "roukaku.vn.theme.classic".into(),
        parent: None,
        tokens: [(
            "surface.system".into(),
            roukaku_ui_core::UiThemeValue::Color([0, 0, 0, 255]),
        )]
        .into_iter()
        .collect(),
        high_contrast_tokens: Default::default(),
        revision: 1,
    };
    test_controller_options(CompileRoukakuProjectOptions::default().with_ui_theme(theme))
}

fn test_controller_options(
    mut options: CompileRoukakuProjectOptions,
) -> CompileRoukakuProjectOptions {
    let source = test_controller_source();
    for id in ["test.message", "test.choice", "test.system"] {
        options = options.with_ui_controller_source(id, source.clone());
    }
    options
}

fn test_controller_source() -> String {
    r#"
local controllers = {
  { "test.message", "ui.test.message", "roukaku.vn.ui_model.message.v2" },
  { "test.choice", "ui.test.choice", "roukaku.vn.ui_model.choice.v1" },
  { "test.system", "ui.test.system", "roukaku.vn.ui_model.system.v1" },
}
for _, definition in controllers do
  roukaku.ui.controller.register(definition[1], {
    schema = "roukaku.vn.ui_controller.v1", view = definition[2],
    model_schema = definition[3], snapshot = "none",
  }, { on_action = function(_, _, action)
    return { roukaku.ui.effect.forward(action) }
  end })
end
"#
    .to_string()
}

fn bind_product_provider_authority(request: &mut PackageBuildRequest) {
    let specs = [
        ("presentation", "roukaku.renderer.wgpu", "renderer2d.wgpu"),
        ("vfs_provider", "roukaku.vfs.package", "vfs.backend.package"),
    ];
    let bindings = specs
        .iter()
        .map(|(slot, provider_id, capability)| {
            ProviderBinding::new(
                *slot,
                *provider_id,
                ProviderBindingContext {
                    package_id: request.package_id.clone(),
                    target: "nativevn-game".to_string(),
                    profile: request.profile.clone(),
                    required_capability: capability.to_string(),
                    engine_version: env!("CARGO_PKG_VERSION").to_string(),
                    rustc_fingerprint: "rustc-stable".to_string(),
                    feature_fingerprint: "runtime-typed-v3".to_string(),
                    abi_fingerprint: "roukaku-plugin-abi-v3".to_string(),
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    request.provider_policy = serde_json::to_vec(&ProviderPolicy {
        schema: PROVIDER_POLICY_SCHEMA.to_string(),
        profile: request.profile.clone(),
        renderer: "roukaku.renderer.wgpu".to_string(),

        bindings: bindings.clone(),
    })
    .unwrap();
    request.plugin_extension_registry = serde_json::to_vec(&PluginExtensionRegistrySnapshot {
        schema: PLUGIN_EXTENSION_REGISTRY_SCHEMA.to_string(),
        providers: specs
            .iter()
            .map(|(slot, provider_id, capability)| ProviderExtensionRecord {
                slot: slot.to_string(),
                provider_id: provider_id.to_string(),
                capability: capability.to_string(),
                phase: LoadPhase::Runtime,
                packaged: true,
                engine_version: env!("CARGO_PKG_VERSION").to_string(),
                rustc_fingerprint: "rustc-stable".to_string(),
                feature_fingerprint: "runtime-typed-v3".to_string(),
                abi_fingerprint: "roukaku-plugin-abi-v3".to_string(),
            })
            .collect(),
        bindings,
        conflicts: vec![],
    })
    .unwrap();
    request.target_manifest = serde_json::to_vec(&serde_json::json!({
        "schema": "roukaku.target_manifest.v3",
        "targets": [{
            "id": "nativevn-game",
            "kind": "game",
            "crate": "roukaku-vn",
            "default_profile": "classic",
            "platforms": ["windows", "web"],
            "packaged": true
        }]
    }))
    .unwrap();
    request.platform_eligibility = serde_json::to_vec(&serde_json::json!({
        "schema": "roukaku.platform_eligibility.v1",
        "target": "nativevn-game",
        "profiles": ["classic"],
        "platforms": ["windows", "web"]
    }))
    .unwrap();
}
