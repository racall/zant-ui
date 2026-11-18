// Portable Skia bridge core: canvas, text (SkParagraph), images, paths and
// the raster renderer. Font manager, font names and the GPU swap chain live
// in skia_bridge_mac.mm / skia_bridge_win.cpp (see skia_bridge_internal.h).
#include "skia_bridge_internal.h"

#include "include/core/SkBitmap.h"
#include "include/core/SkColorSpace.h"
#include "include/core/SkData.h"
#include "include/core/SkImageInfo.h"
#include "include/core/SkPixmap.h"
#include "include/core/SkStream.h"
#include "include/encode/SkPngEncoder.h"

#include <cstdio>
#include <new>
#include <utility>

#if defined(_WIN32)
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#endif

using zant_skia::ensure_skia;

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

}  // extern "C"
