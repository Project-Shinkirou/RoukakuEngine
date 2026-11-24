use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use cosmic_text::{CacheKey, FontSystem, Placement, SubpixelBin, SwashCache, fontdb};
use roukaku_core::Hash256;
use roukaku_media_core::GlyphBitmap;

use crate::{GlyphResource, LayoutClip, LogicalTextLayout, MediaError, ShapedGlyph, TextLayoutResult};

use super::{logical::RasterGlyph, shaping::glyph_bitmap, vertical::is_vertical};

// Bound retained masks independently of the layout LRU: changing viewport
// density can otherwise accumulate a new set of masks for every resize.
const MAX_MASK_CACHE_BYTES: usize = 64 * 1024 * 1024;
const MAX_MASK_CACHE_ENTRIES: usize = 16_384;
// Reject oversized em sizes before asking Swash to allocate a bitmap. The
// renderer's per-glyph bitmap dimension limit is also 4096 pixels.
const MAX_PHYSICAL_EM: f32 = 4096.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RasterLayoutKey {
    revision: u64,
    scale: u32,
    offset: [u32; 2],
}

impl RasterLayoutKey {
    pub(super) fn new(revision: u64, scale: f32, offset: (f32, f32)) -> Result<Self, MediaError> {
        if !scale.is_finite() || scale <= 0.0 {
            return Err(raster_size_error());
        }
        quantize(offset.0)?;
        quantize(offset.1)?;
        let offset_bits = |v: f32| if v == 0.0 { 0 } else { v.to_bits() };
        Ok(Self {
            revision,
            scale: scale.to_bits(),
            offset: [offset_bits(offset.0), offset_bits(offset.1)],
        })
    }
}

struct CachedLayout {
    layout: Arc<TextLayoutResult>,
    last_access: u64,
}

struct Mask {
    placement: Placement,
    bitmap: GlyphBitmap,
}

enum RasterImage {
    Missing,
    Empty,
    Mask(Mask),
}

struct RasterPlacement {
    key: CacheKey,
    x: i32,
    y: i32,
}

pub(super) struct Rasterizer {
    generation: u64,
    font_system: FontSystem,
    swash: SwashCache,
    masks: HashMap<CacheKey, RasterImage>,
    mask_bytes: usize,
    font_namespaces: HashMap<fontdb::ID, Hash256>,
    layouts: BTreeMap<RasterLayoutKey, CachedLayout>,
    access: u64,
}

impl Rasterizer {
    pub(super) fn new(generation: u64, locale: &str, database: fontdb::Database) -> Self {
        Self {
            generation,
            font_system: FontSystem::new_with_locale_and_db(locale.to_owned(), database),
            swash: SwashCache::new(),
            masks: HashMap::new(),
            mask_bytes: 0,
            layouts: BTreeMap::new(),
            font_namespaces: HashMap::new(),
            access: 0,
        }
    }

    pub(super) fn clear_layouts(&mut self) {
        self.layouts.clear();
        self.access = 0;
    }

    pub(super) fn cached(
        &mut self,
        key: RasterLayoutKey,
        generation: u64,
    ) -> Result<Option<Arc<TextLayoutResult>>, MediaError> {
        if self.generation != generation {
            return Err(MediaError::message(
                "ROUKAKU_TEXT_RASTER_LAYOUT: logical layout predates the current font set",
            ));
        }
        let Some(entry) = self.layouts.get_mut(&key) else {
            return Ok(None);
        };
        self.access += 1;
        entry.last_access = self.access;
        Ok(Some(Arc::clone(&entry.layout)))
    }

    pub(super) fn prepare(
        &mut self,
        logical: &LogicalTextLayout,
        key: RasterLayoutKey,
        max_layouts: usize,
    ) -> Result<Arc<TextLayoutResult>, MediaError> {
        if let Some(layout) = self.cached(key, logical.generation)? {
            return Ok(layout);
        }
        let layout = Arc::new(self.rasterize(logical, key)?);
        if self.layouts.len() == max_layouts {
            let oldest = self
                .layouts
                .iter()
                .min_by_key(|(_, entry)| entry.last_access)
                .map(|(key, _)| *key)
                .expect("full raster layout cache is non-empty");
            self.layouts.remove(&oldest);
        }
        self.access += 1;
        self.layouts.insert(
            key,
            CachedLayout {
                layout: Arc::clone(&layout),
                last_access: self.access,
            },
        );
        Ok(layout)
    }

    fn rasterize(
        &mut self,
        logical: &LogicalTextLayout,
        key: RasterLayoutKey,
    ) -> Result<TextLayoutResult, MediaError> {
        let scale = f32::from_bits(key.scale);
        let offset = key.offset.map(f32::from_bits);
        let mut layout = logical.shaped.layout.clone();
        layout.clip = logical
            .shaped
            .clip_extent
            .map(|extent| physical_clip(extent, scale, offset))
            .transpose()?;
        let mut resources = BTreeMap::<CacheKey, GlyphResource>::new();
        for (glyph, spec) in layout
            .shaped_runs
            .iter_mut()
            .flat_map(|run| &mut run.glyphs)
            .zip(&logical.shaped.raster_glyphs)
        {
            let Some(placement) = self.raster_glyph(glyph, *spec, scale, offset)? else {
                continue;
            };
            let key = placement.key;
            let resource = resources.entry(key).or_insert_with(|| {
                let font_namespace = self.font_namespaces.entry(key.font_id).or_insert_with(|| {
                    let font_identity = format!("{}/{}", glyph.font_asset_id, glyph.font_hash);
                    Hash256::from_sha256(font_identity.as_bytes())
                });
                let RasterImage::Mask(mask) = &self.masks[&key] else {
                    unreachable!("a placed glyph has a cached mask")
                };
                GlyphResource {
                    resource_id: format!(
                        "glyph:{}:{}:{}:{}:{}:{}:{}:{}",
                        font_namespace,
                        glyph.font_face_index,
                        key.glyph_id,
                        key.font_size_bits,
                        key.x_bin as u8,
                        key.y_bin as u8,
                        key.font_weight.0,
                        key.flags.bits(),
                    ),
                    font_asset_id: glyph.font_asset_id.clone(),
                    font_hash: glyph.font_hash,
                    glyph_id: glyph.glyph_id,
                    bitmap: mask.bitmap.clone(),
                }
            });
            glyph.resource_id = Some(resource.resource_id.clone());
            glyph.render_x = Some(placement.x);
            glyph.render_y = Some(placement.y);
        }
        layout.glyph_resources = resources.into_values().collect();
        Ok(layout)
    }

    fn raster_glyph(
        &mut self,
        glyph: &ShapedGlyph,
        spec: RasterGlyph,
        scale: f32,
        offset: [f32; 2],
    ) -> Result<Option<RasterPlacement>, MediaError> {
        let size = spec.size * scale;
        if !size.is_finite() || size <= 0.0 || size > MAX_PHYSICAL_EM {
            return Err(raster_size_error());
        }
        let vertical = is_vertical(glyph.direction);
        let (x, y) = if vertical {
            (glyph.x, glyph.y)
        } else {
            (glyph.x + spec.x_offset, glyph.y + spec.y_offset)
        };
        let (x, x_phase) = quantize(x.mul_add(scale, offset[0]))?;
        let (y, y_phase) = quantize(y.mul_add(scale, offset[1]))?;
        let rotated = vertical && glyph.rotation_quadrants == 1;
        let fx = x_phase.as_float();
        let fy = y_phase.as_float();
        // Map screen phase to the unrotated outline; Swash's Y points up.
        let local = if rotated { (fy, fx) } else { (fx, -fy) };
        let (key, dx, dy_up) =
            CacheKey::new(spec.font, glyph.glyph_id, size, local, spec.weight, spec.flags);
        let zero = if vertical && (fx != 0.0 || fy != 0.0) {
            let zero = CacheKey {
                x_bin: SubpixelBin::Zero,
                y_bin: SubpixelBin::Zero,
                ..key
            };
            match self.image(zero)? {
                RasterImage::Mask(mask) => Some(mask.placement),
                _ => None,
            }
        } else {
            None
        };
        let mask = match self.image(key)? {
            RasterImage::Mask(mask) => &mask.placement,
            RasterImage::Empty if spec.invisible || spec.zero_advance => return Ok(None),
            RasterImage::Missing if spec.invisible => return Ok(None),
            _ => return Err(MediaError::message(
                "ROUKAKU_TEXT_GLYPH_RASTER: covered glyph could not be rasterized",
            )),
        };
        let left = i64::from(dx) + i64::from(mask.left);
        let top = -i64::from(dy_up) - i64::from(mask.top);
        let (left, top) = if let Some(zero) = zero {
            // Vertical cells anchor the tight bitmap, not the baseline.
            // Apply only the phase-induced bearing change to that anchor.
            let dx = left - i64::from(zero.left);
            let dy = top + i64::from(zero.top);
            if rotated {
                (i64::from(zero.height) - i64::from(mask.height) - dy, dx)
            } else {
                (dx, dy)
            }
        } else if vertical {
            (0, 0)
        } else {
            (left, top)
        };
        Ok(Some(RasterPlacement {
            key,
            x: i32::try_from(i64::from(x) + left).map_err(|_| raster_bounds_error())?,
            y: i32::try_from(i64::from(y) + top).map_err(|_| raster_bounds_error())?,
        }))
    }

    fn image(&mut self, key: CacheKey) -> Result<&RasterImage, MediaError> {
        if !self.masks.contains_key(&key) {
            let image = match self.swash.get_image_uncached(&mut self.font_system, key) {
                Some(image)
                    if image.placement.width == 0
                        || image.placement.height == 0
                        || image.data.is_empty() =>
                {
                    RasterImage::Empty
                }
                Some(image) => RasterImage::Mask(Mask {
                    placement: image.placement,
                    bitmap: glyph_bitmap(image)?,
                }),
                None => RasterImage::Missing,
            };
            let bytes = match &image {
                RasterImage::Mask(mask) => mask.bitmap.pixels.len(),
                _ => 0,
            };
            if self.mask_bytes + bytes > MAX_MASK_CACHE_BYTES
                || self.masks.len() == MAX_MASK_CACHE_ENTRIES
            {
                self.masks.clear();
                self.mask_bytes = 0;
            }
            self.mask_bytes += bytes;
            self.masks.insert(key, image);
        }
        Ok(&self.masks[&key])
    }
}

fn quantize(value: f32) -> Result<(i32, SubpixelBin), MediaError> {
    // Floor first so negative and positive origins use the same bin thresholds
    // and translating by an integer does not change a glyph's cache key.
    let whole = value.floor();
    let base = checked_origin(f64::from(whole))?;
    let (carry, phase) = SubpixelBin::new(value - whole);
    Ok((base.checked_add(carry).ok_or_else(raster_bounds_error)?, phase))
}

fn physical_clip(extent: (f32, f32), scale: f32, offset: [f32; 2]) -> Result<LayoutClip, MediaError> {
    let left = f64::from(offset[0]);
    let top = f64::from(offset[1]);
    let scale = f64::from(scale);
    let x = checked_origin(left.floor())?;
    let y = checked_origin(top.floor())?;
    let width = clip_length(left, f64::from(extent.0) * scale)?;
    let height = clip_length(top, f64::from(extent.1) * scale)?;
    Ok(LayoutClip { x, y, width, height })
}

fn checked_origin(value: f64) -> Result<i32, MediaError> {
    if !(f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&value) {
        return Err(raster_bounds_error());
    }
    Ok(value as i32)
}

fn clip_length(start: f64, length: f64) -> Result<u32, MediaError> {
    let length = ((start + length).ceil() - start.floor()).max(1.0);
    if length > f64::from(u32::MAX) {
        return Err(raster_bounds_error());
    }
    Ok(length as u32)
}

fn raster_bounds_error() -> MediaError {
    MediaError::message("ROUKAKU_TEXT_RASTER_BOUNDS: physical text exceeds renderer coordinates")
}

fn raster_size_error() -> MediaError {
    MediaError::message("ROUKAKU_TEXT_RASTER_SCALE: density or physical font size is out of range")
}
