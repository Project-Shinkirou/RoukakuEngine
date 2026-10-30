use super::{
    contract::*,
    native::{NativeFonts, NativeRange, NativeSpan, cstring},
    provider::{FontState, RawLayout},
    validation::cluster_covered,
};
use crate::MediaError;
use unicode_segmentation::UnicodeSegmentation;

pub(super) fn append_shaped_glyph(
    runs: &mut Vec<ShapedGlyphRun>,
    run_index: usize,
    role: GlyphRole,
    glyph: ShapedGlyph,
) {
    if let Some(last) = runs.last_mut()
        && last.run_index == run_index
        && last.role == role
        && last.line == glyph.line
        && last.direction == glyph.direction
        && last.font_asset_id == glyph.font_asset_id
        && last.font_face_index == glyph.font_face_index
        && last.font_hash == glyph.font_hash
    {
        last.glyphs.push(glyph);
        return;
    }
    runs.push(ShapedGlyphRun {
        run_index,
        role,
        line: glyph.line,
        direction: glyph.direction,
        font_asset_id: glyph.font_asset_id.clone(),
        font_family: glyph.font_family.clone(),
        font_face_index: glyph.font_face_index,
        font_hash: glyph.font_hash,
        baseline: glyph.baseline,
        glyphs: vec![glyph],
    });
}

#[allow(clippy::too_many_arguments)]
pub(super) fn shape_raw(
    state: &mut FontState,
    text: &str,
    locale: &str,
    direction: TextDirection,
    families: &[String],
    features: &str,
    constraint: LayoutConstraint,
    keep: &[RubySpan],
    config: &TextLayoutConfig,
) -> Result<RawLayout, MediaError> {
    if !state.locales.contains(locale) && state.locales.len() == config.max_locales {
        return Err(MediaError::message(
            "ROUKAKU_TEXT_LOCALE_BUDGET: active locale count exceeds the configured budget",
        ));
    }
    state.locales.insert(locale.to_string());
    if state.native.is_none() {
        let ids = state
            .fonts
            .iter()
            .map(|font| {
                state
                    .faces
                    .iter()
                    .find(|(_, face)| face.asset_id == font.asset_id)
                    .map(|(id, _)| id.clone())
                    .expect("validated font face")
            })
            .collect();
        state.native = Some(NativeFonts::new(&state.fonts, ids, config.max_glyphs)?);
    }
    let names = families
        .iter()
        .map(|family| cstring(family))
        .collect::<Result<Vec<_>, _>>()?;
    let mut spans: Vec<NativeSpan> = Vec::new();
    for (start, grapheme) in text.grapheme_indices(true) {
        let index = families
            .iter()
            .position(|family| {
                state.faces.values().any(|face| {
                    face.family.eq_ignore_ascii_case(family)
                        && cluster_covered(grapheme, &face.coverage)
                })
            })
            .ok_or_else(|| {
                MediaError::message(
                    "ROUKAKU_TEXT_GLYPH_MISSING: no declared fallback covers a grapheme cluster",
                )
            })?;
        let family = names[index].as_ptr();
        let end = (start + grapheme.len()) as i32;
        if let Some(previous) = spans.last_mut()
            && previous.family == family
        {
            previous.end = end;
        } else {
            spans.push(NativeSpan {
                start: start as i32,
                end,
                family,
            });
        }
    }
    let keep = keep
        .iter()
        .map(|ruby| NativeRange {
            start: ruby.base_range.start as i32,
            end: ruby.base_range.end as i32,
        })
        .collect::<Vec<_>>();
    let (lines, overflowed) = state.native.as_mut().expect("initialized font map").shape(
        text,
        locale,
        features,
        &spans,
        &keep,
        match direction {
            TextDirection::Auto => 0,
            TextDirection::LeftToRight => 1,
            TextDirection::RightToLeft => 2,
            TextDirection::VerticalRightToLeft => 3,
            TextDirection::VerticalLeftToRight => 4,
        },
        match constraint.wrap {
            WrapPolicy::None => 0,
            WrapPolicy::Glyph => 1,
            WrapPolicy::Word => 2,
            WrapPolicy::WordOrGlyph => 3,
        },
        match constraint.overflow {
            OverflowPolicy::Visible => 0,
            OverflowPolicy::Clip => 1,
            OverflowPolicy::EllipsisStart => 2,
            OverflowPolicy::EllipsisMiddle => 3,
            OverflowPolicy::EllipsisEnd => 4,
        },
        constraint.font_size,
        constraint.line_height,
        constraint.max_width,
        constraint.max_height.unwrap_or(0.0),
        constraint.max_lines.unwrap_or(0) as i32,
    )?;
    Ok(RawLayout { lines, overflowed })
}
