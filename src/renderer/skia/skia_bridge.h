#ifndef ZANT_SKIA_BRIDGE_H
#define ZANT_SKIA_BRIDGE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct ZantRenderer ZantRenderer;
typedef struct ZantCanvas ZantCanvas;
typedef struct ZantTextLayout ZantTextLayout;
typedef struct ZantImage ZantImage;

typedef struct ZantRect {
    float x;
    float y;
    float width;
    float height;
} ZantRect;

typedef struct ZantTextStyle {
    float font_size;
    int weight;       /* 100..900 */
    int align;        /* 0 = left, 1 = center, 2 = right */
    bool wrap;
    float line_height; /* 0 = font default */
    uint32_t color;    /* 0xAARRGGBB */
    int max_lines;     /* 0 = unlimited */
    int family;        /* 0 = sans, 1 = serif, 2 = mono */
} ZantTextStyle;

typedef struct ZantRichSpan {
    const char *text;
    size_t length;
    float font_size;
    int weight;
    uint32_t color;
    int italic;
    int family;
    uint32_t background;
    int decoration; /* bit 0 = underline, bit 1 = line-through */
    /* Inline placeholder (embedded drawing such as inline math). When
     * placeholder_width > 0 the span's text is ignored and a box of
     * width x (ascent + descent) is reserved, its baseline on the text
     * baseline. */
    float placeholder_width;
    float placeholder_ascent;
    float placeholder_descent;
} ZantRichSpan;

typedef struct ZantInkMetrics {
    float advance;
    float ascent;       /* ink extent above the baseline (positive) */
    float descent;      /* ink extent below the baseline (positive) */
    float font_ascent;  /* font ascent (positive) */
    float font_descent; /* font descent (positive) */
    float ink_top;      /* signed ink top relative to the baseline (y down) */
    float ink_bottom;   /* signed ink bottom relative to the baseline (y down) */
} ZantInkMetrics;

typedef struct ZantTextMetrics {
    float width;
    float height;
    float max_intrinsic_width;
    float longest_line;
    float first_baseline;
    int line_count;
} ZantTextMetrics;

/* Global one-time Skia initialization. Safe to call repeatedly. */
void zant_skia_init(void);
/* Writes a whole file; path is UTF-8 (wide APIs on Windows). 1 on success. */
int zant_write_file(const char *path, const void *data, size_t size);

ZantRenderer *zant_renderer_create_raster(int pixel_width, int pixel_height);
/* Optional GPU paths. Return NULL when the backend could not be created;
 * callers fall back to raster.
 * macOS: layer is a CAMetalLayer*. Windows: hwnd is the window's HWND
 * (Direct3D 12 flip-model swap chain, vsync'd present). */
ZantRenderer *zant_renderer_create_metal(void *metal_layer, int pixel_width, int pixel_height);
ZantRenderer *zant_renderer_create_d3d(void *hwnd, int pixel_width, int pixel_height);
void zant_renderer_destroy(ZantRenderer *renderer);
bool zant_renderer_resize(ZantRenderer *renderer, int pixel_width, int pixel_height);
ZantCanvas *zant_renderer_begin(ZantRenderer *renderer, float scale);
void zant_renderer_end(ZantRenderer *renderer);
void zant_renderer_present(ZantRenderer *renderer);
/* Raster pixels in the native byte order (RGBA on macOS, BGRA on Windows). */
void *zant_renderer_pixels(ZantRenderer *renderer, int *width, int *height, int *stride);
/* One raster pixel as premultiplied 0xAARRGGBB (tests). */
uint32_t zant_renderer_pixel_argb(ZantRenderer *renderer, int x, int y);
bool zant_renderer_is_gpu(ZantRenderer *renderer);
const char *zant_renderer_backend_name(ZantRenderer *renderer);
bool zant_renderer_write_png(ZantRenderer *renderer, const char *path);
void zant_renderer_capture_next_png(ZantRenderer *renderer, const char *path);

void zant_canvas_save(ZantCanvas *canvas);
void zant_canvas_restore(ZantCanvas *canvas);
void zant_canvas_translate(ZantCanvas *canvas, float x, float y);
void zant_canvas_scale(ZantCanvas *canvas, float sx, float sy);
/* Offscreen layer composited with `alpha` (0..1) on restore; `bounds` limits it. */
void zant_canvas_save_layer_alpha(ZantCanvas *canvas, float x, float y, float width, float height, float alpha);
void zant_canvas_clip_rect(ZantCanvas *canvas, float x, float y, float width, float height);
void zant_canvas_clear(ZantCanvas *canvas, uint32_t color);
void zant_canvas_fill_rect(ZantCanvas *canvas, float x, float y, float width, float height,
                           uint32_t color);
void zant_canvas_fill_round_rect(ZantCanvas *canvas, float x, float y, float width, float height,
                                 float radius, uint32_t color);
void zant_canvas_shadow_round_rect(ZantCanvas *canvas, float x, float y, float width, float height,
                                   float radius, uint32_t color, float blur);
void zant_canvas_stroke_round_rect(ZantCanvas *canvas, float x, float y, float width, float height,
                                   float radius, uint32_t color, float stroke_width);
void zant_canvas_draw_line(ZantCanvas *canvas, float x0, float y0, float x1, float y1,
                           uint32_t color, float stroke_width);
void zant_canvas_draw_text_layout(ZantCanvas *canvas, ZantTextLayout *layout, float x, float y);

ZantImage *zant_image_load_file(const char *path);
/* Copies premultiplied 32-bit pixels in the native byte order (RGBA on
 * macOS, BGRA on Windows). Thread-safe. */
ZantImage *zant_image_from_pixels(const void *pixels, int width, int height, int stride);
/* Decodes an encoded image (PNG, JPEG, GIF, BMP, ICO) from memory. Thread-safe. */
ZantImage *zant_image_from_encoded(const void *data, size_t size);
/* A second handle to the same pixels; destroy each handle separately. */
ZantImage *zant_image_retain(ZantImage *image);
void zant_image_destroy(ZantImage *image);
int zant_image_width(ZantImage *image);
int zant_image_height(ZantImage *image);
void zant_canvas_draw_image_rect(ZantCanvas *canvas, ZantImage *image, float x, float y,
                                 float width, float height, float radius, int fit);

/* Creates a shaped paragraph. max_width is used for wrapping when wrap is true.
 * The layout object owns the copied UTF-8 text and is immutable after creation. */
ZantTextLayout *zant_rich_text_layout_create(const ZantRichSpan *spans, size_t count,
                                             float max_width, float line_height);
ZantTextLayout *zant_text_layout_create(const char *utf8, size_t length,
                                        const ZantTextStyle *style, float max_width);
void zant_text_layout_destroy(ZantTextLayout *layout);
ZantTextMetrics zant_text_layout_metrics(ZantTextLayout *layout);
/* UTF-8 byte offsets in, logical caret geometry out (relative to the text origin). */
bool zant_text_layout_caret_rect(ZantTextLayout *layout, size_t byte_offset, ZantRect *out_rect);
/* Returns the UTF-8 byte offset closest to a point relative to the text origin. */
size_t zant_text_layout_index_at(ZantTextLayout *layout, float x, float y);
/* Placeholder boxes (in span order) of a rich text layout. */
int zant_text_layout_placeholder_rects(ZantTextLayout *layout, ZantRect *out_rects, int max_rects);

/* Single-line glyph run measurement / drawing with per-character font
 * fallback. family: 0 sans, 1 serif, 2 mono, 3 serif CJK, 4 math (STIX Two
 * Math; Cambria Math on Windows). y is the baseline; scale_y stretches glyphs
 * vertically around it. */
ZantInkMetrics zant_text_measure_ink(const char *utf8, size_t length, float size, int family,
                                     int weight, int italic);
void zant_canvas_draw_simple_text(ZantCanvas *canvas, const char *utf8, size_t length, float x,
                                  float y, float size, int family, int weight, int italic,
                                  uint32_t color, float scale_y);
/* xy holds count points (x0, y0, x1, y1, ...). */
void zant_canvas_fill_path(ZantCanvas *canvas, const float *xy, int count, uint32_t color);
void zant_canvas_stroke_path(ZantCanvas *canvas, const float *xy, int count, uint32_t color,
                             float stroke_width, int closed);

/* Returns up to max_rects selection rectangles for the UTF-8 byte range. */
int zant_text_layout_range_rects(ZantTextLayout *layout, size_t start_byte, size_t end_byte,
                                 ZantRect *out_rects, int max_rects);

#ifdef __cplusplus
}
#endif

#endif
