use std::collections::BTreeMap;

use roukaku_core::Diagnostic;

use super::vertical::{is_vertical, shaping_constraint, transform_vertical_run};
use crate::MediaError;

use super::ruby::append_ruby;
use super::{
    contract::*,
    provider::{FontState, RawLayout},
    shaping::{append_shaped_glyph, shape_raw},
    validation::{cluster_covered, pango_features, validate_direction},
};

pub(super) fn layout_uncached(
    request: &TextLayoutRequest,
    state: &mut FontState,
    config: &TextLayoutConfig,
) -> Result<TextLayoutResult, MediaError> {
    let features = pango_features(&request.features)?;
    let mut lines = Vec::new();
    let mut shaped_runs = Vec::new();
    let mut resources = BTreeMap::new();
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
            (request.constraint.font_size * 0.6).max(request.constraint.line_height * 0.5)
        };
        let mut base_constraint = shaping_constraint(request.constraint, run.direction);
        base_constraint.line_height += ruby_padding;
        let raw = shape_raw(
            state,
            &run.text,
            &run.language,
            run.direction,
            &request.font_families,
            &features,
            base_constraint,
            &run.ruby,
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
            &mut resources,
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
                &mut resources,
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
                &resources,
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

    let glyph_count = shaped_runs
        .iter()
        .map(|run| run.glyphs.len())
        .sum::<usize>();
    if glyph_count > config.max_glyphs {
        return Err(MediaError::message(
            "ROUKAKU_TEXT_GLYPH_BUDGET: shaped glyph count exceeds the configured budget",
        ));
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
    let glyph_resources = resources.into_values().collect::<Vec<_>>();
    let clip = if request.constraint.overflow == OverflowPolicy::Clip {
        Some(LayoutClip {
            x: 0,
            y: 0,
            width: request.constraint.max_width.ceil() as u32,
            height: request
                .constraint
                .max_height
                .unwrap_or_else(|| {
                    request.constraint.max_lines.map_or(height, |lines| {
                        lines as f32 * request.constraint.line_height
                    })
                })
                .ceil()
                .max(1.0) as u32,
        })
    } else {
        None
    };
    Ok(TextLayoutResult {
        schema: TEXT_LAYOUT_SCHEMA.to_string(),
        key: request.key.clone(),
        width,
        height,
        lines,
        shaped_runs,
        glyph_resources,
        ruby_boxes,
        voice_refs,
        clip,
        clipped,
        ellipsized,
        diagnostics,
        revision: 0,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_raw_layout(
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
    resources: &mut BTreeMap<String, GlyphResource>,
    diagnostics: &mut Vec<Diagnostic>,
    config: &TextLayoutConfig,
) -> Result<(), MediaError> {
    let RawLayout {
        lines: raw_lines, ..
    } = raw;
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
                .get(&glyph.font_id)
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
            let direction = if glyph.rtl {
                TextDirection::RightToLeft
            } else {
                TextDirection::LeftToRight
            };
            let invisible = cluster
                .chars()
                .all(|value| value.is_whitespace() || is_invisible_scalar(value));
            let (resource_id, render_x, render_y) = if let Some(bitmap) = glyph.bitmap {
                let resource_id = format!(
                    "glyph:{}:{}:{}:{}:{}:{}:{}:{}:{}",
                    face.asset_id.len(),
                    face.asset_id,
                    face.source_mount.len(),
                    face.source_mount,
                    face.hash,
                    face.face_index,
                    glyph.glyph_id,
                    glyph.size.to_bits(),
                    glyph.gravity
                );
                resources
                    .entry(resource_id.clone())
                    .or_insert_with(|| GlyphResource {
                        resource_id: resource_id.clone(),
                        font_asset_id: face.asset_id.clone(),
                        font_hash: face.hash,
                        glyph_id: glyph.glyph_id,
                        bitmap,
                    });
                (
                    Some(resource_id),
                    Some(glyph.x.round() as i32 + glyph.left),
                    Some((y_offset + raw_line.baseline + glyph.y).round() as i32 + glyph.top),
                )
            } else if invisible || glyph.w.abs() <= f32::EPSILON {
                (None, None, None)
            } else {
                return Err(MediaError::message(
                    "ROUKAKU_TEXT_GLYPH_RASTER: covered glyph could not be rasterized",
                ));
            };
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
                resource_id,
                render_x,
                render_y,
                rotation_quadrants: 0,
                tate_chu_yoko: false,
            };
            append_shaped_glyph(shaped_runs, run_index, role.clone(), shaped);
        }
        if shaped_runs
            .iter()
            .map(|run| run.glyphs.len())
            .sum::<usize>()
            > config.max_glyphs
        {
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
