use std::{
    ops::Deref,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use cosmic_text::{CacheKeyFlags, fontdb};

use crate::{GlyphRole, MediaError, ShapedGlyph, TextLayoutResult};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphPlacement {
    pub x: f32,
    pub y: f32,
    pub advance: f32,
}

impl From<&ShapedGlyph> for GlyphPlacement {
    fn from(glyph: &ShapedGlyph) -> Self {
        Self {
            x: glyph.x,
            y: glyph.y,
            advance: glyph.advance,
        }
    }
}

/// Logical text metrics and shaped clusters, with no renderer resources.
///
/// Pass this layout back to the provider that shaped it to obtain physical
/// glyph bitmaps. It becomes stale when that provider's font set changes.
#[derive(Clone, Debug)]
pub struct LogicalTextLayout {
    pub(super) shaped: ShapedText,
    pub(super) domain: Arc<AtomicU64>,
    pub(super) generation: u64,
}

#[derive(Clone, Debug)]
pub(super) struct ShapedText {
    pub(super) layout: TextLayoutResult,
    // One entry per glyph, in flattened shaped run order.
    pub(super) raster_glyphs: Vec<RasterGlyph>,
    pub(super) clip_extent: Option<(f32, f32)>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct RasterGlyph {
    pub(super) font: fontdb::ID,
    pub(super) size: f32,
    pub(super) weight: fontdb::Weight,
    pub(super) flags: CacheKeyFlags,
    pub(super) x_offset: f32,
    pub(super) y_offset: f32,
    pub(super) invisible: bool,
    pub(super) zero_advance: bool,
}

impl Deref for LogicalTextLayout {
    type Target = TextLayoutResult;

    fn deref(&self) -> &Self::Target {
        &self.shaped.layout
    }
}

impl LogicalTextLayout {
    /// Position or hide whole shaped glyphs in logical units before rasterizing.
    /// Return `None` to omit a glyph from the draw (for example, during reveal).
    /// This preserves the original lines, cluster source ranges and ruby boxes.
    /// Position deltas carry the glyph's shaping offsets and baseline with it.
    pub fn place_glyphs(
        &mut self,
        mut place: impl FnMut(usize, &GlyphRole, &ShapedGlyph) -> Option<GlyphPlacement>,
    ) -> Result<(), MediaError> {
        let mut changed = false;
        let mut placements = Vec::with_capacity(self.shaped.raster_glyphs.len());
        for run in &self.shaped.layout.shaped_runs {
            for glyph in &run.glyphs {
                let placement = place(run.run_index, &run.role, glyph);
                if let Some(placement) = placement
                    && (!placement.x.is_finite()
                        || !placement.y.is_finite()
                        || !placement.advance.is_finite())
                {
                    return Err(MediaError::message(
                        "ROUKAKU_TEXT_GLYPH_PLACEMENT: position and advance must be finite",
                    ));
                }
                changed |= placement != Some(GlyphPlacement::from(glyph));
                placements.push(placement);
            }
        }
        if !changed {
            return Ok(());
        }
        self.shaped.layout.revision = next_revision(&self.domain)?;
        let mut placements_iter = placements.iter();
        self.shaped.layout.shaped_runs.retain_mut(|run| {
            run.glyphs.retain_mut(|glyph| {
                let Some(placement) = *placements_iter.next().expect("one placement per glyph")
                else {
                    return false;
                };
                glyph.baseline += placement.y - glyph.y;
                glyph.x = placement.x;
                glyph.y = placement.y;
                glyph.advance = placement.advance;
                true
            });
            !run.glyphs.is_empty()
        });
        let mut placements_iter = placements.into_iter();
        self.shaped
            .raster_glyphs
            .retain(|_| placements_iter.next().expect("one placement per glyph").is_some());
        Ok(())
    }
}

pub(super) fn next_revision(domain: &AtomicU64) -> Result<u64, MediaError> {
    domain
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map_err(|_| {
            MediaError::message("ROUKAKU_TEXT_LAYOUT_REVISION: layout revision overflowed")
        })
}
