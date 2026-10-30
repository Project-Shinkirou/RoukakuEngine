//! Live NativeVN host regression for the shared Pango text resource path.
use super::*;
use roukaku_media::{
    FontBindingContext, GlyphRole, LayoutConstraint, OverflowPolicy, PackagedFont,
    PangoTextLayoutProvider, RubySpan, SourceRange, TextDirection, TextLayoutConfig,
    TextLayoutProvider, TextLayoutRequest, TextRenderResourceOwner, TextRun, WrapPolicy,
};
use roukaku_platform::{
    HostLaunchProfile, PlatformHostFactory, PlatformHostProfile, SurfaceRequest, WindowRequest,
};
use roukaku_player_core::{PlatformCommandSink, PlayerHostCommandExecutor};

#[tokio::test]
#[ignore = "requires a Windows hardware GPU adapter"]
async fn nativevn_page_renders_ruby_and_vertical_punctuation() {
    let bytes = product_package();
    let package = PackageReader::open(&bytes).unwrap();
    let mut source = NativeVnHostCommandSource::from_package(
        &package,
        VnRunConfig::classic("en"),
        640,
        360,
        PlayerHostResourceId(1),
    )
    .unwrap();
    let font_bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../Fixtures/PublicDomainFonts/NotoSansSC-Variable.ttf"),
    )
    .unwrap();
    let provider = PangoTextLayoutProvider::new(
        FontBindingContext {
            target: "nativevn-game".into(),
            profile: "classic".into(),
            default_locale: "ja".into(),
        },
        vec![PackagedFont {
            asset_id: "asset:/font/pango-corpus/noto-sans-sc".into(),
            family: "Noto Sans SC".into(),
            face_index: 0,
            source_mount: "memory".into(),
            hash: Hash256::from_sha256(&font_bytes),
            license_id: "OFL-1.1".into(),
            subset: None,
            coverage: roukaku_media::font_unicode_coverage(&font_bytes, 0).unwrap(),
            targets: vec!["nativevn-game".into()],
            profiles: vec!["classic".into()],
            bytes: font_bytes,
        }],
        TextLayoutConfig::production_defaults(),
    )
    .unwrap();
    let session = roukaku_platform_windows::factory()
        .start(HostLaunchProfile::platform(
            PlatformHostProfile::windows_release("nativevn-game", "com.example.pango"),
        ))
        .await
        .unwrap();
    let window = session
        .client
        .create_window(WindowRequest {
            title: "NativeVN — Pango ruby / vertical".into(),
            width: 640,
            height: 360,
            visible: true,
        })
        .await
        .unwrap();
    let surface = session
        .client
        .create_surface(SurfaceRequest {
            window,
            width: 640,
            height: 360,
        })
        .await
        .unwrap();
    let mut sink = PlatformCommandSink::new(session.client.clone());
    sink.bind_surface(PlayerHostResourceId(1), surface).unwrap();
    let mut executor = PlayerHostCommandExecutor::new(sink);
    let mut batch = source.launch().unwrap();
    let commands = batch
        .commands
        .iter_mut()
        .find_map(|command| match command {
            PlayerHostCommand::PresentScene { commands, .. } => Some(commands),
            _ => None,
        })
        .unwrap();
    let mut resources = TextRenderResourceOwner::default();
    for (direction, x, y, width, height) in [
        (TextDirection::LeftToRight, 20, 20, 320.0, 140.0),
        (TextDirection::VerticalRightToLeft, 340, 20, 160.0, 260.0),
    ] {
        let request = TextLayoutRequest {
            key: format!("nativevn.pango.{direction:?}"),
            runs: vec![TextRun {
                text: "東京、縦書き。".into(),
                language: "ja".into(),
                script: Some("Jpan".into()),
                direction,
                ruby: vec![RubySpan {
                    base_range: SourceRange { start: 0, end: 6 },
                    text: "とうきょう".into(),
                }],
                voice: None,
            }],
            constraint: LayoutConstraint {
                max_width: width,
                max_height: Some(height),
                max_lines: None,
                font_size: 32.0,
                line_height: 44.0,
                wrap: WrapPolicy::WordOrGlyph,
                overflow: OverflowPolicy::Visible,
            },
            font_families: vec!["Noto Sans SC".into()],
            features: Vec::new(),
        };
        let layout = provider.layout_shared(&request).unwrap();
        assert_eq!(
            provider.measure(&request).unwrap().revision,
            layout.revision
        );
        assert_eq!(layout.ruby_boxes.len(), 1);
        assert!(
            layout
                .shaped_runs
                .iter()
                .any(|run| matches!(run.role, GlyphRole::Ruby { .. }))
        );
        let mut text_commands = resources
            .update_layout(&request.key, &layout, [245, 235, 210, 255])
            .unwrap();
        for command in &mut text_commands {
            if let SceneCommand::GlyphRun { glyphs, .. } = command {
                for glyph in std::sync::Arc::make_mut(glyphs) {
                    glyph.x += x;
                    glyph.y += y;
                }
            }
        }
        commands.extend(text_commands);
    }
    executor.execute_batch(batch).await.unwrap();
    let captured = session.client.capture_surface(surface).await.unwrap();
    for (left, right) in [(20, 330), (340, 600)] {
        let drawn = captured
            .rgba8
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .filter(|(index, pixel)| {
                let x = index % 640;
                let y = index / 640;
                (left..right).contains(&x)
                    && (20..180).contains(&y)
                    && pixel[0] > 100
                    && pixel[1] > 100
            })
            .count();
        assert!(drawn > 100, "text region has no visible glyph coverage");
    }
    if let Some(directory) = std::env::var_os("ROUKAKU_TEXT_SMOKE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("nativevn-ruby-vertical.png");
        image::save_buffer(&path, &captured.rgba8, 640, 360, image::ColorType::Rgba8).unwrap();
        println!("Pango NativeVN screenshot: {}", path.display());
    }
    let mut released = source.release_resources().unwrap();
    for command in &mut released.commands {
        if let PlayerHostCommand::PresentScene { commands, .. } = command {
            commands.extend(resources.shutdown());
        }
    }
    executor.execute_batch(released).await.unwrap();
    source.shutdown().unwrap();
    drop(executor);
    session.client.destroy_surface(surface).await.unwrap();
    session.client.destroy_window(window).await.unwrap();
    session.client.shutdown().await.unwrap();
}
