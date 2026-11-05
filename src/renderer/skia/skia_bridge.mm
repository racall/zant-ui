#include "skia_bridge.h"

#include "include/core/SkBitmap.h"
#include "include/core/SkCanvas.h"
#include "include/core/SkColorSpace.h"
#include "include/core/SkData.h"
#include "include/core/SkFontMgr.h"
#include "include/core/SkFontStyle.h"
#include "include/core/SkImageInfo.h"
#include "include/core/SkImage.h"
#include "include/core/SkData.h"
#include "include/core/SkSamplingOptions.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPixmap.h"
#include "include/core/SkRRect.h"
#include "include/core/SkStream.h"
#include "include/core/SkSurface.h"
#include "include/encode/SkPngEncoder.h"
#include "include/ports/SkFontMgr_mac_ct.h"
#include "modules/skparagraph/include/FontCollection.h"
#include "modules/skparagraph/include/Paragraph.h"
#include "modules/skparagraph/include/ParagraphBuilder.h"
#include "modules/skparagraph/include/ParagraphStyle.h"
#include "modules/skparagraph/include/TextStyle.h"
#include "modules/skunicode/include/SkUnicode_icu.h"
#include "include/gpu/ganesh/GrBackendSurface.h"
#include "include/gpu/ganesh/GrDirectContext.h"
#include "include/gpu/ganesh/SkSurfaceGanesh.h"
#include "include/gpu/ganesh/mtl/GrMtlBackendContext.h"
#include "include/gpu/ganesh/mtl/GrMtlDirectContext.h"
#include "include/gpu/ganesh/mtl/GrMtlBackendSurface.h"
#include "include/gpu/ganesh/mtl/SkSurfaceMetal.h"
#include "include/ports/SkCFObject.h"

#import <Metal/Metal.h>
#import <QuartzCore/CAMetalLayer.h>

#include <algorithm>
#include <cstring>
#include <cstdio>
#include <fstream>
#include <limits>
#include <memory>
#include <string>
#include <string_view>
#include <vector>

using namespace skia::textlayout;

namespace {

sk_sp<SkFontMgr> g_font_mgr;
sk_sp<FontCollection> g_font_collection;
sk_sp<SkUnicode> g_unicode;

void ensure_skia() {
    if (g_font_collection) {
        return;
    }
    g_font_mgr = SkFontMgr_New_CoreText(nullptr);
    if (!g_font_mgr) {
        g_font_mgr = SkFontMgr::RefEmpty();
    }
    g_font_collection = sk_make_sp<FontCollection>();
    g_font_collection->setDefaultFontManager(
            g_font_mgr, std::vector<SkString>{SkString("SF Pro Text"), SkString("Helvetica Neue"),
                                              SkString("PingFang SC"), SkString("Apple Color Emoji")});
    g_font_collection->enableFontFallback();
    g_unicode = SkUnicodes::ICU::Make();
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

struct ZantCanvas {
    SkCanvas *canvas = nullptr;
};

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

    void layout_at(float width) {
        if (!paragraph) return;
        const float target = width > 0.0f ? width : 1.0e9f;
        if (target == layout_width) return;
        paragraph->layout(target);
        layout_width = target;
    }
};

struct ZantRenderer {
    bool gpu = false;
    int width = 0;
    int height = 0;
    sk_sp<SkSurface> surface;
    ZantCanvas canvas{};
    int saved = 0;

    __unsafe_unretained CAMetalLayer *metal_layer = nil;
    sk_sp<GrDirectContext> gr_context;
    sk_sp<SkSurface> gpu_surface;
    GrMTLHandle current_drawable = nil;
    std::string capture_path;
};

namespace {

std::unique_ptr<Paragraph> build_paragraph(std::string_view text, const ZantTextStyle &style,
                                           float max_width) {
    ParagraphStyle paragraph_style;
    paragraph_style.setTextAlign(text_align_from_int(style.align));
    if (style.max_lines > 0) {
        paragraph_style.setMaxLines(static_cast<size_t>(style.max_lines));
    }

    TextStyle text_style;
    text_style.setColor(style.color);
    text_style.setFontSize(style.font_size > 0.0f ? style.font_size : 14.0f);
    const int weight = std::max(100, std::min(900, style.weight));
    const SkFontStyle sk_style(weight, SkFontStyle::kNormal_Width, SkFontStyle::kUpright_Slant);
    text_style.setFontStyle(sk_style);
    if (style.family == 1) {
        auto face = g_font_mgr->matchFamilyStyle("Georgia", sk_style);
        if (face) text_style.setTypeface(std::move(face));
    } else if (style.family == 2) {
        auto face = g_font_mgr->matchFamilyStyle("Menlo", sk_style);
        if (face) text_style.setTypeface(std::move(face));
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
        const float width = style.wrap ? std::max(max_width, 1.0f) : 1.0e9f;
        paragraph->layout(width);
    }
    return paragraph;
}

}  // namespace

extern "C" {

void zant_skia_init(void) {
    ensure_skia();
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

ZantRenderer *zant_renderer_create_metal(void *metal_layer_handle, int pixel_width, int pixel_height) {
    ensure_skia();
    if (!metal_layer_handle || pixel_width <= 0 || pixel_height <= 0) return nullptr;
    @autoreleasepool {
        CAMetalLayer *layer = (__bridge CAMetalLayer *)metal_layer_handle;
        id<MTLDevice> device = layer.device;
        if (!device) device = MTLCreateSystemDefaultDevice();
        if (!device) return nullptr;
        id<MTLCommandQueue> queue = [device newCommandQueue];
        if (!queue) return nullptr;

        layer.device = device;
        layer.pixelFormat = MTLPixelFormatBGRA8Unorm;
        layer.framebufferOnly = YES;
        layer.opaque = YES;
        layer.drawableSize = CGSizeMake(pixel_width, pixel_height);

        GrMtlBackendContext backend;
        backend.fDevice = sk_cfp<GrMTLHandle>((__bridge_retained GrMTLHandle)device);
        backend.fQueue = sk_cfp<GrMTLHandle>((__bridge_retained GrMTLHandle)queue);
        auto context = GrDirectContexts::MakeMetal(backend);
        if (!context) return nullptr;

        auto *renderer = new (std::nothrow) ZantRenderer();
        if (!renderer) return nullptr;
        renderer->gpu = true;
        renderer->width = pixel_width;
        renderer->height = pixel_height;
        renderer->metal_layer = layer;
        renderer->gr_context = std::move(context);
        return renderer;
    }
}

void zant_renderer_destroy(ZantRenderer *renderer) {
    if (!renderer) return;
    renderer->canvas.canvas = nullptr;
    renderer->surface.reset();
    renderer->gpu_surface.reset();
    renderer->gr_context.reset();
    renderer->current_drawable = nil;
    renderer->metal_layer = nil;
    delete renderer;
}

bool zant_renderer_resize(ZantRenderer *renderer, int pixel_width, int pixel_height) {
    if (!renderer || pixel_width <= 0 || pixel_height <= 0) return false;
    if (renderer->width == pixel_width && renderer->height == pixel_height) return true;
    if (renderer->gpu) {
        renderer->width = pixel_width;
        renderer->height = pixel_height;
        if (renderer->metal_layer) {
            renderer->metal_layer.drawableSize = CGSizeMake(pixel_width, pixel_height);
        }
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
        if (!renderer->gr_context || !renderer->metal_layer) return nullptr;
        id<CAMetalDrawable> drawable = [renderer->metal_layer nextDrawable];
        if (!drawable) return nullptr;
        id<MTLTexture> texture = drawable.texture;
        GrMtlTextureInfo texture_info;
        texture_info.fTexture =
                sk_cfp<GrMTLHandle>((__bridge_retained GrMTLHandle)texture);
        GrBackendRenderTarget target = GrBackendRenderTargets::MakeMtl(
                (int)texture.width, (int)texture.height, texture_info);
        SkSurfaceProps props(0, kUnknown_SkPixelGeometry);
        auto surface = SkSurfaces::WrapBackendRenderTarget(
                renderer->gr_context.get(), target, kTopLeft_GrSurfaceOrigin,
                kBGRA_8888_SkColorType, SkColorSpace::MakeSRGB(), &props);
        if (!surface) return nullptr;
        renderer->gpu_surface = std::move(surface);
        renderer->current_drawable = (__bridge_retained GrMTLHandle)drawable;
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
            renderer->gr_context->flushAndSubmit(renderer->gpu_surface.get(), GrSyncCpu::kYes);
            if (!renderer->capture_path.empty()) {
                SkBitmap bitmap;
                if (bitmap.tryAllocPixels(renderer->gpu_surface->imageInfo())) {
                    if (renderer->gpu_surface->readPixels(bitmap.pixmap(), 0, 0)) {
                        SkDynamicMemoryWStream stream;
                        SkPngEncoder::Options opts;
                        if (SkPngEncoder::Encode(&stream, bitmap.pixmap(), opts)) {
                            auto data = stream.detachAsData();
                            std::ofstream file(renderer->capture_path, std::ios::binary);
                            if (file) {
                                file.write(static_cast<const char *>(data->data()),
                                           static_cast<std::streamsize>(data->size()));
                                fprintf(stderr, "[zant] GPU snapshot: %s\n",
                                        renderer->capture_path.c_str());
                            }
                        }
                    }
                }
                renderer->capture_path.clear();
            }
            id<CAMetalDrawable> drawable =
                    (__bridge_transfer id<CAMetalDrawable>)renderer->current_drawable;
            renderer->current_drawable = nil;
            if (drawable) [drawable present];
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

bool zant_renderer_is_gpu(ZantRenderer *renderer) {
    return renderer && renderer->gpu;
}

const char *zant_renderer_backend_name(ZantRenderer *renderer) {
    if (!renderer) return "none";
    return renderer->gpu ? "Metal (Skia Ganesh)" : "Raster CPU (Skia)";
}

bool zant_renderer_write_png(ZantRenderer *renderer, const char *path) {
    if (!renderer || !renderer->surface || !path) return false;
    SkPixmap pixmap;
    if (!renderer->surface->peekPixels(&pixmap)) return false;
    SkDynamicMemoryWStream stream;
    SkPngEncoder::Options opts;
    if (!SkPngEncoder::Encode(&stream, pixmap, opts)) return false;
    auto data = stream.detachAsData();
    std::ofstream file(path, std::ios::binary);
    if (!file) return false;
    file.write(static_cast<const char *>(data->data()), static_cast<std::streamsize>(data->size()));
    return file.good();
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
        style.setFontStyle(SkFontStyle(weight, SkFontStyle::kNormal_Width,
                                       span.italic ? SkFontStyle::kItalic_Slant : SkFontStyle::kUpright_Slant));
        if (line_height > 0.0f) {
            style.setHeight(line_height);
            style.setHeightOverride(true);
        }
        builder->pushStyle(style);
        if (span.text && span.length > 0) builder->addText(span.text, span.length);
        builder->pop();
    }
    auto paragraph = builder->Build();
    if (!paragraph) return nullptr;
    paragraph->layout(std::max(max_width, 1.0f));
    auto *result = new (std::nothrow) ZantTextLayout();
    if (!result) return nullptr;
    result->paragraph = std::move(paragraph);
    result->layout_width = max_width;
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
