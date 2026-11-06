// Windows part of the Skia bridge: DirectWrite font manager, ICU data (an
// RCDATA resource of zant_skia.dll) and a Direct3D 12 (Ganesh) flip-model
// swap chain with vsync'd present.
#include "skia_bridge_internal.h"

#include "include/core/SkColorSpace.h"
#include "include/core/SkData.h"
#include "include/core/SkSurfaceProps.h"
#include "include/gpu/ganesh/GrBackendSurface.h"
#include "include/gpu/ganesh/GrTypes.h"
#include "include/gpu/ganesh/SkSurfaceGanesh.h"
#include "include/gpu/ganesh/d3d/GrD3DBackendContext.h"
#include "include/gpu/ganesh/d3d/GrD3DBackendSurface.h"
#include "include/gpu/ganesh/d3d/GrD3DDirectContext.h"
#include "include/gpu/ganesh/d3d/GrD3DTypes.h"
#include "include/ports/SkTypeface_win.h"

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <d3d12.h>
#include <dxgi1_6.h>
#include <malloc.h>

#include <cstdio>
#include <cstring>
#include <new>

#include "zant_skia_resources.h"

/* rust-skia's ICU loader hook (third_party/icu/SkLoadICU.cpp): ICU common
   data to use instead of looking for icudtl.dat next to the executable. */
extern "C" void C_SetICU(const void *addr);

namespace {

constexpr int kFrames = 2;
constexpr DXGI_FORMAT kFormat = DXGI_FORMAT_R8G8B8A8_UNORM;

HMODULE this_module() {
    HMODULE module = nullptr;
    GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
                               GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                       reinterpret_cast<LPCWSTR>(&this_module), &module);
    return module;
}

/* An RCDATA resource of this DLL (bytes live as long as the module). */
const void *resource_bytes(int id, DWORD *size) {
    HMODULE module = this_module();
    HRSRC info = FindResourceW(module, MAKEINTRESOURCEW(id), MAKEINTRESOURCEW(10) /* RT_RCDATA */);
    if (!info) return nullptr;
    HGLOBAL handle = LoadResource(module, info);
    *size = SizeofResource(module, info);
    return handle ? LockResource(handle) : nullptr;
}

/* ICU wants its data 16-byte aligned; copy the resource once. */
void load_icu_data() {
    static bool done = false;
    if (done) return;
    done = true;
    DWORD size = 0;
    const void *bytes = resource_bytes(ZANT_ICU_DATA, &size);
    if (!bytes || size == 0) {
        fprintf(stderr, "[zant] ICU data resource missing; text shaping may fail\n");
        return;
    }
    void *copy = _aligned_malloc(size, 16);
    if (!copy) return;
    memcpy(copy, bytes, size);
    C_SetICU(copy); /* lives for the whole process */
}

}  // namespace

struct ZantGpu {
    HWND hwnd = nullptr;
    gr_cp<ID3D12Device> device;
    gr_cp<ID3D12CommandQueue> queue;
    gr_cp<IDXGISwapChain3> swap_chain;
    HANDLE frame_waitable = nullptr;
    gr_cp<ID3D12Resource> buffers[kFrames];
    sk_sp<SkSurface> surfaces[kFrames];
    unsigned buffer_index = 0;
    HANDLE fence_event = nullptr;
    gr_cp<ID3D12Fence> fence;
    uint64_t fence_values[kFrames] = {};
};

namespace {

void wait_for_buffer(ZantGpu *gpu, int index) {
    if (gpu->fence->GetCompletedValue() < gpu->fence_values[index]) {
        gpu->fence->SetEventOnCompletion(gpu->fence_values[index], gpu->fence_event);
        WaitForSingleObjectEx(gpu->fence_event, INFINITE, FALSE);
    }
}

bool setup_surfaces(ZantRenderer *renderer, int width, int height) {
    ZantGpu *gpu = renderer->gpu_state;
    GrD3DTextureResourceInfo info(nullptr, nullptr, D3D12_RESOURCE_STATE_PRESENT, kFormat, 1, 1, 0);
    SkSurfaceProps props(0, kUnknown_SkPixelGeometry);
    for (int i = 0; i < kFrames; ++i) {
        if (FAILED(gpu->swap_chain->GetBuffer(i, IID_PPV_ARGS(&gpu->buffers[i])))) return false;
        info.fResource = gpu->buffers[i];
        GrBackendRenderTarget target = GrBackendRenderTargets::MakeD3D(width, height, info);
        gpu->surfaces[i] = SkSurfaces::WrapBackendRenderTarget(
                renderer->gr_context.get(), target, kTopLeft_GrSurfaceOrigin, kRGBA_8888_SkColorType,
                SkColorSpace::MakeSRGB(), &props);
        if (!gpu->surfaces[i]) return false;
    }
    return true;
}

void release_surfaces(ZantRenderer *renderer) {
    ZantGpu *gpu = renderer->gpu_state;
    if (renderer->gr_context) {
        renderer->gr_context->flush();
        renderer->gr_context->submit(GrSyncCpu::kYes);
    }
    for (int i = 0; i < kFrames; ++i) {
        if (gpu->fence) wait_for_buffer(gpu, i);
        gpu->surfaces[i].reset();
        gpu->buffers[i].reset();
    }
}

gr_cp<IDXGIAdapter1> pick_adapter(IDXGIFactory4 *factory) {
    gr_cp<IDXGIFactory6> factory6;
    if (SUCCEEDED(factory->QueryInterface(IID_PPV_ARGS(&factory6)))) {
        for (UINT i = 0;; ++i) {
            gr_cp<IDXGIAdapter1> adapter;
            if (factory6->EnumAdapterByGpuPreference(i, DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE,
                                                     IID_PPV_ARGS(&adapter)) == DXGI_ERROR_NOT_FOUND) {
                break;
            }
            DXGI_ADAPTER_DESC1 desc;
            if (SUCCEEDED(adapter->GetDesc1(&desc)) && (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE)) continue;
            if (SUCCEEDED(D3D12CreateDevice(adapter.get(), D3D_FEATURE_LEVEL_11_0,
                                            __uuidof(ID3D12Device), nullptr))) {
                return adapter;
            }
        }
    }
    for (UINT i = 0;; ++i) {
        gr_cp<IDXGIAdapter1> adapter;
        if (factory->EnumAdapters1(i, &adapter) == DXGI_ERROR_NOT_FOUND) break;
        if (SUCCEEDED(D3D12CreateDevice(adapter.get(), D3D_FEATURE_LEVEL_11_0,
                                        __uuidof(ID3D12Device), nullptr))) {
            return adapter;
        }
    }
    return gr_cp<IDXGIAdapter1>();
}

}  // namespace

namespace zant_platform {

sk_sp<SkFontMgr> make_font_mgr() {
    load_icu_data(); /* before SkUnicodes::ICU::Make() in ensure_skia() */
    return SkFontMgr_New_DirectWrite();
}

std::vector<SkString> default_families() {
    return {SkString("Segoe UI"), SkString("Microsoft YaHei UI"), SkString("Segoe UI Emoji"),
            SkString("Segoe UI Symbol")};
}

const char *const *family_names(int family) {
    static const char *const sans[] = {"Segoe UI", "Microsoft YaHei UI", nullptr};
    static const char *const serif[] = {"Georgia", "Cambria", "Times New Roman", nullptr};
    static const char *const mono[] = {"Cascadia Mono", "Consolas", "Courier New", nullptr};
    static const char *const serif_cjk[] = {"SimSun", "NSimSun", nullptr};
    static const char *const math[] = {"STIX Two Math", "Cambria Math", nullptr};
    switch (family) {
        case 1: return serif;
        case 2: return mono;
        case 3: return serif_cjk;
        case 4: return math;
        default: return sans;
    }
}

sk_sp<SkTypeface> bundled_typeface(int family) {
    if (family != 4) return nullptr;
    static sk_sp<SkTypeface> math;
    static bool loaded = false;
    if (!loaded) {
        loaded = true;
        DWORD size = 0;
        const void *bytes = resource_bytes(ZANT_MATH_FONT, &size);
        if (bytes && size) {
            static sk_sp<SkFontMgr> mgr = SkFontMgr_New_DirectWrite();
            if (mgr) math = mgr->makeFromData(SkData::MakeWithoutCopy(bytes, size));
        }
    }
    return math;
}

sk_sp<SkSurface> gpu_begin_frame(ZantRenderer *renderer) {
    ZantGpu *gpu = renderer->gpu_state;
    if (!gpu || !gpu->swap_chain) return nullptr;
    /* frame latency 1: wait until the swap chain can take another frame */
    if (gpu->frame_waitable) WaitForSingleObjectEx(gpu->frame_waitable, 1000, TRUE);
    const uint64_t current = gpu->fence_values[gpu->buffer_index];
    gpu->buffer_index = gpu->swap_chain->GetCurrentBackBufferIndex();
    wait_for_buffer(gpu, static_cast<int>(gpu->buffer_index));
    gpu->fence_values[gpu->buffer_index] = current + 1;
    return gpu->surfaces[gpu->buffer_index];
}

void gpu_present(ZantRenderer *renderer) {
    ZantGpu *gpu = renderer->gpu_state;
    if (!gpu || !gpu->swap_chain) return;
    GrFlushInfo info;
    renderer->gr_context->flush(renderer->gpu_surface.get(), SkSurfaces::BackendSurfaceAccess::kPresent,
                                info);
    renderer->gr_context->submit();
    gpu->swap_chain->Present(1, 0);
    gpu->queue->Signal(gpu->fence.get(), gpu->fence_values[gpu->buffer_index]);
}

void gpu_resize(ZantRenderer *renderer, int pixel_width, int pixel_height) {
    ZantGpu *gpu = renderer->gpu_state;
    if (!gpu || !gpu->swap_chain) return;
    renderer->gpu_surface.reset();
    release_surfaces(renderer);
    HRESULT hr = gpu->swap_chain->ResizeBuffers(kFrames, static_cast<UINT>(pixel_width),
                                                static_cast<UINT>(pixel_height), kFormat,
                                                DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT);
    if (FAILED(hr)) {
        renderer->gr_context->freeGpuResources();
        hr = gpu->swap_chain->ResizeBuffers(kFrames, static_cast<UINT>(pixel_width),
                                            static_cast<UINT>(pixel_height), kFormat,
                                            DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT);
    }
    if (FAILED(hr)) {
        fprintf(stderr, "[zant] D3D12 ResizeBuffers failed: 0x%08lx\n", static_cast<unsigned long>(hr));
        return;
    }
    gpu->buffer_index = gpu->swap_chain->GetCurrentBackBufferIndex();
    setup_surfaces(renderer, pixel_width, pixel_height);
}

void gpu_destroy(ZantRenderer *renderer) {
    ZantGpu *gpu = renderer->gpu_state;
    if (!gpu) return;
    renderer->gpu_surface.reset();
    release_surfaces(renderer);
    if (gpu->fence_event) CloseHandle(gpu->fence_event);
    if (gpu->frame_waitable) CloseHandle(gpu->frame_waitable);
    gpu->fence.reset();
    gpu->swap_chain.reset();
    renderer->gr_context.reset();
    gpu->queue.reset();
    gpu->device.reset();
    delete gpu;
    renderer->gpu_state = nullptr;
}

const char *gpu_backend_name() {
    return "Direct3D 12 (Skia Ganesh)";
}

}  // namespace zant_platform

extern "C" {

ZantRenderer *zant_renderer_create_metal(void *metal_layer, int pixel_width, int pixel_height) {
    (void)metal_layer;
    (void)pixel_width;
    (void)pixel_height;
    return nullptr;
}

ZantRenderer *zant_renderer_create_d3d(void *hwnd_handle, int pixel_width, int pixel_height) {
    zant_skia_ensure();
    HWND hwnd = static_cast<HWND>(hwnd_handle);
    if (!hwnd || pixel_width <= 0 || pixel_height <= 0) return nullptr;

    gr_cp<IDXGIFactory4> factory;
    if (FAILED(CreateDXGIFactory2(0, IID_PPV_ARGS(&factory)))) return nullptr;
    gr_cp<IDXGIAdapter1> adapter = pick_adapter(factory.get());
    if (!adapter) return nullptr;

    auto *gpu = new (std::nothrow) ZantGpu();
    auto *renderer = new (std::nothrow) ZantRenderer();
    if (!gpu || !renderer) {
        delete gpu;
        delete renderer;
        return nullptr;
    }
    gpu->hwnd = hwnd;
    renderer->gpu_state = gpu;
    renderer->gpu = true;
    renderer->width = pixel_width;
    renderer->height = pixel_height;

    auto fail = [&]() -> ZantRenderer * {
        zant_renderer_destroy(renderer);
        return nullptr;
    };

    if (FAILED(D3D12CreateDevice(adapter.get(), D3D_FEATURE_LEVEL_11_0, IID_PPV_ARGS(&gpu->device)))) {
        return fail();
    }
    D3D12_COMMAND_QUEUE_DESC queue_desc = {};
    queue_desc.Type = D3D12_COMMAND_LIST_TYPE_DIRECT;
    if (FAILED(gpu->device->CreateCommandQueue(&queue_desc, IID_PPV_ARGS(&gpu->queue)))) return fail();

    GrD3DBackendContext backend;
    backend.fAdapter = adapter;
    backend.fDevice = gpu->device;
    backend.fQueue = gpu->queue;
    renderer->gr_context = GrDirectContexts::MakeD3D(backend);
    if (!renderer->gr_context) return fail();

    DXGI_SWAP_CHAIN_DESC1 desc = {};
    desc.BufferCount = kFrames;
    desc.Width = static_cast<UINT>(pixel_width);
    desc.Height = static_cast<UINT>(pixel_height);
    desc.Format = kFormat;
    desc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
    desc.SwapEffect = DXGI_SWAP_EFFECT_FLIP_DISCARD;
    desc.SampleDesc.Count = 1;
    desc.AlphaMode = DXGI_ALPHA_MODE_IGNORE;
    desc.Flags = DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT;
    gr_cp<IDXGISwapChain1> swap_chain;
    if (FAILED(factory->CreateSwapChainForHwnd(gpu->queue.get(), hwnd, &desc, nullptr, nullptr,
                                               &swap_chain))) {
        return fail();
    }
    factory->MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER);
    if (FAILED(swap_chain->QueryInterface(IID_PPV_ARGS(&gpu->swap_chain)))) return fail();
    gpu->swap_chain->SetMaximumFrameLatency(1);
    gpu->frame_waitable = gpu->swap_chain->GetFrameLatencyWaitableObject();
    gpu->buffer_index = gpu->swap_chain->GetCurrentBackBufferIndex();

    for (int i = 0; i < kFrames; ++i) gpu->fence_values[i] = 1;
    if (FAILED(gpu->device->CreateFence(gpu->fence_values[gpu->buffer_index], D3D12_FENCE_FLAG_NONE,
                                        IID_PPV_ARGS(&gpu->fence)))) {
        return fail();
    }
    gpu->fence_event = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    if (!gpu->fence_event) return fail();
    if (!setup_surfaces(renderer, pixel_width, pixel_height)) return fail();
    return renderer;
}

}  // extern "C"

/* DLL entry: nothing to do, but keep thread attach notifications away. */
BOOL WINAPI DllMain(HINSTANCE instance, DWORD reason, LPVOID reserved) {
    (void)reserved;
    if (reason == DLL_PROCESS_ATTACH) DisableThreadLibraryCalls(instance);
    return TRUE;
}
