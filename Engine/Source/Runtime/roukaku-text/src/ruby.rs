//! Roukaku ruby composition: Pango owns shaping and atomic base line breaking;
//! this module aligns the annotation to the measured base cluster box.
use super::{
    contract::*,
    layout_engine::append_raw_layout,
    provider::FontState,
    shaping::shape_raw,
    validation::{ranges_overlap, validate_ruby},
};
use crate::MediaError;
use roukaku_core::Diagnostic;
use std::collections::{BTreeMap, BTreeSet};
#[allow(clippy::too_many_arguments)]
pub(super) fn append_ruby(
    state: &mut FontState,
    request: &TextLayoutRequest,
    run: &TextRun,
    run_index: usize,
    span_index: usize,
    ruby: &RubySpan,
    line_cursor: u32,
    features: &str,
    lines: &mut Vec<LayoutLine>,
    shaped_runs: &mut Vec<ShapedGlyphRun>,
    resources: &mut BTreeMap<String, GlyphResource>,
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
        &[],
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
        resources,
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
        glyph.render_x = glyph.render_x.map(|x| x + ruby_x.round() as i32);
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
