#include <pango/pangoft2.h>
#include <pango/pangofc-fontmap.h>
#include <pango/pangofc-font.h>
#include <fontconfig/fontconfig.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>

typedef struct { FcConfig *config; PangoFontMap *map; char **paths; int *indices; int count; } RkFonts;
typedef struct { int start, end; const char *family; } RkSpan;
typedef struct { int start, end; } RkRange;
typedef struct { int start, end, rtl; float top, baseline, width, height; } RkLine;
typedef struct {
    int start, end, font, glyph, rtl, gravity, probe;
    float x, y, advance, size;
    int left, top, width, height;
    const unsigned char *pixels;
} RkGlyph;
typedef void (*LineFn)(void *, const RkLine *);
typedef int (*GlyphFn)(void *, const RkGlyph *);

void rk_fonts_free(RkFonts *fonts) {
    if (!fonts) return;
    if (fonts->map) g_object_unref(fonts->map);
    if (fonts->config) FcConfigDestroy(fonts->config);
    for (int i = 0; i < fonts->count; ++i) free(fonts->paths[i]);
    free(fonts->paths); free(fonts->indices); free(fonts);
}

RkFonts *rk_fonts_new(const char *const *paths, const int *indices, int count) {
    RkFonts *fonts = calloc(1, sizeof(*fonts));
    if (!fonts) return NULL;
    fonts->config = FcConfigCreate();
    fonts->paths = calloc((size_t)count, sizeof(char *));
    fonts->indices = calloc((size_t)count, sizeof(int));
    if (!fonts->config || !fonts->paths || !fonts->indices) { rk_fonts_free(fonts); return NULL; }
    fonts->count = count;
    for (int i = 0; i < count; ++i) {
        size_t len = strlen(paths[i]) + 1;
        fonts->paths[i] = malloc(len);
        if (!fonts->paths[i]) { rk_fonts_free(fonts); return NULL; }
        memcpy(fonts->paths[i], paths[i], len);
        fonts->indices[i] = indices[i];
        if (!FcConfigAppFontAddFile(fonts->config, (const FcChar8 *)paths[i])) { rk_fonts_free(fonts); return NULL; }
    }
    /* No system config or global font map: only selected project faces exist. */
    FcFontSet *set = FcConfigGetFonts(fonts->config, FcSetApplication);
    for (int i = set->nfont - 1; i >= 0; --i) {
        FcChar8 *file = NULL; int index = -1; int selected = 0;
        FcPatternGetString(set->fonts[i], FC_FILE, 0, &file);
        FcPatternGetInteger(set->fonts[i], FC_INDEX, 0, &index);
        for (int j = 0; j < count; ++j)
            if (file && !strcmp((const char *)file, paths[j]) && index == indices[j]) selected = 1;
        if (!selected) {
            FcPatternDestroy(set->fonts[i]);
            memmove(&set->fonts[i], &set->fonts[i+1], (size_t)(set->nfont-i-1)*sizeof(FcPattern *));
            --set->nfont;
        }
    }
    fonts->map = pango_ft2_font_map_new();
    if (!fonts->map) { rk_fonts_free(fonts); return NULL; }
    pango_fc_font_map_set_config(PANGO_FC_FONT_MAP(fonts->map), fonts->config);
    pango_ft2_font_map_set_resolution(PANGO_FT2_FONT_MAP(fonts->map), 72, 72);
    return fonts;
}

int rk_shape(RkFonts *fonts, const char *text, int length, const char *locale,
             const char *features, const RkSpan *spans, int nspans,
             const RkRange *keep, int nkeep, int direction, int wrap, int overflow,
             float size, float line_height, float width, float height, int max_lines,
             void *output, LineFn line_fn, GlyphFn glyph_fn) {
    PangoContext *context = pango_font_map_create_context(fonts->map);
    pango_context_set_language(context, pango_language_from_string(locale));
    pango_context_set_base_dir(context, direction == 2 ? PANGO_DIRECTION_RTL : PANGO_DIRECTION_LTR);
    pango_context_set_base_gravity(context, direction >= 3 ? PANGO_GRAVITY_EAST : PANGO_GRAVITY_SOUTH);
    pango_context_set_gravity_hint(context, PANGO_GRAVITY_HINT_NATURAL);
    PangoLayout *layout = pango_layout_new(context);
    pango_layout_set_auto_dir(layout, TRUE);
    PangoFontDescription *desc = pango_font_description_new();
    pango_font_description_set_absolute_size(desc, size * PANGO_SCALE);
    pango_layout_set_font_description(layout, desc);
    pango_font_description_free(desc);
    pango_layout_set_text(layout, text, length);
    PangoAttrList *attrs = pango_attr_list_new();
    PangoAttribute *attr = pango_attr_font_features_new(features);
    pango_attr_list_insert(attrs, attr);
    pango_attr_list_insert(attrs, pango_attr_fallback_new(FALSE));
    if (direction >= 3) {
        for (int i = 0; i < length;) {
            int start = i;
            while (i < length && text[i] >= '0' && text[i] <= '9') ++i;
            if (i - start >= 2 && i - start <= 4) {
                attr = pango_attr_gravity_new(PANGO_GRAVITY_SOUTH);
                attr->start_index = (guint)start; attr->end_index = (guint)i;
                pango_attr_list_insert(attrs, attr);
            }
            if (start == i) ++i;
        }
    }
    for (int i = 0; i < nspans; ++i) {
        attr = pango_attr_family_new(spans[i].family);
        attr->start_index = (guint)spans[i].start; attr->end_index = (guint)spans[i].end;
        pango_attr_list_insert(attrs, attr);
    }
    for (int i = 0; i < nkeep; ++i) {
        attr = pango_attr_allow_breaks_new(FALSE);
        attr->start_index = (guint)keep[i].start; attr->end_index = (guint)keep[i].end;
        pango_attr_list_insert(attrs, attr);
    }
    attr = pango_attr_line_height_new_absolute((int)roundf(line_height * PANGO_SCALE));
    pango_attr_list_insert(attrs, attr);
    pango_layout_set_attributes(layout, attrs);
    pango_attr_list_unref(attrs);
    pango_layout_set_width(layout, wrap == 0 ? -1 : (int)roundf(width * PANGO_SCALE));
    pango_layout_set_wrap(layout, wrap == 1 ? PANGO_WRAP_CHAR : wrap == 2 ? PANGO_WRAP_WORD : PANGO_WRAP_WORD_CHAR);
    PangoRectangle logical;
    pango_layout_get_extents(layout, NULL, &logical);
    int count = pango_layout_get_line_count(layout);
    int clipped = (wrap == 0 && logical.width > width * PANGO_SCALE)
        || (max_lines > 0 && count > max_lines)
        || (height > 0 && logical.height > height * PANGO_SCALE);
    if (overflow >= 2) {
        pango_layout_set_width(layout, (int)roundf(width * PANGO_SCALE));
        pango_layout_set_ellipsize(layout, overflow == 2 ? PANGO_ELLIPSIZE_START : overflow == 3 ? PANGO_ELLIPSIZE_MIDDLE : PANGO_ELLIPSIZE_END);
        pango_layout_set_height(layout, max_lines > 0 ? -max_lines : height > 0 ? (int)roundf(height * PANGO_SCALE) : -1);
    }
    PangoLayoutIter *iter = pango_layout_get_iter(layout);
    int status = clipped;
    int line_index = 0;
    do {
        PangoLayoutLine *line = pango_layout_iter_get_line_readonly(iter);
        PangoRectangle rect;
        pango_layout_iter_get_line_extents(iter, NULL, &rect);
        float baseline = (float)pango_layout_iter_get_baseline(iter) / PANGO_SCALE;
        if (overflow == 1 && ((max_lines > 0 && line_index >= max_lines) || (height > 0 && rect.y >= height * PANGO_SCALE))) break;
        ++line_index;
        RkLine out_line = {line->start_index, line->start_index + line->length,
            line->resolved_dir == PANGO_DIRECTION_RTL,
            (float)rect.y / PANGO_SCALE, baseline, (float)rect.width / PANGO_SCALE, (float)rect.height / PANGO_SCALE};
        line_fn(output, &out_line);
        float pen = (float)rect.x / PANGO_SCALE;
        for (GSList *node = line->runs; node; node = node->next) {
            PangoGlyphItem *run = node->data;
            PangoGlyphString *glyphs = run->glyphs;
            PangoFont *font = run->item->analysis.font;
            FcPattern *pattern = pango_fc_font_get_pattern(PANGO_FC_FONT(font));
            FcChar8 *file = NULL; int index = -1; int selected = -1;
            FcPatternGetString(pattern, FC_FILE, 0, &file);
            FcPatternGetInteger(pattern, FC_INDEX, 0, &index);
            for (int j = 0; j < fonts->count; ++j)
                if (file && !strcmp((const char *)file, fonts->paths[j]) && index == fonts->indices[j]) { selected = j; break; }
            if (selected < 0) { status = -1; goto done; }
            int last_cluster = -1, cluster_end = run->item->length;
            for (int i = 0; i < glyphs->num_glyphs; ++i) {
                PangoGlyphInfo info = glyphs->glyphs[i];
                if ((info.glyph & PANGO_GLYPH_UNKNOWN_FLAG) ||
                    (info.glyph > 0xffff && info.glyph != PANGO_GLYPH_EMPTY)) { status = -3; goto done; }
                int start = glyphs->log_clusters[i];
                if (start != last_cluster) {
                    if (run->item->analysis.level & 1) cluster_end = last_cluster < 0 ? run->item->length : last_cluster;
                    else {
                        cluster_end = run->item->length;
                        int next = i + 1;
                        while (next < glyphs->num_glyphs && glyphs->log_clusters[next] == start) ++next;
                        if (next < glyphs->num_glyphs) cluster_end = glyphs->log_clusters[next];
                    }
                    last_cluster = start;
                }
                int end = cluster_end;
                RkGlyph out = {run->item->offset + start, run->item->offset + end, selected,
                    info.glyph == PANGO_GLYPH_EMPTY ? 0xffff : (int)info.glyph, run->item->analysis.level & 1,
                    run->item->analysis.gravity, 1,
                    pen + (float)info.geometry.x_offset / PANGO_SCALE,
                    -(float)info.geometry.y_offset / PANGO_SCALE,
                    (float)info.geometry.width / PANGO_SCALE, size, 0, 0, 0, 0, NULL};
                if (glyph_fn(output, &out)) {
                    pen += (float)info.geometry.width / PANGO_SCALE;
                    continue;
                }
                out.probe = 0;
                PangoRectangle ink;
                pango_font_get_glyph_extents(font, info.glyph, &ink, NULL);
                int left = (int)floor((double)ink.x / PANGO_SCALE);
                int top = (int)floor((double)ink.y / PANGO_SCALE);
                int right = (int)ceil((double)(ink.x + ink.width) / PANGO_SCALE);
                int bottom = (int)ceil((double)(ink.y + ink.height) / PANGO_SCALE);
                out.left = left; out.top = top; out.width = right-left; out.height = bottom-top;
                if (out.width > 0 && out.height > 0 && info.glyph != PANGO_GLYPH_EMPTY) {
                    FT_Bitmap bitmap;
                    memset(&bitmap, 0, sizeof(bitmap));
                    bitmap.width = (unsigned)out.width; bitmap.rows = (unsigned)out.height;
                    bitmap.pitch = out.width; bitmap.pixel_mode = FT_PIXEL_MODE_GRAY; bitmap.num_grays = 256;
                    bitmap.buffer = calloc((size_t)out.width, (size_t)out.height);
                    if (!bitmap.buffer) { status = -2; goto done; }
                    PangoGlyphString single = {1, &info, &start, 1};
                    single.glyphs->geometry.x_offset = 0; single.glyphs->geometry.y_offset = 0;
                    pango_ft2_render(&bitmap, font, &single, -left, -top);
                    out.pixels = bitmap.buffer;
                    glyph_fn(output, &out);
                    free(bitmap.buffer);
                } else { glyph_fn(output, &out); }
                pen += (float)info.geometry.width / PANGO_SCALE;
            }
        }
    } while (pango_layout_iter_next_line(iter));
done:
    pango_layout_iter_free(iter);
    g_object_unref(layout); g_object_unref(context);
    return status;
}
