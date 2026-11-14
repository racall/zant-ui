// macOS part of the Skia bridge: CoreText font manager and the Metal
// (Ganesh) swap chain on a CAMetalLayer.
#include "skia_bridge_internal.h"

#include "include/core/SkColorSpace.h"
#include "include/core/SkSurfaceProps.h"
#include "include/gpu/ganesh/GrBackendSurface.h"
#include "include/gpu/ganesh/SkSurfaceGanesh.h"
#include "include/gpu/ganesh/mtl/GrMtlBackendContext.h"
#include "include/gpu/ganesh/mtl/GrMtlBackendSurface.h"
#include "include/gpu/ganesh/mtl/GrMtlDirectContext.h"
#include "include/gpu/ganesh/mtl/SkSurfaceMetal.h"
#include "include/ports/SkCFObject.h"
#include "include/ports/SkFontMgr_mac_ct.h"

#import <Metal/Metal.h>
#import <QuartzCore/CAMetalLayer.h>

#include <new>

struct ZantGpu {
    __unsafe_unretained CAMetalLayer *layer = nil;
    GrMTLHandle drawable = nil;
};

namespace zant_platform {

sk_sp<SkFontMgr> make_font_mgr() {
    return SkFontMgr_New_CoreText(nullptr);
}

std::vector<SkString> default_families() {
    return {SkString("SF Pro Text"), SkString("Helvetica Neue"), SkString("PingFang SC"),
            SkString("Apple Color Emoji")};
}

const char *const *family_names(int family) {
    static const char *const sans[] = {"SF Pro Text", "Helvetica Neue", nullptr};
    static const char *const serif[] = {"Georgia", nullptr};
    static const char *const mono[] = {"Menlo", nullptr};
    static const char *const serif_cjk[] = {"Songti SC", nullptr};
    static const char *const math[] = {"STIX Two Math", nullptr};
    switch (family) {
        case 1: return serif;
        case 2: return mono;
        case 3: return serif_cjk;
        case 4: return math;
        default: return sans;
    }
}

sk_sp<SkTypeface> bundled_typeface(int family) {
    (void)family; /* macOS ships STIX Two Math */
    return nullptr;
}

sk_sp<SkSurface> gpu_begin_frame(ZantRenderer *renderer) {
    ZantGpu *gpu = renderer->gpu_state;
    if (!gpu || !gpu->layer) return nullptr;
    id<CAMetalDrawable> drawable = [gpu->layer nextDrawable];
    if (!drawable) return nullptr;
    id<MTLTexture> texture = drawable.texture;
    GrMtlTextureInfo texture_info;
    texture_info.fTexture = sk_cfp<GrMTLHandle>((__bridge_retained GrMTLHandle)texture);
    GrBackendRenderTarget target =
            GrBackendRenderTargets::MakeMtl((int)texture.width, (int)texture.height, texture_info);
    SkSurfaceProps props(0, kUnknown_SkPixelGeometry);
    auto surface = SkSurfaces::WrapBackendRenderTarget(renderer->gr_context.get(), target,
                                                       kTopLeft_GrSurfaceOrigin,
                                                       kBGRA_8888_SkColorType,
                                                       SkColorSpace::MakeSRGB(), &props);
    if (!surface) return nullptr;
    gpu->drawable = (__bridge_retained GrMTLHandle)drawable;
    return surface;
}

void gpu_present(ZantRenderer *renderer) {
    renderer->gr_context->flushAndSubmit(renderer->gpu_surface.get(), GrSyncCpu::kYes);
    ZantGpu *gpu = renderer->gpu_state;
    if (!gpu) return;
    id<CAMetalDrawable> drawable = (__bridge_transfer id<CAMetalDrawable>)gpu->drawable;
    gpu->drawable = nil;
    if (drawable) [drawable present];
}

void gpu_resize(ZantRenderer *renderer, int pixel_width, int pixel_height) {
    ZantGpu *gpu = renderer->gpu_state;
    if (gpu && gpu->layer) gpu->layer.drawableSize = CGSizeMake(pixel_width, pixel_height);
}

void gpu_destroy(ZantRenderer *renderer) {
    ZantGpu *gpu = renderer->gpu_state;
    if (!gpu) return;
    if (gpu->drawable) {
        id<CAMetalDrawable> drawable = (__bridge_transfer id<CAMetalDrawable>)gpu->drawable;
        (void)drawable;
        gpu->drawable = nil;
    }
    gpu->layer = nil;
    delete gpu;
    renderer->gpu_state = nullptr;
}

const char *gpu_backend_name() {
    return "Metal (Skia Ganesh)";
}

}  // namespace zant_platform

extern "C" {

ZantRenderer *zant_renderer_create_metal(void *metal_layer_handle, int pixel_width, int pixel_height) {
    zant_skia_ensure();
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
        auto *gpu = new (std::nothrow) ZantGpu();
        if (!renderer || !gpu) {
            delete renderer;
            delete gpu;
            return nullptr;
        }
        gpu->layer = layer;
        renderer->gpu = true;
        renderer->width = pixel_width;
        renderer->height = pixel_height;
        renderer->gpu_state = gpu;
        renderer->gr_context = std::move(context);
        return renderer;
    }
}

ZantRenderer *zant_renderer_create_d3d(void *hwnd, int pixel_width, int pixel_height) {
    (void)hwnd;
    (void)pixel_width;
    (void)pixel_height;
    return nullptr;
}

}  // extern "C"
