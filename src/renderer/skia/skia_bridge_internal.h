// Internal C++ interface between the portable Skia bridge core
// (skia_bridge.cpp) and the per-platform parts (skia_bridge_mac.mm,
// skia_bridge_win.cpp): font manager, font names and the GPU swap chain.
#ifndef ZANT_SKIA_BRIDGE_INTERNAL_H
#define ZANT_SKIA_BRIDGE_INTERNAL_H

#include "skia_bridge.h"

#include "include/core/SkCanvas.h"
#include "include/core/SkFontMgr.h"
#include "include/core/SkRefCnt.h"
#include "include/core/SkString.h"
#include "include/core/SkTypeface.h"
#include "include/core/SkSurface.h"
#include "include/gpu/ganesh/GrDirectContext.h"

#include <string>
#include <vector>

struct ZantCanvas {
    SkCanvas *canvas = nullptr;
};

/* Platform GPU state (Metal layer, D3D12 swap chain); opaque to the core. */
struct ZantGpu;

struct ZantRenderer {
    bool gpu = false;
    int width = 0;
    int height = 0;
    sk_sp<SkSurface> surface; /* raster */
    ZantCanvas canvas{};
    int saved = 0;

    sk_sp<GrDirectContext> gr_context;
    sk_sp<SkSurface> gpu_surface; /* the frame being drawn */
    ZantGpu *gpu_state = nullptr;
    std::string capture_path;
};

namespace zant_platform {

/* System font manager (CoreText / DirectWrite). */
sk_sp<SkFontMgr> make_font_mgr();
/* Default family stack for SkParagraph (UI font, CJK font, emoji...). */
std::vector<SkString> default_families();
/* Candidate family names for a ZantTextStyle family id (0 sans, 1 serif,
   2 mono, 3 serif CJK, 4 math), most preferred first, NULL terminated. */
const char *const *family_names(int family);
/* A font shipped with the framework for a family id (Windows: STIX Two Math
   for 4, which Windows does not install); null to use family_names. */
sk_sp<SkTypeface> bundled_typeface(int family);

/* GPU frame hooks; only called when renderer->gpu is true. */
sk_sp<SkSurface> gpu_begin_frame(ZantRenderer *renderer);
/* Flushes the frame drawn into renderer->gpu_surface and presents it. */
void gpu_present(ZantRenderer *renderer);
void gpu_resize(ZantRenderer *renderer, int pixel_width, int pixel_height);
void gpu_destroy(ZantRenderer *renderer);
const char *gpu_backend_name();

}  // namespace zant_platform

/* Shared with the platform files. */
void zant_skia_ensure();
/* Writes PNG bytes; path is UTF-8 on every platform. */
bool zant_write_file_utf8(const char *path, const void *data, size_t size);

#endif
