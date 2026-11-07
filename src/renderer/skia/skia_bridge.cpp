// Portable Skia bridge core: canvas, text (SkParagraph), images, paths and
// the raster renderer. Font manager, font names and the GPU swap chain live
// in skia_bridge_mac.mm / skia_bridge_win.cpp (see skia_bridge_internal.h).
#include "skia_bridge_internal.h"

#include "include/core/SkBitmap.h"
#include "include/core/SkCanvas.h"
#include "include/core/SkColorSpace.h"
#include "include/core/SkData.h"
#include "include/core/SkFont.h"
#include "include/core/SkFontMetrics.h"
#include "include/core/SkFontMgr.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkFontStyle.h"
#include "include/core/SkImageInfo.h"
#include "include/core/SkImage.h"
#include "include/core/SkData.h"
#include "include/core/SkSamplingOptions.h"
#include "include/core/SkPaint.h"
#include "include/core/SkMaskFilter.h"
#include "include/core/SkBlurTypes.h"
#include "include/core/SkPixmap.h"
#include "include/core/SkRRect.h"
#include "include/core/SkStream.h"
#include "include/core/SkSurface.h"
#include "include/encode/SkPngEncoder.h"
#include "modules/skparagraph/include/FontCollection.h"
#include "modules/skparagraph/include/Paragraph.h"
#include "modules/skparagraph/include/ParagraphBuilder.h"
#include "modules/skparagraph/include/ParagraphStyle.h"
#include "modules/skparagraph/include/TextStyle.h"
#include "modules/skunicode/include/SkUnicode_icu.h"
#include "include/gpu/ganesh/GrDirectContext.h"

#include <algorithm>
#include <cmath>
#include <cstdlib>
#include <cstring>
#include <cstdio>
#include <limits>
#include <memory>
#include <string>
#include <string_view>
#include <vector>

#if defined(_WIN32)
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#endif

using namespace skia::textlayout;

namespace {

sk_sp<SkFontMgr> g_font_mgr;
sk_sp<FontCollection> g_font_collection;
sk_sp<SkUnicode> g_unicode;

void ensure_skia() {
    if (g_font_collection) {
        return;
    }
    g_font_mgr = zant_platform::make_font_mgr();
    if (!g_font_mgr) {
        g_font_mgr = SkFontMgr::RefEmpty();
    }
    g_font_collection = sk_make_sp<FontCollection>();
    g_font_collection->setDefaultFontManager(g_font_mgr, zant_platform::default_families());
    g_font_collection->enableFontFallback();
    g_unicode = SkUnicodes::ICU::Make();
}

/* First installed family for a ZantTextStyle family id (cached). */
sk_sp<SkTypeface> family_typeface(int family, const SkFontStyle &style) {
    if (auto face = zant_platform::bundled_typeface(family)) return face;
    for (const char *const *name = zant_platform::family_names(family); name && *name; ++name) {
        if (auto face = g_font_mgr->matchFamilyStyle(*name, style)) return face;
    }
    return nullptr;
}

SkColor make_color(uint32_t argb) {
    return static_cast<SkColor>(argb);
}

SkPaint fill_paint(uint32_t argb) {
    SkPaint paint;
    paint.setAntiAlias(true);
    paint.setColor(make_color(argb));
    paint.setStyle(SkPaint::kFill_Style);
    return paint;
}

SkPaint stroke_paint(uint32_t argb, float width) {
    SkPaint paint;
    paint.setAntiAlias(true);
    paint.setColor(make_color(argb));
    paint.setStyle(SkPaint::kStroke_Style);
    paint.setStrokeWidth(width);
    paint.setStrokeJoin(SkPaint::kRound_Join);
    paint.setStrokeCap(SkPaint::kRound_Cap);
    return paint;
}

SkRRect make_rrect(float x, float y, float w, float h, float radius) {
    const float r = std::max(0.0f, std::min(radius, std::min(w, h) * 0.5f));
    return SkRRect::MakeRectXY(SkRect::MakeXYWH(x, y, w, h), r, r);
}

/* Number of UTF-8 bytes in one code point; invalid bytes consume one byte. */
int utf8_unit_length(unsigned char c) {
    if ((c & 0x80) == 0) return 1;
    if ((c & 0xE0) == 0xC0) return 2;
    if ((c & 0xF0) == 0xE0) return 3;
    if ((c & 0xF8) == 0xF0) return 4;
    return 1;
}

bool utf8_decode_one(std::string_view text, size_t at, uint32_t *out_cp, int *out_len) {
    if (at >= text.size()) return false;
    const unsigned char c = static_cast<unsigned char>(text[at]);
    const int len = std::min<int>(utf8_unit_length(c), static_cast<int>(text.size() - at));
    if (len == 1) {
        *out_cp = c;
        *out_len = 1;
        return true;
    }
    uint32_t cp = c & (0xFF >> (len + 1));
    for (int i = 1; i < len; ++i) {
        const unsigned char cc = static_cast<unsigned char>(text[at + i]);
        if ((cc & 0xC0) != 0x80) {
            *out_cp = c;
            *out_len = 1;
            return true;
        }
        cp = (cp << 6) | (cc & 0x3F);
    }
    *out_cp = cp;
    *out_len = len;
    return true;
}

size_t utf8_to_utf16_offset(std::string_view text, size_t byte_offset) {
    const size_t limit = std::min(byte_offset, text.size());
    size_t at = 0;
    size_t units = 0;
    while (at < limit) {
        uint32_t cp = 0;
        int len = 1;
        utf8_decode_one(text, at, &cp, &len);
        if (at + static_cast<size_t>(len) > limit) {
            break;
        }
        units += (cp > 0xFFFF) ? 2 : 1;
        at += static_cast<size_t>(len);
    }
    return units;
}

size_t utf16_to_utf8_offset(std::string_view text, size_t unit_offset) {
    size_t at = 0;
    size_t units = 0;
    while (at < text.size() && units < unit_offset) {
        uint32_t cp = 0;
        int len = 1;
        utf8_decode_one(text, at, &cp, &len);
        const size_t add = (cp > 0xFFFF) ? 2 : 1;
        if (units + add > unit_offset) {
            break;
        }
        units += add;
        at += static_cast<size_t>(len);
    }
    return at;
}

TextAlign text_align_from_int(int align) {
    switch (align) {
        case 1:
            return TextAlign::kCenter;
        case 2:
            return TextAlign::kRight;
        default:
            return TextAlign::kLeft;
    }
}

}  // namespace

struct ZantImage {
    sk_sp<SkImage> image;
    int width = 0;
    int height = 0;
};

struct ZantTextLayout {
    std::string text;
    ZantTextStyle style{};
    std::unique_ptr<Paragraph> paragraph;
    float layout_width = 0.0f;
    /* rich text: always laid out at the given width */
    bool rich = false;

    void layout_at(float width) {
        if (!paragraph) return;
        const float target = width > 0.0f ? width : 1.0e9f;
        if (target == layout_width) return;
        paragraph->layout(target);
        layout_width = target;
    }
};

namespace {

std::unique_ptr<Paragraph> build_paragraph(std::string_view text, const ZantTextStyle &style,
                                           float max_width) {
    ParagraphStyle paragraph_style;
    paragraph_style.setTextAlign(text_align_from_int(style.align));
    if (style.max_lines > 0) {
        paragraph_style.setMaxLines(static_cast<size_t>(style.max_lines));
        // text cut off by max_lines ends in "…"
        paragraph_style.setEllipsis(SkString("\u2026"));
    }

    TextStyle text_style;
    text_style.setColor(style.color);
    text_style.setFontSize(style.font_size > 0.0f ? style.font_size : 14.0f);
    const int weight = std::max(100, std::min(900, style.weight));
    const SkFontStyle sk_style(weight, SkFontStyle::kNormal_Width, SkFontStyle::kUpright_Slant);
    text_style.setFontStyle(sk_style);
    if (style.family >= 1 && style.family <= 4) {
        if (auto face = family_typeface(style.family, sk_style)) text_style.setTypeface(std::move(face));
    }
    if (style.line_height > 0.0f) {
        text_style.setHeight(style.line_height);
        text_style.setHeightOverride(true);
    }

    auto builder = ParagraphBuilder::make(paragraph_style, g_font_collection, g_unicode);
    if (!builder) {
        return nullptr;
    }
    builder->pushStyle(text_style);
    if (!text.empty()) {
        builder->addText(text.data(), text.size());
    }
    auto paragraph = builder->Build();
    if (paragraph) {
        // max_lines text is laid out at the box width so it ends in "…"
        const bool fixed_width = style.wrap || style.max_lines > 0;
        paragraph->layout(fixed_width ? std::max(max_width, 1.0f) : 1.0e9f);
        // a single unwrapped line: centre / right alignment within the huge
        // layout width would move the glyphs far off the box, so lay it out
        // at its own width (the container positions it)
        if (!fixed_width && style.align != 0) {
            paragraph->layout(std::ceil(paragraph->getMaxIntrinsicWidth()) + 1.0f);
        }
    }
    return paragraph;
}

}  // namespace

void zant_skia_ensure() {
    ensure_skia();
}

bool zant_write_file_utf8(const char *path, const void *data, size_t size) {
    if (!path) return false;
#if defined(_WIN32)
    const int wide_len = MultiByteToWideChar(CP_UTF8, 0, path, -1, nullptr, 0);
    if (wide_len <= 0) return false;
    std::wstring wide(static_cast<size_t>(wide_len), L'\0');
    MultiByteToWideChar(CP_UTF8, 0, path, -1, wide.data(), wide_len);
    FILE *file = _wfopen(wide.c_str(), L"wb");
#else
    FILE *file = fopen(path, "wb");
#endif
    if (!file) return false;
    const bool ok = size == 0 || fwrite(data, 1, size, file) == size;
    return fclose(file) == 0 && ok;
}

extern "C" {

void zant_skia_init(void) {
    ensure_skia();
}

int zant_write_file(const char *path, const void *data, size_t size) {
    return zant_write_file_utf8(path, data, size) ? 1 : 0;
}

ZantRenderer *zant_renderer_create_raster(int pixel_width, int pixel_height) {
    ensure_skia();
    if (pixel_width <= 0 || pixel_height <= 0) return nullptr;
    auto *renderer = new (std::nothrow) ZantRenderer();
    if (!renderer) return nullptr;
    renderer->width = pixel_width;
    renderer->height = pixel_height;
    const SkImageInfo info = SkImageInfo::Make(pixel_width, pixel_height, kN32_SkColorType,
                                               kPremul_SkAlphaType, SkColorSpace::MakeSRGB());
    renderer->surface = SkSurfaces::Raster(info, nullptr);
    if (!renderer->surface) {
        delete renderer;
        return nullptr;
    }
    return renderer;
}

void zant_renderer_destroy(ZantRenderer *renderer) {
    if (!renderer) return;
    renderer->canvas.canvas = nullptr;
    renderer->surface.reset();
    renderer->gpu_surface.reset();
    if (renderer->gpu) zant_platform::gpu_destroy(renderer);
    renderer->gr_context.reset();
    delete renderer;
}

bool zant_renderer_resize(ZantRenderer *renderer, int pixel_width, int pixel_height) {
    if (!renderer || pixel_width <= 0 || pixel_height <= 0) return false;
    if (renderer->width == pixel_width && renderer->height == pixel_height) return true;
    if (renderer->gpu) {
        zant_platform::gpu_resize(renderer, pixel_width, pixel_height);
        renderer->width = pixel_width;
        renderer->height = pixel_height;
        return true;
    }
    const SkImageInfo info = SkImageInfo::Make(pixel_width, pixel_height, kN32_SkColorType,
                                               kPremul_SkAlphaType, SkColorSpace::MakeSRGB());
    auto new_surface = SkSurfaces::Raster(info, nullptr);
    if (!new_surface) return false;
    renderer->surface = std::move(new_surface);
    renderer->width = pixel_width;
    renderer->height = pixel_height;
    return true;
}

ZantCanvas *zant_renderer_begin(ZantRenderer *renderer, float scale) {
    if (!renderer) return nullptr;
    if (renderer->gpu) {
        if (!renderer->gr_context) return nullptr;
        renderer->gpu_surface = zant_platform::gpu_begin_frame(renderer);
        if (!renderer->gpu_surface) return nullptr;
        renderer->canvas.canvas = renderer->gpu_surface->getCanvas();
    } else {
        if (!renderer->surface) return nullptr;
        renderer->canvas.canvas = renderer->surface->getCanvas();
    }
    if (!renderer->canvas.canvas) return nullptr;
    renderer->canvas.canvas->save();
    renderer->saved++;
    if (scale > 0.0f && scale != 1.0f) {
        renderer->canvas.canvas->scale(scale, scale);
    }
    return &renderer->canvas;
}

namespace {

bool write_png(const SkPixmap &pixmap, const char *path) {
    SkDynamicMemoryWStream stream;
    SkPngEncoder::Options opts;
    if (!SkPngEncoder::Encode(&stream, pixmap, opts)) return false;
    auto data = stream.detachAsData();
    return zant_write_file_utf8(path, data->data(), data->size());
}

}  // namespace

void zant_renderer_end(ZantRenderer *renderer) {
    if (!renderer) return;
    if (renderer->canvas.canvas) {
        while (renderer->saved > 0) {
            renderer->canvas.canvas->restore();
            renderer->saved--;
        }
        renderer->canvas.canvas = nullptr;
    }
    if (renderer->gpu) {
        if (renderer->gpu_surface && renderer->gr_context) {
            if (!renderer->capture_path.empty()) {
                SkBitmap bitmap;
                if (bitmap.tryAllocPixels(renderer->gpu_surface->imageInfo()) &&
                    renderer->gpu_surface->readPixels(bitmap.pixmap(), 0, 0) &&
                    write_png(bitmap.pixmap(), renderer->capture_path.c_str())) {
                    fprintf(stderr, "[zant] GPU snapshot: %s\n", renderer->capture_path.c_str());
                }
                renderer->capture_path.clear();
            }
            zant_platform::gpu_present(renderer);
        }
        renderer->gpu_surface.reset();
    }
}

void zant_renderer_present(ZantRenderer *renderer) {
    (void)renderer;
}

void zant_renderer_capture_next_png(ZantRenderer *renderer, const char *path) {
    if (!renderer || !path) return;
    renderer->capture_path.assign(path);
}

void *zant_renderer_pixels(ZantRenderer *renderer, int *width, int *height, int *stride) {
    if (!renderer || !renderer->surface) return nullptr;
    SkPixmap pixmap;
    if (!renderer->surface->peekPixels(&pixmap)) return nullptr;
    if (width) *width = pixmap.width();
    if (height) *height = pixmap.height();
    if (stride) *stride = static_cast<int>(pixmap.rowBytes());
    return pixmap.writable_addr();
}

uint32_t zant_renderer_pixel_argb(ZantRenderer *renderer, int x, int y) {
    if (!renderer || !renderer->surface) return 0;
    SkPixmap pixmap;
    if (!renderer->surface->peekPixels(&pixmap)) return 0;
    if (x < 0 || y < 0 || x >= pixmap.width() || y >= pixmap.height()) return 0;
    /* raw (premultiplied) bytes, independent of the native byte order */
    const uint8_t *p = static_cast<const uint8_t *>(pixmap.addr(x, y));
    const bool bgra = pixmap.colorType() == kBGRA_8888_SkColorType;
    const uint32_t r = bgra ? p[2] : p[0];
    const uint32_t b = bgra ? p[0] : p[2];
    return (uint32_t(p[3]) << 24) | (r << 16) | (uint32_t(p[1]) << 8) | b;
}

bool zant_renderer_is_gpu(ZantRenderer *renderer) {
    return renderer && renderer->gpu;
}

const char *zant_renderer_backend_name(ZantRenderer *renderer) {
    if (!renderer) return "none";
    return renderer->gpu ? zant_platform::gpu_backend_name() : "Raster CPU (Skia)";
}

bool zant_renderer_write_png(ZantRenderer *renderer, const char *path) {
    if (!renderer || !renderer->surface || !path) return false;
    SkPixmap pixmap;
    if (!renderer->surface->peekPixels(&pixmap)) return false;
    return write_png(pixmap, path);
}

void zant_canvas_save(ZantCanvas *canvas) {
    if (canvas && canvas->canvas) canvas->canvas->save();
}

void zant_canvas_restore(ZantCanvas *canvas) {
    if (canvas && canvas->canvas) canvas->canvas->restore();
}

void zant_canvas_translate(ZantCanvas *canvas, float x, float y) {
    if (canvas && canvas->canvas) canvas->canvas->translate(x, y);
}

void zant_canvas_scale(ZantCanvas *canvas, float sx, float sy) {
    if (canvas && canvas->canvas) canvas->canvas->scale(sx, sy);
}

void zant_canvas_save_layer_alpha(ZantCanvas *canvas, float x, float y, float width, float height, float alpha) {
    if (!canvas || !canvas->canvas) return;
    SkRect bounds = SkRect::MakeXYWH(x, y, width, height);
    canvas->canvas->saveLayerAlphaf(&bounds, alpha);
}

void zant_canvas_clip_rect(ZantCanvas *canvas, float x, float y, float width, float height) {
    if (canvas && canvas->canvas) {
        canvas->canvas->clipRect(SkRect::MakeXYWH(x, y, width, height), SkClipOp::kIntersect, true);
    }
}

void zant_canvas_clear(ZantCanvas *canvas, uint32_t color) {
    if (canvas && canvas->canvas) canvas->canvas->clear(make_color(color));
}

void zant_canvas_fill_rect(ZantCanvas *canvas, float x, float y, float width, float height,
                           uint32_t color) {
    if (!canvas || !canvas->canvas) return;
    canvas->canvas->drawRect(SkRect::MakeXYWH(x, y, width, height), fill_paint(color));
}

void zant_canvas_shadow_round_rect(ZantCanvas *canvas, float x, float y, float width, float height,
                                   float radius, uint32_t color, float blur) {
    if (!canvas || !canvas->canvas || blur <= 0.0f) return;
    SkPaint paint;
    paint.setAntiAlias(true);
    paint.setColor(color);
    paint.setMaskFilter(SkMaskFilter::MakeBlur(kNormal_SkBlurStyle, blur));
    canvas->canvas->drawRRect(make_rrect(x, y, width, height, radius), paint);
}

void zant_canvas_fill_round_rect(ZantCanvas *canvas, float x, float y, float width, float height,
                                 float radius, uint32_t color) {
    if (!canvas || !canvas->canvas) return;
    canvas->canvas->drawRRect(make_rrect(x, y, width, height, radius), fill_paint(color));
}

void zant_canvas_stroke_round_rect(ZantCanvas *canvas, float x, float y, float width, float height,
                                   float radius, uint32_t color, float stroke_width) {
    if (!canvas || !canvas->canvas) return;
    canvas->canvas->drawRRect(make_rrect(x, y, width, height, radius),
                              stroke_paint(color, std::max(0.5f, stroke_width)));
}

void zant_canvas_draw_line(ZantCanvas *canvas, float x0, float y0, float x1, float y1,
                           uint32_t color, float stroke_width) {
    if (!canvas || !canvas->canvas) return;
    canvas->canvas->drawLine(x0, y0, x1, y1, stroke_paint(color, std::max(0.5f, stroke_width)));
}

void zant_canvas_draw_text_layout(ZantCanvas *canvas, ZantTextLayout *layout, float x, float y) {
    if (!canvas || !canvas->canvas || !layout || !layout->paragraph) return;
    layout->paragraph->paint(canvas->canvas, x, y);
}

ZantImage *zant_image_load_file(const char *path) {
    if (!path) return nullptr;
    auto data = SkData::MakeFromFileName(path);
    if (!data) return nullptr;
    auto image = SkImages::DeferredFromEncodedData(std::move(data));
    if (!image) return nullptr;
    auto *result = new (std::nothrow) ZantImage();
    if (!result) return nullptr;
    result->image = std::move(image);
    result->width = result->image->width();
    result->height = result->image->height();
    return result;
}

static ZantImage *wrap_image(sk_sp<SkImage> image) {
    if (!image) return nullptr;
    auto *result = new (std::nothrow) ZantImage();
    if (!result) return nullptr;
    result->width = image->width();
    result->height = image->height();
    result->image = std::move(image);
    return result;
}

ZantImage *zant_image_from_pixels(const void *pixels, int width, int height, int stride) {
    if (!pixels || width <= 0 || height <= 0 || stride < width * 4) return nullptr;
    const SkImageInfo info = SkImageInfo::MakeN32Premul(width, height);
    return wrap_image(SkImages::RasterFromPixmapCopy(SkPixmap(info, pixels, static_cast<size_t>(stride))));
}

ZantImage *zant_image_from_encoded(const void *data, size_t size) {
    if (!data || size == 0) return nullptr;
    auto encoded = SkData::MakeWithCopy(data, size);
    auto image = SkImages::DeferredFromEncodedData(std::move(encoded));
    // decode now (on the calling thread) so drawing never stalls
    if (image) image = image->makeRasterImage(nullptr);
    return wrap_image(std::move(image));
}

ZantImage *zant_image_thumbnail(const void *data, size_t size, int max_dim) {
    if (!data || size == 0 || max_dim <= 0) return nullptr;
    auto image = SkImages::DeferredFromEncodedData(SkData::MakeWithCopy(data, size));
    if (!image) return nullptr;
    const int w = image->width(), h = image->height();
    if (w <= 0 || h <= 0) return nullptr;
    const float scale = std::min(1.0f, static_cast<float>(max_dim) / static_cast<float>(std::max(w, h)));
    const int tw = std::max(1, static_cast<int>(std::lround(w * scale)));
    const int th = std::max(1, static_cast<int>(std::lround(h * scale)));
    SkBitmap bitmap;
    if (!bitmap.tryAllocPixels(SkImageInfo::MakeN32Premul(tw, th))) return nullptr;
    // mipmapped linear filtering: a smooth downscale from any size
    if (!image->scalePixels(bitmap.pixmap(), SkSamplingOptions(SkFilterMode::kLinear, SkMipmapMode::kLinear))) return nullptr;
    bitmap.setImmutable();
    return wrap_image(SkImages::RasterFromBitmap(bitmap));
}

void *zant_image_encode_png(ZantImage *image, size_t *out_size) {
    if (out_size) *out_size = 0;
    if (!image || !image->image) return nullptr;
    SkBitmap bitmap;
    if (!bitmap.tryAllocPixels(SkImageInfo::MakeN32Premul(image->width, image->height))) return nullptr;
    if (!image->image->readPixels(nullptr, bitmap.pixmap(), 0, 0)) return nullptr;
    SkDynamicMemoryWStream stream;
    SkPngEncoder::Options opts;
    opts.fZLibLevel = 4; // clipboard images: speed over the last few percent
    if (!SkPngEncoder::Encode(&stream, bitmap.pixmap(), opts)) return nullptr;
    auto encoded = stream.detachAsData();
    void *bytes = std::malloc(encoded->size());
    if (!bytes) return nullptr;
    std::memcpy(bytes, encoded->data(), encoded->size());
    if (out_size) *out_size = encoded->size();
    return bytes;
}

int zant_image_read_rgba(ZantImage *image, void *dst, size_t dst_size) {
    if (!image || !image->image || !dst) return 0;
    const size_t row = static_cast<size_t>(image->width) * 4;
    if (dst_size < row * static_cast<size_t>(image->height)) return 0;
    const SkImageInfo info = SkImageInfo::Make(image->width, image->height, kRGBA_8888_SkColorType, kUnpremul_SkAlphaType);
    return image->image->readPixels(nullptr, info, dst, row, 0, 0) ? 1 : 0;
}

void zant_bytes_free(void *bytes) {
    std::free(bytes);
}

ZantImage *zant_image_retain(ZantImage *image) {
    if (!image) return nullptr;
    return wrap_image(image->image);
}

void zant_image_destroy(ZantImage *image) {
    delete image;
}

int zant_image_width(ZantImage *image) {
    return image ? image->width : 0;
}

int zant_image_height(ZantImage *image) {
    return image ? image->height : 0;
}

void zant_canvas_draw_image_rect(ZantCanvas *canvas, ZantImage *image, float x, float y,
                                 float width, float height, float radius, int fit) {
    if (!canvas || !canvas->canvas || !image || !image->image || width <= 0 || height <= 0) return;
    SkCanvas *c = canvas->canvas;
    c->save();
    c->clipRRect(make_rrect(x, y, width, height, radius));
    const float image_w = static_cast<float>(image->width);
    const float image_h = static_cast<float>(image->height);
    SkRect src = SkRect::MakeWH(image_w, image_h);
    SkRect dst = SkRect::MakeXYWH(x, y, width, height);
    if (fit == 0) {
        const float scale = std::max(width / image_w, height / image_h);
        const float crop_w = width / scale;
        const float crop_h = height / scale;
        src = SkRect::MakeXYWH((image_w - crop_w) * 0.5f, (image_h - crop_h) * 0.5f, crop_w, crop_h);
    } else if (fit == 1) {
        const float scale = std::min(width / image_w, height / image_h);
        const float draw_w = image_w * scale;
        const float draw_h = image_h * scale;
        dst = SkRect::MakeXYWH(x + (width - draw_w) * 0.5f, y + (height - draw_h) * 0.5f, draw_w, draw_h);
    }
    SkSamplingOptions sampling(SkFilterMode::kLinear, SkMipmapMode::kLinear);
    c->drawImageRect(image->image.get(), src, dst, sampling, nullptr, SkCanvas::kStrict_SrcRectConstraint);
    c->restore();
}

ZantTextLayout *zant_rich_text_layout_create(const ZantRichSpan *spans, size_t count,
                                             float max_width, float line_height) {
    if (!spans || count == 0) return nullptr;
    std::string joined;
    for (size_t i = 0; i < count; ++i) {
        if (spans[i].placeholder_width > 0.0f) {
            joined.append("\xEF\xBF\xBC");  // U+FFFC, one UTF-16 unit like the placeholder
        } else if (spans[i].text && spans[i].length > 0) {
            joined.append(spans[i].text, spans[i].length);
        }
    }
    ParagraphStyle paragraph_style;
    paragraph_style.setTextAlign(TextAlign::kLeft);
    auto builder = ParagraphBuilder::make(paragraph_style, g_font_collection, g_unicode);
    if (!builder) return nullptr;
    for (size_t i = 0; i < count; ++i) {
        const ZantRichSpan &span = spans[i];
        TextStyle style;
        style.setColor(span.color);
        style.setFontSize(span.font_size > 0.0f ? span.font_size : 14.0f);
        const int weight = std::max(100, std::min(900, span.weight));
        const SkFontStyle sk_style(weight, SkFontStyle::kNormal_Width,
                                   span.italic ? SkFontStyle::kItalic_Slant : SkFontStyle::kUpright_Slant);
        style.setFontStyle(sk_style);
        if (span.family >= 1 && span.family <= 4) {
            if (auto face = family_typeface(span.family, sk_style)) style.setTypeface(std::move(face));
        }
        if (line_height > 0.0f) {
            style.setHeight(line_height);
            style.setHeightOverride(true);
        }
        if (span.background != 0) {
            SkPaint background;
            background.setColor(span.background);
            style.setBackgroundPaint(std::move(background));
        }
        if (span.decoration != 0) {
            int decoration = 0;
            if (span.decoration & 1) decoration |= TextDecoration::kUnderline;
            if (span.decoration & 2) decoration |= TextDecoration::kLineThrough;
            style.setDecoration(static_cast<TextDecoration>(decoration));
            style.setDecorationColor(span.color);
            style.setDecorationThicknessMultiplier(1.0f);
        }
        builder->pushStyle(style);
        if (span.placeholder_width > 0.0f) {
            const float height = std::max(1.0f, span.placeholder_ascent + span.placeholder_descent);
            builder->addPlaceholder(PlaceholderStyle(span.placeholder_width, height,
                                                     PlaceholderAlignment::kBaseline,
                                                     TextBaseline::kAlphabetic,
                                                     span.placeholder_ascent));
        } else if (span.text && span.length > 0) {
            builder->addText(span.text, span.length);
        }
        builder->pop();
    }
    auto paragraph = builder->Build();
    if (!paragraph) return nullptr;
    paragraph->layout(std::max(max_width, 1.0f));
    auto *result = new (std::nothrow) ZantTextLayout();
    if (!result) return nullptr;
    result->text = std::move(joined);
    result->paragraph = std::move(paragraph);
    result->layout_width = max_width;
    result->rich = true;
    return result;
}

ZantTextLayout *zant_text_layout_create(const char *utf8, size_t length,
                                        const ZantTextStyle *style, float max_width) {
    ensure_skia();
    if (!utf8 || !style || style->font_size <= 0.0f) return nullptr;
    auto *layout = new (std::nothrow) ZantTextLayout();
    if (!layout) return nullptr;
    layout->text.assign(utf8, length);
    layout->style = *style;
    layout->paragraph = build_paragraph(layout->text, *style, max_width);
    if (!layout->paragraph) {
        delete layout;
        return nullptr;
    }
    layout->layout_width = style->wrap ? std::max(max_width, 1.0f) : 1.0e9f;
    return layout;
}

void zant_text_layout_destroy(ZantTextLayout *layout) {
    delete layout;
}

void zant_text_layout_relayout(ZantTextLayout *layout, float max_width) {
    if (!layout || !layout->paragraph) return;
    Paragraph *p = layout->paragraph.get();
    if (layout->rich) {
        p->layout(std::max(max_width, 1.0f));
        layout->layout_width = max_width;
        return;
    }
    // same rules as build_paragraph
    const ZantTextStyle &style = layout->style;
    const bool fixed_width = style.wrap || style.max_lines > 0;
    p->layout(fixed_width ? std::max(max_width, 1.0f) : 1.0e9f);
    if (!fixed_width && style.align != 0) p->layout(std::ceil(p->getMaxIntrinsicWidth()) + 1.0f);
    layout->layout_width = style.wrap ? std::max(max_width, 1.0f) : 1.0e9f;
}

ZantTextMetrics zant_text_layout_metrics(ZantTextLayout *layout) {
    ZantTextMetrics metrics{};
    if (!layout || !layout->paragraph) return metrics;
    metrics.width = layout->paragraph->getLongestLine();
    metrics.height = layout->paragraph->getHeight();
    metrics.max_intrinsic_width = layout->paragraph->getMaxIntrinsicWidth();
    metrics.longest_line = layout->paragraph->getLongestLine();
    metrics.first_baseline = layout->paragraph->getAlphabeticBaseline();
    metrics.line_count = static_cast<int>(layout->paragraph->lineNumber());
    return metrics;
}

bool zant_text_layout_caret_rect(ZantTextLayout *layout, size_t byte_offset, ZantRect *out_rect) {
    if (!layout || !layout->paragraph || !out_rect) return false;
    const size_t utf16_len = utf8_to_utf16_offset(layout->text, layout->text.size());
    const size_t offset = utf8_to_utf16_offset(layout->text, byte_offset);
    auto try_range = [&](size_t start, size_t end, bool take_right) -> bool {
        if (start > end || end > utf16_len) return false;
        auto rects = layout->paragraph->getRectsForRange(
                static_cast<unsigned>(start), static_cast<unsigned>(end), RectHeightStyle::kMax,
                RectWidthStyle::kTight);
        if (rects.empty()) return false;
        const SkRect &rect = rects.front().rect;
        out_rect->x = take_right ? rect.right() : rect.left();
        out_rect->y = rect.top();
        out_rect->width = 1.0f;
        out_rect->height = rect.height();
        return true;
    };

    if (layout->text.empty()) {
        const auto metrics = zant_text_layout_metrics(layout);
        out_rect->x = 0.0f;
        out_rect->y = 0.0f;
        out_rect->width = 1.0f;
        out_rect->height = metrics.height > 0.0f ? metrics.height : layout->style.font_size * 1.2f;
        return true;
    }
    if (offset >= utf16_len) {
        if (try_range(utf16_len - 1, utf16_len, true)) return true;
    } else {
        if (try_range(offset, offset + 1, false)) return true;
        if (try_range(offset, offset, false)) return true;
    }
    const auto metrics = zant_text_layout_metrics(layout);
    out_rect->x = 0.0f;
    out_rect->y = 0.0f;
    out_rect->width = 1.0f;
    out_rect->height = metrics.height > 0.0f ? metrics.height : layout->style.font_size * 1.2f;
    return true;
}

size_t zant_text_layout_index_at(ZantTextLayout *layout, float x, float y) {
    if (!layout || !layout->paragraph) return 0;
    auto position = layout->paragraph->getGlyphPositionAtCoordinate(x, y);
    if (position.position < 0) return 0;
    return utf16_to_utf8_offset(layout->text, static_cast<size_t>(position.position));
}

int zant_text_layout_range_rects(ZantTextLayout *layout, size_t start_byte, size_t end_byte,
                                 ZantRect *out_rects, int max_rects) {
    if (!layout || !layout->paragraph || !out_rects || max_rects <= 0) return 0;
    size_t start = utf8_to_utf16_offset(layout->text, start_byte);
    size_t end = utf8_to_utf16_offset(layout->text, end_byte);
    if (start > end) std::swap(start, end);
    if (start == end) return 0;
    auto rects = layout->paragraph->getRectsForRange(static_cast<unsigned>(start),
                                                     static_cast<unsigned>(end),
                                                     RectHeightStyle::kMax, RectWidthStyle::kTight);
    int count = 0;
    for (const auto &box : rects) {
        if (count >= max_rects) break;
        out_rects[count].x = box.rect.left();
        out_rects[count].y = box.rect.top();
        out_rects[count].width = box.rect.width();
        out_rects[count].height = box.rect.height();
        count++;
    }
    return count;
}

}  // extern "C"

namespace {

sk_sp<SkTypeface> typeface_for(int family, int weight, bool italic) {
    ensure_skia();
    struct Entry { int family; int weight; bool italic; sk_sp<SkTypeface> face; };
    static std::vector<Entry> cache;
    for (const auto &entry : cache) {
        if (entry.family == family && entry.weight == weight && entry.italic == italic) return entry.face;
    }
    const SkFontStyle style(std::max(100, std::min(900, weight)), SkFontStyle::kNormal_Width,
                            italic ? SkFontStyle::kItalic_Slant : SkFontStyle::kUpright_Slant);
    sk_sp<SkTypeface> face = family_typeface((family >= 0 && family <= 4) ? family : 0, style);
    if (!face) face = family_typeface(0, style);
    if (!face) face = g_font_mgr->legacyMakeTypeface(nullptr, style);
    cache.push_back({family, weight, italic, face});
    return face;
}

struct GlyphRun {
    sk_sp<SkTypeface> face;
    size_t start;
    size_t end;
};

// Splits UTF-8 text into runs that share one typeface, falling back per
// character to any installed font that has the glyph (CJK in \text{}, emoji).
std::vector<GlyphRun> fallback_runs(const char *utf8, size_t length, int family, int weight,
                                    bool italic) {
    std::vector<GlyphRun> runs;
    sk_sp<SkTypeface> primary = typeface_for(family, weight, italic);
    const SkFontStyle style(std::max(100, std::min(900, weight)), SkFontStyle::kNormal_Width,
                            italic ? SkFontStyle::kItalic_Slant : SkFontStyle::kUpright_Slant);
    std::string_view text(utf8, length);
    size_t at = 0;
    while (at < length) {
        uint32_t cp = 0;
        int len = 1;
        if (!utf8_decode_one(text, at, &cp, &len) || len <= 0) {
            cp = 0xFFFD;
            len = 1;
        }
        sk_sp<SkTypeface> face = primary;
        if (primary && primary->unicharToGlyph(static_cast<SkUnichar>(cp)) == 0 && cp > 0x20) {
            sk_sp<SkTypeface> fallback =
                    g_font_mgr->matchFamilyStyleCharacter(nullptr, style, nullptr, 0, static_cast<SkUnichar>(cp));
            if (fallback) face = fallback;
        }
        if (!runs.empty() && runs.back().face == face) {
            runs.back().end = at + len;
        } else {
            runs.push_back({face, at, at + static_cast<size_t>(len)});
        }
        at += static_cast<size_t>(len);
    }
    return runs;
}

}  // namespace

extern "C" {

ZantInkMetrics zant_text_measure_ink(const char *utf8, size_t length, float size, int family,
                                     int weight, int italic) {
    ZantInkMetrics m{};
    if (!utf8 || length == 0 || size <= 0.0f) return m;
    float x = 0.0f;
    float top = 0.0f;
    float bottom = 0.0f;
    bool any_ink = false;
    for (const auto &run : fallback_runs(utf8, length, family, weight, italic != 0)) {
        SkFont font(run.face, size);
        font.setSubpixel(true);
        SkRect bounds;
        const float advance =
                font.measureText(utf8 + run.start, run.end - run.start, SkTextEncoding::kUTF8, &bounds);
        if (!bounds.isEmpty()) {
            top = any_ink ? std::min(top, bounds.top()) : bounds.top();
            bottom = any_ink ? std::max(bottom, bounds.bottom()) : bounds.bottom();
            any_ink = true;
        }
        SkFontMetrics fm;
        font.getMetrics(&fm);
        m.font_ascent = std::max(m.font_ascent, -fm.fAscent);
        m.font_descent = std::max(m.font_descent, fm.fDescent);
        x += advance;
    }
    m.advance = x;
    m.ink_top = any_ink ? top : 0.0f;
    m.ink_bottom = any_ink ? bottom : 0.0f;
    m.ascent = any_ink ? std::max(0.0f, -top) : 0.0f;
    m.descent = any_ink ? std::max(0.0f, bottom) : 0.0f;
    return m;
}

void zant_canvas_draw_simple_text(ZantCanvas *canvas, const char *utf8, size_t length, float x,
                                  float y, float size, int family, int weight, int italic,
                                  uint32_t color, float scale_y) {
    if (!canvas || !canvas->canvas || !utf8 || length == 0 || size <= 0.0f) return;
    SkPaint paint;
    paint.setAntiAlias(true);
    paint.setColor(make_color(color));
    SkCanvas *c = canvas->canvas;
    const bool stretch = scale_y > 0.0f && std::abs(scale_y - 1.0f) > 0.001f;
    if (stretch) {
        c->save();
        c->translate(0.0f, y);
        c->scale(1.0f, scale_y);
        c->translate(0.0f, -y);
    }
    float pen = x;
    for (const auto &run : fallback_runs(utf8, length, family, weight, italic != 0)) {
        SkFont font(run.face, size);
        font.setSubpixel(true);
        font.setEdging(SkFont::Edging::kSubpixelAntiAlias);
        c->drawSimpleText(utf8 + run.start, run.end - run.start, SkTextEncoding::kUTF8, pen, y, font, paint);
        pen += font.measureText(utf8 + run.start, run.end - run.start, SkTextEncoding::kUTF8);
    }
    if (stretch) c->restore();
}

void zant_canvas_fill_path(ZantCanvas *canvas, const float *xy, int count, uint32_t color) {
    if (!canvas || !canvas->canvas || !xy || count < 3) return;
    SkPathBuilder builder;
    builder.moveTo(xy[0], xy[1]);
    for (int i = 1; i < count; ++i) builder.lineTo(xy[i * 2], xy[i * 2 + 1]);
    builder.close();
    SkPaint paint;
    paint.setAntiAlias(true);
    paint.setColor(make_color(color));
    canvas->canvas->drawPath(builder.detach(), paint);
}

void zant_canvas_stroke_path(ZantCanvas *canvas, const float *xy, int count, uint32_t color,
                             float stroke_width, int closed) {
    if (!canvas || !canvas->canvas || !xy || count < 2) return;
    SkPathBuilder builder;
    builder.moveTo(xy[0], xy[1]);
    for (int i = 1; i < count; ++i) builder.lineTo(xy[i * 2], xy[i * 2 + 1]);
    if (closed) builder.close();
    SkPaint paint;
    paint.setAntiAlias(true);
    paint.setStyle(SkPaint::kStroke_Style);
    paint.setStrokeWidth(stroke_width);
    paint.setStrokeJoin(SkPaint::kRound_Join);
    paint.setStrokeCap(SkPaint::kRound_Cap);
    paint.setColor(make_color(color));
    canvas->canvas->drawPath(builder.detach(), paint);
}

int zant_text_layout_placeholder_rects(ZantTextLayout *layout, ZantRect *out_rects, int max_rects) {
    if (!layout || !layout->paragraph || !out_rects || max_rects <= 0) return 0;
    auto rects = layout->paragraph->getRectsForPlaceholders();
    int count = 0;
    for (const auto &box : rects) {
        if (count >= max_rects) break;
        out_rects[count].x = box.rect.left();
        out_rects[count].y = box.rect.top();
        out_rects[count].width = box.rect.width();
        out_rects[count].height = box.rect.height();
        count++;
    }
    return count;
}

}  // extern "C"
