use super::*;
use roukaku_text::{GlyphPlacement, GlyphResource, LayoutClip, ShapedGlyph, TextLayoutResult};

#[test]
fn density_preserves_logical_lines_clusters_and_ruby_but_grows_masks() {
    let provider = provider();
    let mut request = request("AV cafe\u{301} office AV tail");
    request.constraint.max_width = 128.25;
    request.constraint.max_height = Some(96.125);
    request.constraint.line_height = 31.75;
    request.constraint.overflow = OverflowPolicy::Clip;
    request.runs[0].ruby.push(RubySpan {
        base_range: SourceRange { start: 0, end: 2 },
        text: "pair".into(),
    });

    let logical = provider.shape_shared(&request).unwrap();
    assert!(logical.lines.iter().filter(|line| line.role == GlyphRole::Base).count() > 1);
    let native = provider.layout_shared(&request).unwrap();
    let dense = provider.rasterize_shared(&logical, 2.0, (0.25, -0.5)).unwrap();
    assert!(Arc::ptr_eq(&native, &provider.rasterize_shared(&logical, 1.0, (0.0, 0.0)).unwrap()));
    assert_eq!(provider.measure(&request).unwrap().width, dense.width);
    assert_eq!((dense.width, dense.height), (logical.width, logical.height));
    assert_eq!(dense.lines, native.lines);
    assert_eq!(dense.ruby_boxes, native.ruby_boxes);
    assert_eq!(dense.clipped, native.clipped);
    assert_eq!(dense.clip, Some(LayoutClip { x: 0, y: -1, width: 257, height: 193 }));
    for (run, native_run) in dense.shaped_runs.iter().zip(&native.shaped_runs) {
        assert_eq!(run.glyphs.len(), native_run.glyphs.len());
        for (glyph, native_glyph) in run.glyphs.iter().zip(&native_run.glyphs) {
            assert_eq!(glyph.source, native_glyph.source);
            assert_eq!(glyph.glyph_id, native_glyph.glyph_id);
            assert_eq!(glyph.line, native_glyph.line);
            assert_eq!(GlyphPlacement::from(glyph), GlyphPlacement::from(native_glyph));
        }
    }
    for role in [GlyphRole::Base, GlyphRole::Ruby { span_index: 0 }] {
        let first = |layout: &TextLayoutResult| {
            layout.shaped_runs.iter().find(|run| run.role == role).unwrap().glyphs[0]
                .resource_id.clone().unwrap()
        };
        let native_bitmap = &native.glyph_resources.iter().find(|r| r.resource_id == first(&native)).unwrap().bitmap;
        let dense_id = first(&dense);
        let dense_bitmap = &dense.glyph_resources.iter().find(|r| r.resource_id == dense_id).unwrap().bitmap;
        assert_ne!(first(&native), dense_id);
        assert!(dense_bitmap.width > native_bitmap.width && dense_bitmap.height > native_bitmap.height);
    }
    assert!(Arc::ptr_eq(&dense, &provider.rasterize_shared(&logical, 2.0, (0.25, -0.5)).unwrap()));
    assert_eq!(provider.cache_stats().unwrap().misses, 1);
}

#[test]
fn x_and_y_phase_shift_actual_coverage_and_whole_pixels_share_masks() {
    let provider = provider();
    let logical = provider.shape_shared(&request("A")).unwrap();
    let raster = |offset| provider.rasterize_shared(&logical, 1.0, offset).unwrap();
    let zero = raster((0.0, 0.0));
    let x_shift = raster((-0.5, 0.0));
    let y_shift = raster((0.0, -0.5));
    let shifted = raster((-0.5, -0.5));

    for changed in [&x_shift, &y_shift, &shifted] {
        let (_, base) = first_mask(&zero);
        let (_, change) = first_mask(changed);
        assert_ne!(base.resource_id, change.resource_id);
        assert_ne!(base.bitmap.pixels, change.bitmap.pixels);
    }
    let centroid = coverage_center(&zero);
    for (layout, dx, dy) in [(&x_shift, -0.5, 0.0), (&y_shift, 0.0, -0.5)] {
        let center = coverage_center(layout);
        assert!((center.0 - centroid.0 - dx).abs() < 0.15, "X centroid: {center:?} vs {centroid:?}");
        assert!((center.1 - centroid.1 - dy).abs() < 0.15, "Y centroid: {center:?} vs {centroid:?}");
    }

    let integral = raster((2.5, 5.5));
    let (glyph, mask) = first_mask(&shifted);
    let (moved, same_mask) = first_mask(&integral);
    assert_eq!(mask.resource_id, same_mask.resource_id);
    assert_eq!(mask.bitmap.pixels.as_ref().as_ptr(), same_mask.bitmap.pixels.as_ref().as_ptr());
    assert_eq!(moved.render_x.unwrap() - glyph.render_x.unwrap(), 3);
    assert_eq!(moved.render_y.unwrap() - glyph.render_y.unwrap(), 6);
    assert!(Arc::ptr_eq(&shifted, &raster((-0.5, -0.5))));
}

#[test]
fn logical_placement_and_reveal_precede_physical_phase() {
    let provider = provider();
    let logical = provider.shape_shared(&request("A V")).unwrap();
    let mut edited = Arc::clone(&logical);
    Arc::make_mut(&mut edited)
        .place_glyphs(|_, _, glyph| {
            (glyph.source.start == 0).then(|| GlyphPlacement {
                x: glyph.x + 0.5,
                y: glyph.y + 0.25,
                advance: glyph.advance + 3.0,
            })
        })
        .unwrap();

    let raster = |layout, offset| provider.rasterize_shared(layout, 2.0, offset).unwrap();
    let positioned = raster(&edited, (0.0, 0.0));
    let equivalent = raster(&logical, (1.0, 0.5));
    let original = raster(&logical, (0.0, 0.0));
    assert_eq!(positioned.shaped_runs.iter().map(|run| run.glyphs.len()).sum::<usize>(), 1);
    assert_eq!(positioned.glyph_resources.len(), 1);
    assert_eq!(positioned.lines, original.lines);
    assert_ne!(positioned.revision, original.revision);
    let (glyph, mask) = first_mask(&positioned);
    let (translated, reference) = first_mask(&equivalent);
    assert_eq!(glyph.source, translated.source);
    assert_eq!(glyph.advance, translated.advance + 3.0);
    assert_eq!((glyph.render_x, glyph.render_y), (translated.render_x, translated.render_y));
    assert_eq!(mask.resource_id, reference.resource_id);
    assert!(Arc::ptr_eq(&positioned, &raster(&edited, (0.0, 0.0))));
}

#[test]
fn invalid_density_and_unrepresentable_offsets_are_rejected() {
    let provider = provider();
    let logical = provider.shape_shared(&request("A")).unwrap();
    for scale in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::MAX] {
        let error = provider.rasterize_shared(&logical, scale, (0.0, 0.0)).unwrap_err();
        assert!(error.to_string().contains("ROUKAKU_TEXT_RASTER_SCALE"));
    }
    for x in [f32::NAN, f32::NEG_INFINITY, f32::MAX] {
        let error = provider.rasterize_shared(&logical, 1.0, (x, 0.0)).unwrap_err();
        assert!(error.to_string().contains("ROUKAKU_TEXT_RASTER_BOUNDS"));
    }
}

fn first_mask(layout: &TextLayoutResult) -> (&ShapedGlyph, &GlyphResource) {
    let glyph = &layout.shaped_runs[0].glyphs[0];
    let resource = layout.glyph_resources.iter().find(|r| Some(&r.resource_id) == glyph.resource_id.as_ref()).unwrap();
    (glyph, resource)
}

fn coverage_center(layout: &TextLayoutResult) -> (f64, f64) {
    let (glyph, resource) = first_mask(layout);
    let bitmap = &resource.bitmap;
    let mut weight = 0.0;
    let mut center = (0.0, 0.0);
    for (i, coverage) in bitmap.pixels.as_ref().iter().enumerate() {
        let coverage = f64::from(*coverage);
        let x = i % bitmap.width as usize;
        let y = i / bitmap.width as usize;
        weight += coverage;
        center.0 += coverage * x as f64;
        center.1 += coverage * y as f64;
    }
    assert!(weight > 0.0);
    (f64::from(glyph.render_x.unwrap()) + center.0 / weight, f64::from(glyph.render_y.unwrap()) + center.1 / weight)
}
