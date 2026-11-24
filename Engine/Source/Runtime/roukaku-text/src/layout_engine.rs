use std::collections::BTreeSet;

use cosmic_text::FontFeatures;
use roukaku_core::Diagnostic;

use super::vertical::{is_vertical, shaping_constraint, transform_vertical_run};
use crate::MediaError;

use super::{
    contract::*,
    logical::{RasterGlyph, ShapedText},
    provider::{FontState, RawLayout},
    shaping::{append_shaped_glyph, shape_raw},
    validation::{
        cluster_covered, cosmic_features, ranges_overlap, validate_direction, validate_ruby,
    },
};

pub(super) fn shape_uncached(
    request: &TextLayoutRequest,
    state: &mut FontState,
    config: &TextLayoutConfig,
) -> Result<ShapedText, MediaError> {
    let features = cosmic_features(&request.features)?;
    let mut lines = Vec::new();
    let mut shaped_runs = Vec::new();
    let mut raster_glyphs = Vec::new();
    let mut ruby_boxes = Vec::new();
    let mut voice_refs = Vec::new();
    let mut diagnostics = Vec::new();
    let mut line_cursor = 0u32;
    let mut y_cursor = 0.0f32;
    let mut clipped = false;
    let mut ellipsized = false;

    for (run_index, run) in request.runs.iter().enumerate() {
        if let Some(voice) = &run.voice {
            voice_refs.push(VoiceReplayRefRecord {
                run_index,
                asset: voice.asset.clone(),
                cue: voice.cue.clone(),
            });
        }
        let ruby_padding = if run.ruby.is_empty() {
            0.0
        } else {
            request.constraint.font_size * 0.6
        };
        let raw = shape_raw(
            state,
            &run.text,
            &run.language,
            run.direction,
            &request.font_families,
            &features,
            shaping_constraint(request.constraint, run.direction),
            config,
        )?;
        clipped |= raw.overflowed && request.constraint.overflow == OverflowPolicy::Clip;
        ellipsized |= raw.overflowed
            && matches!(
                request.constraint.overflow,
                OverflowPolicy::EllipsisStart
                    | OverflowPolicy::EllipsisMiddle
                    | OverflowPolicy::EllipsisEnd
            );
        append_raw_layout(
            raw,
            state,
            run_index,
            GlyphRole::Base,
            line_cursor,
            y_cursor + ruby_padding,
            &run.text,
            run.direction,
            &request.font_families,
            &mut lines,
            &mut shaped_runs,
            &mut raster_glyphs,
            &mut diagnostics,
            config,
        )?;
        let base_line_count = lines
            .iter()
            .filter(|line| line.run_index == run_index && line.role == GlyphRole::Base)
            .count() as u32;
        for (span_index, ruby) in run.ruby.iter().enumerate() {
            append_ruby(
                state,
                request,
                run,
                run_index,
                span_index,
                ruby,
                line_cursor,
                &features,
                &mut lines,
                &mut shaped_runs,
                &mut raster_glyphs,
                &mut ruby_boxes,
                &mut diagnostics,
                config,
            )?;
        }
        if is_vertical(run.direction) {
            transform_vertical_run(
                run,
                run_index,
                line_cursor,
                request.constraint,
                &mut lines,
                &mut shaped_runs,
                &mut ruby_boxes,
            )?;
        }
        let run_bottom = lines
            .iter()
            .filter(|line| line.run_index == run_index)
            .map(|line| line.top + line.height)
            .fold(y_cursor + ruby_padding, f32::max);
        y_cursor = run_bottom.max(y_cursor + request.constraint.line_height);
        line_cursor = line_cursor
            .checked_add(base_line_count.max(1))
            .ok_or_else(|| {
                MediaError::message("ROUKAKU_TEXT_LINE_COUNT: layout line count overflow")
            })?;
    }

    let width = lines
        .iter()
        .map(|line| {
            if request
                .runs
                .get(line.run_index)
                .is_some_and(|run| is_vertical(run.direction))
            {
                (line.baseline - request.constraint.font_size + line.width).max(0.0)
            } else {
                line.width
            }
        })
        .chain(ruby_boxes.iter().map(|ruby| ruby.x + ruby.width))
        .fold(0.0, f32::max);
    let height = lines
        .iter()
        .map(|line| line.top + line.height)
        .chain(ruby_boxes.iter().map(|ruby| ruby.y + ruby.height))
        .fold(0.0, f32::max);
    let clip_extent = (request.constraint.overflow == OverflowPolicy::Clip).then(|| {
        (
            request.constraint.max_width,
            request.constraint.max_height.unwrap_or_else(|| {
                request
                    .constraint
                    .max_lines
                    .map_or(height, |lines| lines as f32 * request.constraint.line_height)
            }),
        )
    });
    let clip = clip_extent.map(|(width, height)| LayoutClip {
        x: 0,
        y: 0,
        width: width.ceil() as u32,
        height: height.ceil().max(1.0) as u32,
    });
    Ok(ShapedText {
        layout: TextLayoutResult {
            schema: TEXT_LAYOUT_SCHEMA.to_string(),
            key: request.key.clone(),
            width,
            height,
            lines,
            shaped_runs,
            glyph_resources: Vec::new(),
            ruby_boxes,
            voice_refs,
            clip,
            clipped,
            ellipsized,
            diagnostics,
            revision: 0,
        },
        raster_glyphs,
        clip_extent,
    })
}

#[allow(clippy::too_many_arguments)]
fn append_ruby(
    state: &mut FontState,
    request: &TextLayoutRequest,
    run: &TextRun,
    run_index: usize,
    span_index: usize,
    ruby: &RubySpan,
    line_cursor: u32,
    features: &FontFeatures,
    lines: &mut Vec<LayoutLine>,
    shaped_runs: &mut Vec<ShapedGlyphRun>,
    raster_glyphs: &mut Vec<RasterGlyph>,
    ruby_boxes: &mut Vec<RubyLayoutBox>,
    diagnostics: &mut Vec<Diagnostic>,
    config: &TextLayoutConfig,
) -> Result<(), MediaError> {
    validate_ruby(run, ruby)?;
    let base_glyphs = shaped_runs
        .iter()
        .filter(|shaped| shaped.run_index == run_index && shaped.role == GlyphRole::Base)
        .flat_map(|shaped| &shaped.glyphs)
        .filter(|glyph| ranges_overlap(&glyph.source, &ruby.base_range))
        .collect::<Vec<_>>();
    if base_glyphs.is_empty() {
        return Err(MediaError::message(
            "ROUKAKU_TEXT_RUBY_CLUSTER: ruby range did not resolve to a shaped base cluster",
        ));
    }
    let base_lines = base_glyphs
        .iter()
        .map(|glyph| glyph.line)
        .collect::<BTreeSet<_>>();
    if base_lines.len() != 1 {
        return Err(MediaError::message(
            "ROUKAKU_TEXT_RUBY_MULTILINE: one ruby span cannot cross a wrapped line boundary",
        ));
    }
    let line = *base_lines.first().expect("base line set is non-empty");
    let base_x = base_glyphs
        .iter()
        .map(|glyph| glyph.x)
        .fold(f32::INFINITY, f32::min);
    let base_end = base_glyphs
        .iter()
        .map(|glyph| glyph.x + glyph.advance)
        .fold(f32::NEG_INFINITY, f32::max);
    let base_width = (base_end - base_x).max(0.0);
    let base_top = lines
        .iter()
        .find(|candidate| {
            candidate.run_index == run_index
                && candidate.role == GlyphRole::Base
                && candidate.line == line
        })
        .map(|candidate| candidate.top)
        .ok_or_else(|| MediaError::message("ROUKAKU_TEXT_RUBY_LINE: base line is missing"))?;
    let ruby_size = request.constraint.font_size * 0.5;
    let ruby_height = request.constraint.line_height * 0.5;
    let ruby_constraint = LayoutConstraint {
        max_width: base_width.max(ruby_size),
        max_height: None,
        max_lines: Some(1),
        font_size: ruby_size,
        line_height: ruby_height,
        wrap: WrapPolicy::None,
        overflow: OverflowPolicy::Visible,
    };
    let raw = shape_raw(
        state,
        &ruby.text,
        &run.language,
        run.direction,
        &request.font_families,
        features,
        ruby_constraint,
        config,
    )?;
    let ruby_width = raw.lines.first().map(|value| value.width).unwrap_or(0.0);
    let ruby_x = base_x + (base_width - ruby_width) * 0.5;
    let ruby_y = (base_top - ruby_height).max(0.0);
    append_raw_layout(
        raw,
        state,
        run_index,
        GlyphRole::Ruby { span_index },
        line,
        ruby_y,
        &ruby.text,
        run.direction,
        &request.font_families,
        lines,
        shaped_runs,
        raster_glyphs,
        diagnostics,
        config,
    )?;
    for glyph in shaped_runs
        .iter_mut()
        .filter(|shaped| {
            shaped.run_index == run_index
                && shaped.role == GlyphRole::Ruby { span_index }
                && shaped.line == line
        })
        .flat_map(|shaped| &mut shaped.glyphs)
    {
        glyph.x += ruby_x;
    }
    ruby_boxes.push(RubyLayoutBox {
        run_index,
        span_index,
        base_range: ruby.base_range.clone(),
        line: line_cursor + (line - line_cursor),
        x: ruby_x,
        y: ruby_y,
        width: ruby_width,
        height: ruby_height,
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn append_raw_layout(
    raw: RawLayout,
    state: &mut FontState,
    run_index: usize,
    role: GlyphRole,
    line_offset: u32,
    y_offset: f32,
    source_text: &str,
    requested_direction: TextDirection,
    family_chain: &[String],
    lines: &mut Vec<LayoutLine>,
    shaped_runs: &mut Vec<ShapedGlyphRun>,
    raster_glyphs: &mut Vec<RasterGlyph>,
    diagnostics: &mut Vec<Diagnostic>,
    config: &TextLayoutConfig,
) -> Result<(), MediaError> {
    let RawLayout { lines: raw_lines, .. } = raw;
    for (line_index, raw_line) in raw_lines.into_iter().enumerate() {
        validate_direction(requested_direction, raw_line.rtl)?;
        let line_number = line_offset + line_index as u32;
        lines.push(LayoutLine {
            run_index,
            role: role.clone(),
            line: line_number,
            source: raw_line.source.clone(),
            rtl: raw_line.rtl,
            top: y_offset + raw_line.top,
            baseline: y_offset + raw_line.baseline,
            width: raw_line.width,
            height: raw_line.height,
        });
        for mut glyph in raw_line.glyphs {
            glyph.start += raw_line.source_offset;
            glyph.end += raw_line.source_offset;
            if glyph.start > glyph.end || glyph.end > source_text.len() {
                return Err(MediaError::message(
                    "ROUKAKU_TEXT_CLUSTER_RANGE: shaper returned an invalid source cluster",
                ));
            }
            let face = state
                .faces
                .get(&glyph.font_id.to_string())
                .cloned()
                .ok_or_else(|| {
                    MediaError::message(
                        "ROUKAKU_TEXT_UNPACKAGED_FACE: shaper selected a face outside the package database",
                    )
                })?;
            let family_index = family_chain
                .iter()
                .position(|family| family.eq_ignore_ascii_case(&face.family))
                .ok_or_else(|| {
                    MediaError::message(
                        "ROUKAKU_TEXT_FALLBACK_UNDECLARED: shaper selected an undeclared fallback face",
                    )
                })?;
            if glyph.glyph_id == 0 {
                return Err(MediaError::message(
                    "ROUKAKU_TEXT_GLYPH_MISSING: packaged fallback chain does not cover a source cluster",
                ));
            }
            let cluster = &source_text[glyph.start..glyph.end];
            if cluster.is_empty() {
                tracing::trace!(
                    target: "roukaku_text",
                    event = "text.font.synthetic_glyph",
                    family = %face.family,
                    asset_id = %face.asset_id,
                    source_offset = glyph.start,
                );
            } else {
                if !cluster_covered(cluster, &face.coverage) {
                    return Err(MediaError::message(
                        "ROUKAKU_TEXT_FONT_COVERAGE: shaped cluster is outside the packaged coverage declaration",
                    ));
                }
                let expected_family_index = family_chain
                    .iter()
                    .position(|family| {
                        state.faces.values().any(|candidate| {
                            candidate.family.eq_ignore_ascii_case(family)
                                && cluster_covered(cluster, &candidate.coverage)
                        })
                    })
                    .ok_or_else(|| {
                        MediaError::message(
                            "ROUKAKU_TEXT_GLYPH_MISSING: no declared fallback coverage contains the source cluster",
                        )
                    })?;
                if family_index != expected_family_index {
                    return Err(MediaError::message(format!(
                        "ROUKAKU_TEXT_FALLBACK_ORDER: shaper did not select the first eligible declared fallback: actual_index={family_index} expected_index={expected_family_index} source_start={} source_end={}",
                        glyph.start, glyph.end,
                    )));
                }
                if family_index > 0
                    && !diagnostics.iter().any(|diagnostic| {
                        diagnostic.code == "ROUKAKU_TEXT_FONT_FALLBACK"
                            && diagnostic
                                .fields
                                .get("family")
                                .is_some_and(|family| family == &face.family)
                    })
                {
                    tracing::debug!(
                        target: "roukaku_text",
                        event = "text.font.fallback",
                        family = %face.family,
                        asset_id = %face.asset_id,
                        fallback_index = family_index,
                    );
                    diagnostics.push(
                        Diagnostic::warning(
                            "ROUKAKU_TEXT_FONT_FALLBACK",
                            "shaping consumed an explicitly declared packaged fallback face",
                        )
                        .with_field("family", &face.family)
                        .with_field("asset_id", &face.asset_id),
                    );
                }
            }
            let direction = if glyph.level.number().is_multiple_of(2) {
                TextDirection::LeftToRight
            } else {
                TextDirection::RightToLeft
            };
            raster_glyphs.push(RasterGlyph {
                font: glyph.font_id,
                size: glyph.font_size,
                weight: glyph.font_weight,
                flags: glyph.cache_key_flags,
                x_offset: glyph.font_size * glyph.x_offset,
                y_offset: glyph.font_size * -glyph.y_offset + raw_line.baseline - raw_line.top,
                invisible: cluster
                    .chars()
                    .all(|value| value.is_whitespace() || is_invisible_scalar(value)),
                zero_advance: glyph.w.abs() <= f32::EPSILON,
            });
            let shaped = ShapedGlyph {
                source: SourceRange {
                    start: glyph.start,
                    end: glyph.end,
                },
                glyph_id: glyph.glyph_id,
                font_asset_id: face.asset_id.clone(),
                font_family: face.family.clone(),
                font_face_index: face.face_index,
                font_hash: face.hash,
                direction,
                x: glyph.x,
                y: y_offset + raw_line.top + glyph.y,
                advance: glyph.w,
                baseline: y_offset + raw_line.baseline,
                line: line_number,
                resource_id: None,
                render_x: None,
                render_y: None,
                rotation_quadrants: 0,
                tate_chu_yoko: false,
            };
            append_shaped_glyph(shaped_runs, run_index, role.clone(), shaped);
        }
        if raster_glyphs.len() > config.max_glyphs {
            return Err(MediaError::message(
                "ROUKAKU_TEXT_GLYPH_BUDGET: shaped glyph count exceeds the configured budget",
            ));
        }
    }
    Ok(())
}

fn is_invisible_scalar(value: char) -> bool {
    value.is_control()
        || matches!(
            value as u32,
            0x200b..=0x200f | 0x202a..=0x202e | 0x2060..=0x206f | 0xfe00..=0xfe0f
        )
}
