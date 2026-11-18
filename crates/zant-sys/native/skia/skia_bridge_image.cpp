// Image loading, encoding, pixel access and canvas drawing.
#include "skia_bridge_internal.h"

#include "include/core/SkBitmap.h"
#include "include/core/SkData.h"
#include "include/core/SkImage.h"
#include "include/core/SkImageInfo.h"
#include "include/core/SkPixmap.h"
#include "include/core/SkRRect.h"
#include "include/core/SkSamplingOptions.h"
#include "include/core/SkStream.h"
#include "include/encode/SkPngEncoder.h"

#include <algorithm>
#include <cmath>
#include <cstdlib>
#include <cstring>
#include <new>
#include <utility>

using zant_skia::make_rrect;

struct ZantImage {
    sk_sp<SkImage> image;
    int width = 0;
    int height = 0;
};

extern "C" {

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

}  // extern "C"
