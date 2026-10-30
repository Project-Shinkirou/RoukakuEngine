use super::*;

#[test]
fn licensed_multiscript_fallback_shapes_cjk_arabic_and_emoji_clusters() {
    let provider = PangoTextLayoutProvider::new(
        FontBindingContext {
            target: "windows".into(),
            profile: "classic".into(),
            default_locale: "en-US".into(),
        },
        multiscript_fonts(),
        TextLayoutConfig::production_defaults(),
    )
    .unwrap();
    let mut request = request("Latin cafe\u{301}");
    request.key = "multiscript.production".into();
    request.font_families = vec![
        "Poppins".into(),
        "Noto Sans SC".into(),
        "Noto Sans Arabic".into(),
        "Noto Emoji".into(),
    ];
    request.runs.extend([
        TextRun {
            text: "中文かなカナ".into(),
            language: "zh-CN".into(),
            script: Some("Hans".into()),
            direction: TextDirection::LeftToRight,
            ruby: vec![RubySpan {
                base_range: SourceRange { start: 0, end: 6 },
                text: "ちゅうぶん".into(),
            }],
            voice: None,
        },
        TextRun {
            text: "السَّلَامُ".into(),
            language: "ar".into(),
            script: Some("Arab".into()),
            direction: TextDirection::RightToLeft,
            ruby: Vec::new(),
            voice: None,
        },
        TextRun {
            text: "☀️👩‍💻😀".into(),
            language: "und".into(),
            script: Some("Zyyy".into()),
            direction: TextDirection::LeftToRight,
            ruby: Vec::new(),
            voice: None,
        },
    ]);
    request.constraint.max_width = 640.0;

    let layout = provider.layout(&request).unwrap();
    let families = layout
        .shaped_runs
        .iter()
        .map(|run| run.font_family.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for family in ["Poppins", "Noto Sans SC", "Noto Sans Arabic", "Noto Emoji"] {
        assert!(families.contains(family), "missing shaped family {family}");
    }
    assert!(
        layout
            .shaped_runs
            .iter()
            .filter(|run| run.run_index == 2)
            .all(|run| run.direction == TextDirection::RightToLeft)
    );
    assert!(
        layout
            .shaped_runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .all(|glyph| glyph.glyph_id != 0 && glyph.source.start <= glyph.source.end)
    );
    assert!(
        layout
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "ROUKAKU_TEXT_FONT_FALLBACK")
            .count()
            >= 3
    );
    assert!(
        layout
            .glyph_resources
            .iter()
            .any(|resource| resource.font_asset_id.ends_with("noto-emoji"))
    );
    assert_eq!(layout.revision, provider.layout(&request).unwrap().revision);
}

#[test]
fn cjk_vertical_layout_places_columns_ruby_rotated_glyphs_and_tate_chu_yoko() {
    let provider = PangoTextLayoutProvider::new(
        FontBindingContext {
            target: "windows".into(),
            profile: "classic".into(),
            default_locale: "ja".into(),
        },
        multiscript_fonts(),
        TextLayoutConfig::production_defaults(),
    )
    .unwrap();
    for (direction, language) in [
        (TextDirection::VerticalRightToLeft, "ja"),
        (TextDirection::VerticalLeftToRight, "zh-Hans"),
    ] {
        let mut vertical = request("終12A端");
        vertical.key = format!("vertical.{language}");
        vertical.font_families = vec!["Poppins".into(), "Noto Sans SC".into()];
        vertical.constraint.max_width = 240.0;
        vertical.constraint.max_height = Some(160.0);
        vertical.constraint.wrap = WrapPolicy::Glyph;
        vertical.runs[0] = TextRun {
            text: "終12A端".into(),
            language: language.into(),
            script: Some(if language == "ja" { "Jpan" } else { "Hans" }.into()),
            direction,
            ruby: vec![RubySpan {
                base_range: SourceRange { start: 0, end: 3 },
                text: "しゅう".into(),
            }],
            voice: None,
        };
        let layout = provider.layout(&vertical).unwrap();
        assert!(layout.width >= vertical.constraint.line_height);
        assert!(layout.height > 0.0);
        assert_eq!(layout.ruby_boxes.len(), 1);
        assert!(
            layout
                .shaped_runs
                .iter()
                .all(|run| run.direction == direction)
        );
        let glyphs = layout
            .shaped_runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .collect::<Vec<_>>();
        let minimum_x = glyphs
            .iter()
            .filter_map(|glyph| glyph.render_x)
            .min()
            .expect("vertical glyph x") as f32;
        match direction {
            TextDirection::VerticalRightToLeft => assert!(minimum_x > 100.0),
            TextDirection::VerticalLeftToRight => assert!(minimum_x < 100.0),
            _ => unreachable!(),
        }
        assert!(glyphs.iter().any(|glyph| glyph.tate_chu_yoko));
        assert!(
            glyphs
                .iter()
                .any(|glyph| glyph.rotation_quadrants == 1 && !glyph.tate_chu_yoko)
        );
        assert!(
            glyphs
                .iter()
                .all(|glyph| glyph.render_x.is_some() && glyph.render_y.is_some())
        );
        assert_eq!(
            layout.revision,
            provider.layout(&vertical).unwrap().revision
        );
    }
}

#[test]
fn open_font_fixture_manifest_is_revision_hash_and_license_bound() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../Fixtures/PublicDomainFonts");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
    let fonts = manifest["fonts"].as_array().unwrap();
    for entry in fonts {
        let bytes = std::fs::read(root.join(entry["file"].as_str().unwrap())).unwrap();
        assert_eq!(entry["byte_size"].as_u64().unwrap(), bytes.len() as u64);
        assert_eq!(
            entry["sha256"].as_str().unwrap(),
            Hash256::from_sha256(&bytes).to_string()
        );
        let license = std::fs::read(root.join(entry["license_file"].as_str().unwrap())).unwrap();
        assert!(!license.is_empty());
        assert!(
            entry["source_url"]
                .as_str()
                .unwrap()
                .contains(manifest["source_revision"].as_str().unwrap())
        );
    }
}

#[test]
fn wrap_ellipsis_and_source_clusters_are_semantic() {
    let provider = provider();
    let mut request = request("A very long production text line with multiple words and clusters");
    request.constraint.max_width = 96.0;
    request.constraint.max_lines = Some(1);
    request.constraint.overflow = OverflowPolicy::EllipsisEnd;
    let layout = provider.layout(&request).unwrap();
    assert!(layout.ellipsized);
    assert!(!layout.clipped);
    assert_eq!(
        layout
            .lines
            .iter()
            .filter(|line| line.run_index == 0)
            .count(),
        1
    );
    assert!(
        layout
            .shaped_runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .all(|glyph| glyph.source.end <= request.runs[0].text.len())
    );
}

#[test]
fn pango_corpus_preserves_clusters_bidi_ruby_and_cjk_breaks() {
    let provider = PangoTextLayoutProvider::new(
        FontBindingContext {
            target: "windows".into(),
            profile: "classic".into(),
            default_locale: "ja".into(),
        },
        multiscript_fonts(),
        TextLayoutConfig::production_defaults(),
    )
    .unwrap();
    for (text, language) in [("مرحبا Latin 42", "ar"), ("שלום Latin 42", "he")] {
        let mut input = request(text);
        input.runs[0].language = language.into();
        input.runs[0].direction = TextDirection::Auto;
        input.font_families = vec![
            "Poppins".into(),
            "Noto Sans Arabic".into(),
            "Noto Sans Hebrew".into(),
        ];
        input.constraint.max_width = 640.0;
        let layout = provider.layout(&input).unwrap();
        let glyphs = layout
            .shaped_runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .collect::<Vec<_>>();
        assert!(
            glyphs
                .iter()
                .any(|glyph| glyph.direction == TextDirection::RightToLeft)
        );
        assert!(
            glyphs
                .iter()
                .any(|glyph| glyph.direction == TextDirection::LeftToRight)
        );
        let rtl = glyphs
            .iter()
            .filter(|glyph| glyph.direction == TextDirection::RightToLeft)
            .collect::<Vec<_>>();
        assert!(
            rtl.windows(2)
                .any(|pair| pair[0].source.start > pair[1].source.start && pair[0].x < pair[1].x)
        );
        for glyph in glyphs {
            assert!(
                text.is_char_boundary(glyph.source.start)
                    && text.is_char_boundary(glyph.source.end)
            );
            assert!(
                glyph.source.start < glyph.source.end && glyph.x.is_finite() && glyph.y.is_finite()
            );
        }
        assert_eq!(provider.measure(&input).unwrap().revision, layout.revision);
    }
    let mut input = request("前東京、後東京。終");
    input.font_families = vec!["Noto Sans SC".into()];
    input.runs[0].language = "ja".into();
    input.runs[0].ruby = vec![RubySpan {
        base_range: SourceRange { start: 3, end: 9 },
        text: "とうきょう".into(),
    }];
    input.constraint.max_width = 60.0;
    input.constraint.wrap = WrapPolicy::WordOrGlyph;
    let layout = provider.layout(&input).unwrap();
    assert!(
        layout
            .lines
            .iter()
            .filter(|line| line.role == GlyphRole::Base)
            .count()
            > 1
    );
    let base = layout
        .shaped_runs
        .iter()
        .filter(|run| run.role == GlyphRole::Base)
        .flat_map(|run| &run.glyphs)
        .filter(|glyph| glyph.source.start >= 3 && glyph.source.end <= 9)
        .collect::<Vec<_>>();
    assert!(!base.is_empty());
    assert!(base.iter().all(|glyph| glyph.line == base[0].line));
    assert_eq!(layout.ruby_boxes[0].line, base[0].line);
    let left = base
        .iter()
        .map(|glyph| glyph.x)
        .fold(f32::INFINITY, f32::min);
    let right = base
        .iter()
        .map(|glyph| glyph.x + glyph.advance)
        .fold(0.0, f32::max);
    let ruby = &layout.ruby_boxes[0];
    assert!(((ruby.x + ruby.width * 0.5) - (left + right) * 0.5).abs() < 0.01);
    input.constraint.max_width = 160.0;
    input.constraint.max_height = Some(120.0);
    input.runs[0].direction = TextDirection::VerticalRightToLeft;
    let vertical = provider.layout(&input).unwrap();
    let punctuation = vertical
        .shaped_runs
        .iter()
        .filter(|run| run.role == GlyphRole::Base)
        .flat_map(|run| &run.glyphs)
        .filter(|glyph| {
            matches!(
                &input.runs[0].text[glyph.source.start..glyph.source.end],
                "、" | "。"
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(punctuation.len(), 2);
    assert!(punctuation.iter().all(|glyph| glyph.render_x.is_some()
        && glyph.render_y.is_some()
        && glyph.rotation_quadrants == 1));
}
