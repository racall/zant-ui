/* Screens, other apps' windows and screen capture (Windows).
 *
 * The process is per-monitor DPI aware (v2), so every rect here is physical
 * pixels of the virtual screen, origin at the primary monitor's top left.
 */

#include <windows.h>
#include <dwmapi.h>

#include <stdbool.h>
#include <stdlib.h>
#include <string.h>

#include "platform/platform.h"

typedef HRESULT(WINAPI *ZantGetDpiForMonitorFn)(HMONITOR, int, UINT *, UINT *);

static UINT zant_monitor_dpi(HMONITOR monitor) {
    static ZantGetDpiForMonitorFn get_dpi = NULL;
    static bool loaded = false;
    if (!loaded) {
        loaded = true;
        HMODULE shcore = LoadLibraryW(L"shcore.dll");
        if (shcore) get_dpi = (ZantGetDpiForMonitorFn)(void *)GetProcAddress(shcore, "GetDpiForMonitor");
    }
    UINT x = 96, y = 96;
    if (!get_dpi || FAILED(get_dpi(monitor, 0 /* MDT_EFFECTIVE_DPI */, &x, &y))) x = GetDpiForSystem();
    return x ? x : 96;
}

/* ---- screens ---- */

typedef struct {
    ZantScreenInfo *out;
    int capacity, count;
} ZantScreenWalk;

static BOOL CALLBACK zant_screen_walk(HMONITOR monitor, HDC dc, LPRECT rect, LPARAM data) {
    (void)dc;
    (void)rect;
    ZantScreenWalk *walk = (ZantScreenWalk *)data;
    MONITORINFO mi = {sizeof(mi)};
    if (!GetMonitorInfoW(monitor, &mi)) return TRUE;
    if (walk->out && walk->count < walk->capacity) {
        const float scale = (float)zant_monitor_dpi(monitor) / 96.0f;
        ZantScreenInfo *info = &walk->out[walk->count];
        info->id = (uint32_t)walk->count;
        info->x = (float)mi.rcMonitor.left;
        info->y = (float)mi.rcMonitor.top;
        info->width = (float)(mi.rcMonitor.right - mi.rcMonitor.left);
        info->height = (float)(mi.rcMonitor.bottom - mi.rcMonitor.top);
        info->work_x = (float)mi.rcWork.left;
        info->work_y = (float)mi.rcWork.top;
        info->work_width = (float)(mi.rcWork.right - mi.rcWork.left);
        info->work_height = (float)(mi.rcWork.bottom - mi.rcWork.top);
        info->scale = scale;
        info->units_per_point = scale;
        info->primary = (mi.dwFlags & MONITORINFOF_PRIMARY) != 0;
    }
    walk->count++;
    return TRUE;
}

int zant_screen_list(ZantScreenInfo *out, int capacity) {
    ZantScreenWalk walk = {out, capacity, 0};
    EnumDisplayMonitors(NULL, NULL, zant_screen_walk, (LPARAM)&walk);
    return walk.count;
}

/* ---- windows ---- */

typedef struct {
    ZantWindowInfo *out;
    int capacity, count;
} ZantWindowWalk;

static void zant_process_name(DWORD pid, char *out, size_t capacity) {
    out[0] = 0;
    HANDLE process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
    if (!process) return;
    WCHAR path[MAX_PATH];
    DWORD length = MAX_PATH;
    if (QueryFullProcessImageNameW(process, 0, path, &length)) {
        const WCHAR *name = path;
        for (const WCHAR *c = path; *c; c++) {
            if (*c == L'\\' || *c == L'/') name = c + 1;
        }
        WCHAR stem[MAX_PATH];
        wcsncpy(stem, name, MAX_PATH - 1);
        stem[MAX_PATH - 1] = 0;
        WCHAR *dot = wcsrchr(stem, L'.');
        if (dot) *dot = 0;
        WideCharToMultiByte(CP_UTF8, 0, stem, -1, out, (int)capacity, NULL, NULL);
        out[capacity - 1] = 0;
    }
    CloseHandle(process);
}

static BOOL CALLBACK zant_window_walk(HWND hwnd, LPARAM data) {
    ZantWindowWalk *walk = (ZantWindowWalk *)data;
    if (!IsWindowVisible(hwnd) || IsIconic(hwnd)) return TRUE;
    int cloaked = 0;
    if (SUCCEEDED(DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &cloaked, sizeof(cloaked))) && cloaked) return TRUE;
    const LONG_PTR ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
    WCHAR title[2];
    const int has_title = GetWindowTextW(hwnd, title, 2) > 0;
    if ((ex & WS_EX_TOOLWINDOW) && !has_title) return TRUE;
    if ((ex & WS_EX_TRANSPARENT) && (ex & WS_EX_LAYERED)) return TRUE;
    WCHAR cls[64];
    if (GetClassNameW(hwnd, cls, 64)) {
        if (!wcscmp(cls, L"Progman") || !wcscmp(cls, L"WorkerW") || !wcscmp(cls, L"Shell_TrayWnd") ||
            !wcscmp(cls, L"Shell_SecondaryTrayWnd"))
            return TRUE;
    }
    RECT r;
    if (FAILED(DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, &r, sizeof(r))) && !GetWindowRect(hwnd, &r))
        return TRUE;
    const float min = 40.0f * (float)GetDpiForWindow(hwnd) / 96.0f;
    if ((float)(r.right - r.left) < min || (float)(r.bottom - r.top) < min) return TRUE;
    if (walk->out && walk->count < walk->capacity) {
        ZantWindowInfo *info = &walk->out[walk->count];
        info->x = (float)r.left;
        info->y = (float)r.top;
        info->width = (float)(r.right - r.left);
        info->height = (float)(r.bottom - r.top);
        DWORD pid = 0;
        GetWindowThreadProcessId(hwnd, &pid);
        info->pid = pid;
        zant_process_name(pid, info->app, sizeof(info->app));
    }
    walk->count++;
    return TRUE;
}

int zant_window_list(ZantWindowInfo *out, int capacity) {
    /* EnumWindows walks top-level windows in z-order, front first */
    ZantWindowWalk walk = {out, capacity, 0};
    EnumWindows(zant_window_walk, (LPARAM)&walk);
    return walk.count;
}

/* ---- capture ---- */

int zant_screen_capture_access(void) {
    return 1;
}

int zant_screen_capture_request(void) {
    return 1;
}

int zant_screen_capture_requested(void) {
    return 1;
}

/* no screen recording permission on Windows */
void zant_open_screen_capture_settings(void) {
}

void zant_capture_free(uint8_t *pixels) {
    free(pixels);
}

int zant_screen_capture_all(ZantCapture *out, int capacity) {
    if (!out || capacity <= 0) return 0;
    ZantScreenInfo screens[16];
    int count = zant_screen_list(screens, 16);
    if (count > capacity) count = capacity;
    if (count > 16) count = 16;
    if (count <= 0) return 0;
    HDC screen = GetDC(NULL);
    if (!screen) return 0;
    HDC memory = CreateCompatibleDC(screen);
    int ok = 0;
    for (int i = 0; i < count; i++) {
        ZantCapture *slot = &out[i];
        slot->screen_id = screens[i].id;
        slot->pixels = NULL;
        slot->width = slot->height = slot->stride = 0;
        const int w = (int)screens[i].width, h = (int)screens[i].height;
        BITMAPINFO bmi = {0};
        bmi.bmiHeader.biSize = sizeof(bmi.bmiHeader);
        bmi.bmiHeader.biWidth = w;
        bmi.bmiHeader.biHeight = -h; /* top-down */
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;
        void *bits = NULL;
        HBITMAP dib = CreateDIBSection(screen, &bmi, DIB_RGB_COLORS, &bits, NULL, 0);
        if (!dib || !bits) {
            if (dib) DeleteObject(dib);
            continue;
        }
        HGDIOBJ old = SelectObject(memory, dib);
        const BOOL copied =
            BitBlt(memory, 0, 0, w, h, screen, (int)screens[i].x, (int)screens[i].y, SRCCOPY | CAPTUREBLT);
        SelectObject(memory, old);
        if (copied) {
            const size_t stride = (size_t)w * 4;
            uint8_t *pixels = malloc(stride * (size_t)h);
            if (pixels) {
                memcpy(pixels, bits, stride * (size_t)h);
                /* BGRX -> opaque BGRA (zant_image_from_pixels order on Windows) */
                for (size_t p = 3; p < stride * (size_t)h; p += 4) pixels[p] = 0xff;
                slot->pixels = pixels;
                slot->width = w;
                slot->height = h;
                slot->stride = (int)stride;
                ok++;
            }
        }
        DeleteObject(dib);
    }
    DeleteDC(memory);
    ReleaseDC(NULL, screen);
    return ok ? count : 0;
}

int zant_local_utc_offset(void) {
    TIME_ZONE_INFORMATION tz;
    const DWORD mode = GetTimeZoneInformation(&tz);
    LONG bias = tz.Bias;
    if (mode == TIME_ZONE_ID_DAYLIGHT) bias += tz.DaylightBias;
    else if (mode == TIME_ZONE_ID_STANDARD) bias += tz.StandardBias;
    return (int)(-bias * 60);
}

/* ---- text recognition: no built-in recognizer here ---- */

int zant_ocr_available(void) {
    return 0;
}

ZantOcrResult *zant_ocr_recognize(const uint8_t *pixels, int width, int height, int stride) {
    (void)pixels;
    (void)width;
    (void)height;
    (void)stride;
    return NULL;
}

void zant_ocr_free(ZantOcrResult *result) {
    (void)result;
}
