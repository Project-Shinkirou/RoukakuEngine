use crate::provider::RawLine;
use crate::{MediaError, PackagedFont, SourceRange};
use roukaku_media_core::{GlyphBitmap, GlyphBitmapFormat};
use std::{
    collections::BTreeMap,
    ffi::{CString, c_char, c_int, c_void},
    ptr::NonNull,
};

#[repr(C)]
pub(super) struct NativeSpan {
    pub start: c_int,
    pub end: c_int,
    pub family: *const c_char,
}
#[repr(C)]
pub(super) struct NativeRange {
    pub start: c_int,
    pub end: c_int,
}
#[repr(C)]
struct NativeLine {
    start: c_int,
    end: c_int,
    rtl: c_int,
    top: f32,
    baseline: f32,
    width: f32,
    height: f32,
}
#[repr(C)]
struct NativeGlyph {
    start: c_int,
    end: c_int,
    font: c_int,
    glyph: c_int,
    rtl: c_int,
    gravity: c_int,
    probe: c_int,
    x: f32,
    y: f32,
    advance: f32,
    size: f32,
    left: c_int,
    top: c_int,
    width: c_int,
    height: c_int,
    pixels: *const u8,
}

pub(super) struct RawGlyph {
    pub start: usize,
    pub end: usize,
    pub font_id: String,
    pub glyph_id: u16,
    pub rtl: bool,
    pub gravity: c_int,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub size: f32,
    pub left: i32,
    pub top: i32,
    pub bitmap: Option<GlyphBitmap>,
}

unsafe extern "C" {
    fn rk_fonts_new(
        paths: *const *const c_char,
        indices: *const c_int,
        count: c_int,
    ) -> *mut c_void;
    fn rk_fonts_free(fonts: *mut c_void);
    fn rk_shape(
        fonts: *mut c_void,
        text: *const c_char,
        length: c_int,
        locale: *const c_char,
        features: *const c_char,
        spans: *const NativeSpan,
        nspans: c_int,
        keep: *const NativeRange,
        nkeep: c_int,
        direction: c_int,
        wrap: c_int,
        overflow: c_int,
        size: f32,
        line_height: f32,
        width: f32,
        height: f32,
        max_lines: c_int,
        output: *mut c_void,
        line_fn: unsafe extern "C" fn(*mut c_void, *const NativeLine),
        glyph_fn: unsafe extern "C" fn(*mut c_void, *const NativeGlyph) -> c_int,
    ) -> c_int;
}

pub(super) struct NativeFonts {
    handle: NonNull<c_void>,
    // Pango/Fontconfig reopen files lazily. The directory outlives the font map.
    _directory: tempfile::TempDir,
    ids: Vec<String>,
    rasters: BTreeMap<RasterKey, Raster>,
    max_rasters: usize,
}
// Each map is exclusively borrowed under its worker mutex; no GObject crosses workers.
unsafe impl Send for NativeFonts {}
impl Drop for NativeFonts {
    fn drop(&mut self) {
        unsafe { rk_fonts_free(self.handle.as_ptr()) }
    }
}

pub(super) fn cstring(value: &str) -> Result<CString, MediaError> {
    CString::new(value).map_err(|_| MediaError::message("ROUKAKU_TEXT_NATIVE_STRING: embedded NUL"))
}

impl NativeFonts {
    pub fn new(
        fonts: &[PackagedFont],
        ids: Vec<String>,
        max_rasters: usize,
    ) -> Result<Self, MediaError> {
        let directory = tempfile::tempdir().map_err(|e| MediaError::message(e.to_string()))?;
        let mut paths = Vec::with_capacity(fonts.len());
        let mut indices = Vec::with_capacity(fonts.len());
        for (index, font) in fonts.iter().enumerate() {
            let path = directory.path().join(format!("{index}.font"));
            std::fs::write(&path, &font.bytes).map_err(|e| MediaError::message(e.to_string()))?;
            paths.push(cstring(&path.to_string_lossy().replace('\\', "/"))?);
            indices.push(
                c_int::try_from(font.face_index).map_err(|e| MediaError::message(e.to_string()))?,
            );
        }
        let ptrs = paths.iter().map(|p| p.as_ptr()).collect::<Vec<_>>();
        let handle = NonNull::new(unsafe {
            rk_fonts_new(ptrs.as_ptr(), indices.as_ptr(), fonts.len() as c_int)
        })
        .ok_or_else(|| {
            MediaError::message("ROUKAKU_TEXT_FONT_MAP: Pango project font map creation failed")
        })?;
        Ok(Self {
            handle,
            _directory: directory,
            ids,
            rasters: BTreeMap::new(),
            max_rasters,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn shape(
        &mut self,
        text: &str,
        locale: &str,
        features: &str,
        spans: &[NativeSpan],
        keep: &[NativeRange],
        direction: c_int,
        wrap: c_int,
        overflow: c_int,
        size: f32,
        line_height: f32,
        width: f32,
        height: f32,
        max_lines: c_int,
    ) -> Result<(Vec<RawLine>, bool), MediaError> {
        let length = c_int::try_from(text.len()).map_err(|e| MediaError::message(e.to_string()))?;
        let locale = cstring(locale)?;
        let features = cstring(features)?;
        let mut output = Output {
            lines: Vec::new(),
            ids: &self.ids,
            rasters: &mut self.rasters,
            max_rasters: self.max_rasters,
        };
        let status = unsafe {
            rk_shape(
                self.handle.as_ptr(),
                text.as_ptr().cast(),
                length,
                locale.as_ptr(),
                features.as_ptr(),
                spans.as_ptr(),
                spans.len() as c_int,
                keep.as_ptr(),
                keep.len() as c_int,
                direction,
                wrap,
                overflow,
                size,
                line_height,
                width,
                height,
                max_lines,
                (&mut output as *mut Output<'_>).cast(),
                collect_line,
                collect_glyph,
            )
        };
        if status < 0 {
            return Err(MediaError::message(format!(
                "ROUKAKU_TEXT_NATIVE_LAYOUT: Pango failed ({status})"
            )));
        }
        Ok((output.lines, status != 0))
    }
}

type RasterKey = (c_int, u16, u32, c_int);
#[derive(Clone)]
struct Raster {
    left: i32,
    top: i32,
    bitmap: Option<GlyphBitmap>,
}

struct Output<'a> {
    lines: Vec<RawLine>,
    ids: &'a [String],
    rasters: &'a mut BTreeMap<RasterKey, Raster>,
    max_rasters: usize,
}
unsafe extern "C" fn collect_line(output: *mut c_void, line: *const NativeLine) {
    let output = unsafe { &mut *output.cast::<Output<'_>>() };
    let line = unsafe { &*line };
    output.lines.push(RawLine {
        source: SourceRange {
            start: line.start as usize,
            end: line.end as usize,
        },
        rtl: line.rtl != 0,
        top: line.top,
        baseline: line.baseline,
        width: line.width,
        height: line.height,
        glyphs: Vec::new(),
        source_offset: 0,
    });
}
unsafe extern "C" fn collect_glyph(output: *mut c_void, glyph: *const NativeGlyph) -> c_int {
    let output = unsafe { &mut *output.cast::<Output<'_>>() };
    let glyph = unsafe { &*glyph };
    let key = (
        glyph.font,
        glyph.glyph as u16,
        glyph.size.to_bits(),
        glyph.gravity,
    );
    let raster = if glyph.probe != 0 {
        let Some(raster) = output.rasters.get(&key) else {
            return 0;
        };
        raster.clone()
    } else {
        let bitmap = if glyph.pixels.is_null() {
            None
        } else {
            Some(GlyphBitmap {
                width: glyph.width as u32,
                height: glyph.height as u32,
                format: GlyphBitmapFormat::Alpha8,
                pixels: unsafe {
                    std::slice::from_raw_parts(
                        glyph.pixels,
                        glyph.width as usize * glyph.height as usize,
                    )
                }
                .to_vec()
                .into(),
            })
        };
        let raster = Raster {
            left: glyph.left,
            top: glyph.top,
            bitmap,
        };
        if output.rasters.len() == output.max_rasters {
            output.rasters.clear();
        }
        output.rasters.insert(key, raster.clone());
        raster
    };
    output
        .lines
        .last_mut()
        .expect("Pango emits a line before its glyphs")
        .glyphs
        .push(RawGlyph {
            start: glyph.start as usize,
            end: glyph.end as usize,
            font_id: output.ids[glyph.font as usize].clone(),
            glyph_id: glyph.glyph as u16,
            rtl: glyph.rtl != 0,
            gravity: glyph.gravity,
            x: glyph.x,
            y: glyph.y,
            w: glyph.advance,
            size: glyph.size,
            left: raster.left,
            top: raster.top,
            bitmap: raster.bitmap,
        });
    1
}
