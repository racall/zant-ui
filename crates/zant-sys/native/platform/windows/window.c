/* Win32 backend of platform/platform.h: one HWND, Skia linked into the exe
 * (Direct3D 12 with a raster fallback presented through GDI), per-monitor
 * DPI (v2), IMM32 composition for CJK input methods, an app-drawn title bar
 * for ZANT_WINDOW_HIDDEN_TITLEBAR (DWM keeps the frame, shadow, snap and
 * resize borders), clipboard, cursors and the native dialogs.
 *
 * Frames are rendered from WM_PAINT: redraw requests invalidate the window,
 * so they are honoured by any message loop (ours, live resize, modal
 * dialogs), and input is always dispatched before the next frame because
 * WM_PAINT has the lowest priority. Animation frames are paced by the
 * swap chain (D3D12) or DwmFlush (raster). */
#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0A00 /* Windows 10 */
#endif
#ifndef WINVER
#define WINVER 0x0A00
#endif
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef UNICODE
#define UNICODE
#endif
#ifndef _UNICODE
#define _UNICODE
#endif
#define COBJMACROS
#include <windows.h>
#include <windowsx.h>
#include <dwmapi.h>
#include <imm.h>
#include <shellapi.h>
#include <commctrl.h>
#include <initguid.h>
#include <shobjidl.h>
#include <shlobj.h>
#include <oleidl.h>

#include <math.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "platform/platform.h"
#include "skia/skia_bridge.h"

#define ZANT_CLASS_NAME L"ZantWindow"
#define ZANT_TIMER_REDRAW 1
#define ZANT_TIMER_SNAPSHOT 2
#define ZANT_TIMER_EXIT 3
#define ZANT_TIMER_MODAL 4
/* app-drawn title bar height when the app does not say (Windows 11) */
#define ZANT_DEFAULT_CAPTION 32.0f

/* zant_window_post_wake: hands background results to the UI thread. */
#define ZANT_WM_WAKE (WM_APP + 7)
#define ZANT_WM_TRAY (WM_APP + 8)
/* zant_window_cover_screen, run on the next loop turn (wParam: screen index) */
#define ZANT_WM_COVER (WM_APP + 9)
/* a frame for a hidden window: Windows sends no WM_PAINT to it, yet the app
   still builds (first build, effects, tray, timers) while it sits in the tray */
#define ZANT_WM_HIDDEN_FRAME (WM_APP + 10)
/* animation frames of a hidden window: nothing is on screen, so slowly */
#define ZANT_HIDDEN_FRAME_MS 250
struct ZantWindow;
static void zant_cover_now(struct ZantWindow *window, int index);

struct ZantWindow {
    ZantHost host;
    ZantRenderer *renderer;
    /* frames of a hidden / minimized window paint into this 1x1 raster:
       nothing is shown, and the swap chain of a window that is not on screen
       is never presented */
    ZantRenderer *background_renderer;
    HWND hwnd;
    int style;
    bool custom_frame; /* hidden_titlebar: no system caption */
    bool resizable;
    bool movable_by_background;
    float titlebar_height;
    float min_width;
    float min_height;
    UINT dpi;
    float scale;
    int width_px;
    int height_px;
    bool closing;
    bool in_draw;
    bool animating;
    bool in_size_move;
    bool close_approved;
    bool suppress_char;
    bool tracking_mouse;
    bool tracking_nc;
    bool pointer_inside;
    bool active;
    bool edited;
    int cursor;
    WCHAR high_surrogate;
    uint32_t background;
    wchar_t title[512];
    wchar_t *autosave_path;
    char backend_name[64];
    char snapshot_path[1024];
    volatile LONG wake_pending;
    bool hidden_frame_pending;
    HWND restore_focus; /* overlay: the window that had the keyboard before it */
    ZantWindow *next;
};

/* The first window: closing it ends the app; dialogs are owned by it. */
static ZantWindow *g_main = NULL;
static ZantWindow *g_windows = NULL;

/* ---------- strings ---------- */

static wchar_t *zant_widen(const char *utf8) {
    if (!utf8) return NULL;
    const int n = MultiByteToWideChar(CP_UTF8, 0, utf8, -1, NULL, 0);
    if (n <= 0) return NULL;
    wchar_t *out = (wchar_t *)malloc((size_t)n * sizeof(wchar_t));
    if (!out) return NULL;
    MultiByteToWideChar(CP_UTF8, 0, utf8, -1, out, n);
    return out;
}

/* malloc'ed UTF-8 copy of a UTF-16 string (length in units, -1 = NUL terminated) */
static char *zant_narrow(const wchar_t *wide, int length, int *out_len) {
    if (!wide) return NULL;
    const int n = WideCharToMultiByte(CP_UTF8, 0, wide, length, NULL, 0, NULL, NULL);
    if (n < 0) return NULL;
    char *out = (char *)malloc((size_t)n + 1);
    if (!out) return NULL;
    if (n > 0) WideCharToMultiByte(CP_UTF8, 0, wide, length, out, n, NULL, NULL);
    out[n] = '\0';
    if (out_len) *out_len = (length < 0 && n > 0) ? n - 1 : n;
    return out;
}

/* ---------- persistent window placement ---------- */

/* The DPI of a monitor (shcore, Windows 8.1+); the system DPI before that. */
static UINT zant_monitor_dpi(HMONITOR monitor) {
    typedef HRESULT(WINAPI * GetDpiForMonitorFn)(HMONITOR, int, UINT *, UINT *);
    static GetDpiForMonitorFn get_dpi = NULL;
    static bool loaded = false;
    if (!loaded) {
        loaded = true;
        HMODULE shcore = LoadLibraryW(L"shcore.dll");
        get_dpi = shcore ? (GetDpiForMonitorFn)(void *)GetProcAddress(shcore, "GetDpiForMonitor") : NULL;
    }
    UINT dpi_x = 96, dpi_y = 96;
    if (!get_dpi || FAILED(get_dpi(monitor, 0 /* MDT_EFFECTIVE_DPI */, &dpi_x, &dpi_y))) return GetDpiForSystem();
    return dpi_x;
}

/* The display a placement was saved on. WINDOWPLACEMENT holds physical
   pixels of the display configuration at that time: restored after the
   resolution, the scale or the monitors changed (or from a record written in
   a remote / virtual session), it lands somewhere unrelated (a corner of a
   larger screen). The window is put back only on the same display. */
typedef struct ZantPlacementDisplay {
    UINT dpi;
    RECT work; /* screen coordinates */
} ZantPlacementDisplay;

static bool zant_window_display(HWND hwnd, ZantPlacementDisplay *out) {
    HMONITOR monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    MONITORINFO info = {sizeof(info)};
    if (!monitor || !GetMonitorInfoW(monitor, &info)) return false;
    out->dpi = zant_monitor_dpi(monitor);
    out->work = info.rcWork;
    return true;
}

static void zant_save_placement(ZantWindow *w) {
    if (!w || !w->autosave_path) return;
    WINDOWPLACEMENT placement = {0};
    placement.length = sizeof(placement);
    if (!GetWindowPlacement(w->hwnd, &placement)) return;
    ZantPlacementDisplay display = {0};
    if (!zant_window_display(w->hwnd, &display)) return;
    HKEY key;
    if (RegCreateKeyExW(HKEY_CURRENT_USER, w->autosave_path, 0, NULL, 0,
                        KEY_SET_VALUE, NULL, &key, NULL) == ERROR_SUCCESS) {
        RegSetValueExW(key, L"WindowPlacement", 0, REG_BINARY,
                       (const BYTE *)&placement, sizeof(placement));
        RegSetValueExW(key, L"WindowDisplay", 0, REG_BINARY,
                       (const BYTE *)&display, sizeof(display));
        RegCloseKey(key);
    }
}

typedef struct ZantPlacementMatch {
    ZantPlacementDisplay saved;
    bool found;
} ZantPlacementMatch;

static BOOL CALLBACK zant_placement_monitor(HMONITOR monitor, HDC dc, LPRECT rect, LPARAM data) {
    (void)dc;
    (void)rect;
    ZantPlacementMatch *match = (ZantPlacementMatch *)data;
    MONITORINFO info = {sizeof(info)};
    if (!GetMonitorInfoW(monitor, &info)) return TRUE;
    if (EqualRect(&info.rcWork, &match->saved.work) && zant_monitor_dpi(monitor) == match->saved.dpi) {
        match->found = true;
        return FALSE;
    }
    return TRUE;
}

void zant_window_set_autosave(ZantWindow *window, const char *name) {
    if (!window || !window->hwnd) return;
    wchar_t *path = NULL;
    if (name && name[0]) {
        wchar_t *wide = zant_widen(name);
        if (!wide) return;
        const wchar_t prefix[] = L"Software\\Zant\\";
        const size_t count = wcslen(prefix) + wcslen(wide) + 1;
        path = (wchar_t *)malloc(count * sizeof(wchar_t));
        if (path) {
            wcscpy(path, prefix);
            wcscat(path, wide);
        }
        free(wide);
        if (!path) return;
    }
    if (path && window->autosave_path && wcscmp(path, window->autosave_path) == 0) {
        free(path);
        return;
    }
    free(window->autosave_path);
    window->autosave_path = path;
    if (!path) return;
    WINDOWPLACEMENT placement = {0};
    DWORD size = sizeof(placement);
    if (RegGetValueW(HKEY_CURRENT_USER, path, L"WindowPlacement", RRF_RT_REG_BINARY,
                     NULL, &placement, &size) != ERROR_SUCCESS ||
        size != sizeof(placement) || placement.length != sizeof(placement) ||
        placement.rcNormalPosition.right <= placement.rcNormalPosition.left ||
        placement.rcNormalPosition.bottom <= placement.rcNormalPosition.top) return;
    /* no display recorded (older releases) or that display is gone / changed:
       keep the position the window was created with (centered by default) */
    ZantPlacementMatch match = {0};
    size = sizeof(match.saved);
    if (RegGetValueW(HKEY_CURRENT_USER, path, L"WindowDisplay", RRF_RT_REG_BINARY,
                     NULL, &match.saved, &size) != ERROR_SUCCESS ||
        size != sizeof(match.saved)) return;
    EnumDisplayMonitors(NULL, NULL, zant_placement_monitor, (LPARAM)&match);
    if (!match.found) return;
    /* Reopen minimized windows normally; restoring a hidden createWindow or a
       login window must not show it before the app requests it. */
    if (!IsWindowVisible(window->hwnd)) placement.showCmd = SW_HIDE;
    else if (placement.showCmd != SW_SHOWMAXIMIZED) placement.showCmd = SW_SHOWNORMAL;
    placement.flags = 0;
    SetWindowPlacement(window->hwnd, &placement);
}

/* ---------- geometry / DPI ---------- */

static float zant_logical_width(ZantWindow *w) {
    return (float)w->width_px / w->scale;
}

static float zant_logical_height(ZantWindow *w) {
    return (float)w->height_px / w->scale;
}

static void zant_update_client_size(ZantWindow *w) {
    RECT rc;
    GetClientRect(w->hwnd, &rc);
    w->width_px = rc.right - rc.left > 0 ? rc.right - rc.left : 1;
    w->height_px = rc.bottom - rc.top > 0 ? rc.bottom - rc.top : 1;
}

static void zant_set_dpi(ZantWindow *w, UINT dpi) {
    w->dpi = dpi ? dpi : 96;
    w->scale = (float)w->dpi / 96.0f;
}

static int zant_frame_x(ZantWindow *w) {
    return GetSystemMetricsForDpi(SM_CXFRAME, w->dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, w->dpi);
}

static int zant_frame_y(ZantWindow *w) {
    return GetSystemMetricsForDpi(SM_CYFRAME, w->dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, w->dpi);
}

static DWORD zant_window_style_bits(ZantWindow *w) {
    if (w->style == ZANT_WINDOW_BORDERLESS) return WS_POPUP | (w->resizable ? WS_THICKFRAME : 0) | WS_MINIMIZEBOX;
    /* panel: no caption; the thick frame (hidden by WM_NCCALCSIZE) gives the
       DWM shadow, DWMWA_WINDOW_CORNER_PREFERENCE the rounded corners */
    if (w->style == ZANT_WINDOW_PANEL) return WS_POPUP | WS_THICKFRAME;
    if (w->style == ZANT_WINDOW_OVERLAY) return WS_POPUP;
    DWORD style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
    if (w->resizable) style |= WS_THICKFRAME | WS_MAXIMIZEBOX;
    return style;
}

/* ---------- events ---------- */

static int zant_modifiers(void) {
    int mods = 0;
    if (GetKeyState(VK_SHIFT) < 0) mods |= ZANT_MOD_SHIFT;
    if (GetKeyState(VK_CONTROL) < 0) mods |= ZANT_MOD_CONTROL;
    if (GetKeyState(VK_MENU) < 0) mods |= ZANT_MOD_ALT;
    if (GetKeyState(VK_LWIN) < 0 || GetKeyState(VK_RWIN) < 0) mods |= ZANT_MOD_META;
    if (GetKeyState(VK_CAPITAL) & 1) mods |= ZANT_MOD_CAPS_LOCK;
    return mods;
}

static ZantEvent zant_event(ZantWindow *w, int type) {
    ZantEvent e;
    memset(&e, 0, sizeof(e));
    e.type = type;
    e.width = (int)lroundf(zant_logical_width(w));
    e.height = (int)lroundf(zant_logical_height(w));
    e.dpr = w->scale;
    return e;
}

/* Hidden or minimized: Windows sends it no WM_PAINT. */
static bool zant_backgrounded(ZantWindow *w) {
    return !IsWindowVisible(w->hwnd) || IsIconic(w->hwnd);
}

static void zant_request(ZantWindow *w) {
    if (!w || !w->hwnd || w->closing) return;
    if (!zant_backgrounded(w)) {
        InvalidateRect(w->hwnd, NULL, FALSE);
    } else if (!w->hidden_frame_pending) {
        w->hidden_frame_pending = true;
        if (!PostMessageW(w->hwnd, ZANT_WM_HIDDEN_FRAME, 0, 0)) w->hidden_frame_pending = false;
    }
}

static int zant_send(ZantWindow *w, ZantEvent *e) {
    if (!w || !w->host.event) return 0;
    const int handled = w->host.event(w->host.userdata, e);
    if (!handled) return handled;
    /* A click or a key press shows its result in the next frame now:
       WM_PAINT is the lowest-priority message and waits behind other input,
       which made fast clicking visibly lag. Pointer moves keep the coalescing
       invalidate (they arrive far faster than frames). */
    const bool discrete = e->type == ZANT_EVENT_POINTER_DOWN || e->type == ZANT_EVENT_POINTER_UP ||
                          e->type == ZANT_EVENT_KEY_DOWN || e->type == ZANT_EVENT_KEY_UP ||
                          e->type == ZANT_EVENT_TEXT_INPUT;
    if (discrete && w->hwnd && !w->closing && !w->in_draw) {
        RedrawWindow(w->hwnd, NULL, NULL, RDW_INVALIDATE | RDW_UPDATENOW);
    } else {
        zant_request(w);
    }
    return handled;
}

static void zant_send_simple(ZantWindow *w, int type) {
    ZantEvent e = zant_event(w, type);
    zant_send(w, &e);
}

static void zant_send_pointer(ZantWindow *w, int type, int button, int x_px, int y_px) {
    ZantEvent e = zant_event(w, type);
    e.x = (float)x_px / w->scale;
    e.y = (float)y_px / w->scale;
    e.button = button;
    e.modifiers = zant_modifiers();
    zant_send(w, &e);
}

/* Zant key code: named keys, else the unshifted character, lowercased
   (what AppKit's charactersIgnoringModifiers gives for letters). */
static int zant_key_from_vk(WPARAM vk) {
    switch (vk) {
        case VK_LEFT: return ZANT_KEY_LEFT;
        case VK_RIGHT: return ZANT_KEY_RIGHT;
        case VK_UP: return ZANT_KEY_UP;
        case VK_DOWN: return ZANT_KEY_DOWN;
        case VK_HOME: return ZANT_KEY_HOME;
        case VK_END: return ZANT_KEY_END;
        case VK_PRIOR: return ZANT_KEY_PAGE_UP;
        case VK_NEXT: return ZANT_KEY_PAGE_DOWN;
        case VK_BACK: return ZANT_KEY_BACKSPACE;
        case VK_DELETE: return ZANT_KEY_DELETE;
        case VK_ESCAPE: return ZANT_KEY_ESCAPE;
        case VK_TAB: return ZANT_KEY_TAB;
        case VK_RETURN: return ZANT_KEY_ENTER;
        case VK_SPACE: return ' ';
        case VK_ADD: return '+';
        case VK_SUBTRACT: return '-';
        case VK_MULTIPLY: return '*';
        case VK_DIVIDE: return '/';
        case VK_DECIMAL: return '.';
        default: break;
    }
    if (vk >= VK_F1 && vk <= VK_F12) return ZANT_KEY_F1 + (int)(vk - VK_F1);
    if (vk >= 'A' && vk <= 'Z') return (int)(vk - 'A' + 'a');
    if (vk >= '0' && vk <= '9') return (int)vk;
    if (vk >= VK_NUMPAD0 && vk <= VK_NUMPAD9) return (int)('0' + (vk - VK_NUMPAD0));
    const UINT ch = MapVirtualKeyW((UINT)vk, MAPVK_VK_TO_CHAR) & 0x7FFF;
    if (ch >= 'A' && ch <= 'Z') return (int)(ch - 'A' + 'a');
    if (ch >= 0x20) return (int)ch;
    return 0;
}

/* ---------- rendering ---------- */

static void zant_present_raster(ZantWindow *w, HDC dc) {
    int width = 0, height = 0, stride = 0;
    void *pixels = zant_renderer_pixels(w->renderer, &width, &height, &stride);
    if (!pixels || width <= 0 || height <= 0 || stride <= 0) return;
    BITMAPINFO bmi;
    memset(&bmi, 0, sizeof(bmi));
    bmi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    bmi.bmiHeader.biWidth = stride / 4;
    bmi.bmiHeader.biHeight = -height; /* top-down */
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB; /* BGRA: Skia's native order on Windows */
    SetDIBitsToDevice(dc, 0, 0, (DWORD)width, (DWORD)height, 0, 0, 0, (UINT)height, pixels, &bmi,
                      DIB_RGB_COLORS);
}

static void zant_schedule_next(ZantWindow *w) {
    if (!w->host.next_frame_delay || w->closing) return;
    const int delay = w->host.next_frame_delay(w->host.userdata);
    w->animating = delay > 0 && delay <= 16;
    KillTimer(w->hwnd, ZANT_TIMER_REDRAW);
    if (w->animating && zant_backgrounded(w)) {
        SetTimer(w->hwnd, ZANT_TIMER_REDRAW, ZANT_HIDDEN_FRAME_MS, NULL);
    } else if (w->animating) {
        zant_request(w);
    } else if (delay > 0) {
        SetTimer(w->hwnd, ZANT_TIMER_REDRAW, (UINT)delay, NULL);
    } else if (delay == 0) {
        zant_request(w);
    }
}

static void zant_render(ZantWindow *w, HDC dc) {
    if (!w || w->closing || w->in_draw || !w->host.draw || !w->renderer) return;
    const bool background = zant_backgrounded(w);
    ZantRenderer *renderer = w->renderer;
    if (background) {
        /* one pixel: the frame builds and lays out at the window's size, and
           its paint is clipped away (a hidden full-screen overlay costs
           nothing to keep) */
        if (!w->background_renderer) w->background_renderer = zant_renderer_create_raster(1, 1);
        if (!w->background_renderer) return;
        renderer = w->background_renderer;
    }
    w->in_draw = true;
    /* a minimized window has no client area: it keeps its restored size */
    if (!IsIconic(w->hwnd)) zant_update_client_size(w);
    if (!background) zant_renderer_resize(renderer, w->width_px, w->height_px);
    ZantCanvas *canvas = zant_renderer_begin(renderer, w->scale);
    if (canvas) {
        w->host.draw(w->host.userdata, canvas, zant_logical_width(w), zant_logical_height(w), w->scale);
    }
    zant_renderer_end(renderer);
    const bool gpu = zant_renderer_is_gpu(w->renderer);
    if (!gpu && !background) {
        HDC target = dc ? dc : GetDC(w->hwnd);
        zant_present_raster(w, target);
        if (!dc) ReleaseDC(w->hwnd, target);
    }
    w->in_draw = false;
    zant_schedule_next(w);
    /* raster animation frames wait for the next composition (vsync) */
    if (w->animating && !gpu && !w->in_size_move && !background) DwmFlush();
}

/* ---------- title bar ---------- */

static float zant_caption_height(ZantWindow *w) {
    return w->titlebar_height > 0 ? w->titlebar_height : ZANT_DEFAULT_CAPTION;
}

/* Hit test of the app-drawn title bar / movable background (client px). */
static LRESULT zant_custom_hit(ZantWindow *w, int x, int y) {
    const bool maximized = IsZoomed(w->hwnd);
    if (w->resizable && !maximized && w->custom_frame) {
        const int border = zant_frame_y(w);
        if (y < border) {
            if (x < border * 2) return HTTOPLEFT;
            if (x >= w->width_px - border * 2) return HTTOPRIGHT;
            return HTTOP;
        }
    }
    const float lx = (float)x / w->scale;
    const float ly = (float)y / w->scale;
    const bool in_bar = w->custom_frame && ly < zant_caption_height(w);
    if (!in_bar && !w->movable_by_background) return HTCLIENT;
    int hit = ZANT_HIT_CAPTION;
    if (w->host.titlebar_hit) hit = w->host.titlebar_hit(w->host.userdata, lx, ly);
    if (hit == ZANT_HIT_MAXIMIZE) return HTMAXBUTTON;
    if (hit == ZANT_HIT_CAPTION) return HTCAPTION;
    return HTCLIENT;
}

static void zant_apply_dark_frame(ZantWindow *w) {
    const uint32_t bg = w->background;
    const float r = (float)((bg >> 16) & 0xff), g = (float)((bg >> 8) & 0xff), b = (float)(bg & 0xff);
    const BOOL dark = (0.2126f * r + 0.7152f * g + 0.0722f * b) < 128.0f;
    DwmSetWindowAttribute(w->hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark, sizeof(dark));
    /* Windows 11: system caption (native style) in the window colour */
    const COLORREF caption = RGB((bg >> 16) & 0xff, (bg >> 8) & 0xff, bg & 0xff);
    DwmSetWindowAttribute(w->hwnd, DWMWA_CAPTION_COLOR, &caption, sizeof(caption));
}

static void zant_update_title(ZantWindow *w) {
    if (!w->hwnd) return;
    if (!w->edited) {
        SetWindowTextW(w->hwnd, w->title);
        return;
    }
    wchar_t buffer[520];
    _snwprintf(buffer, sizeof(buffer) / sizeof(buffer[0]) - 1, L"*%ls", w->title);
    buffer[sizeof(buffer) / sizeof(buffer[0]) - 1] = 0;
    SetWindowTextW(w->hwnd, buffer);
}

/* ---------- IME (IMM32) ---------- */

static void zant_ime_position(ZantWindow *w) {
    HIMC imc = ImmGetContext(w->hwnd);
    if (!imc) return;
    float x = zant_logical_width(w) * 0.5f, y = zant_logical_height(w) * 0.5f, cw = 1, ch = 18;
    if (w->host.ime_rect) {
        if (!w->host.ime_rect(w->host.userdata, &x, &y, &cw, &ch)) {
            x = zant_logical_width(w) * 0.5f;
            y = zant_logical_height(w) * 0.5f;
        }
    }
    const LONG px = (LONG)lroundf(x * w->scale);
    const LONG py = (LONG)lroundf(y * w->scale);
    const LONG pw = (LONG)lroundf(fmaxf(cw, 1) * w->scale);
    const LONG ph = (LONG)lroundf(fmaxf(ch, 1) * w->scale);
    COMPOSITIONFORM composition;
    memset(&composition, 0, sizeof(composition));
    composition.dwStyle = CFS_POINT;
    composition.ptCurrentPos.x = px;
    composition.ptCurrentPos.y = py;
    ImmSetCompositionWindow(imc, &composition);
    CANDIDATEFORM candidate;
    memset(&candidate, 0, sizeof(candidate));
    candidate.dwIndex = 0;
    candidate.dwStyle = CFS_EXCLUDE;
    candidate.ptCurrentPos.x = px;
    candidate.ptCurrentPos.y = py + ph;
    candidate.rcArea.left = px;
    candidate.rcArea.top = py;
    candidate.rcArea.right = px + pw;
    candidate.rcArea.bottom = py + ph;
    ImmSetCandidateWindow(imc, &candidate);
    ImmReleaseContext(w->hwnd, imc);
}

static void zant_send_text(ZantWindow *w, int type, const wchar_t *text, int units, size_t sel_start,
                           size_t sel_end) {
    int len = 0;
    char *utf8 = zant_narrow(text ? text : L"", units, &len);
    if (!utf8) return;
    ZantEvent e = zant_event(w, type);
    e.text = utf8;
    e.text_len = (size_t)len;
    e.selection_start = sel_start;
    e.selection_end = sel_end;
    if (w->host.event) w->host.event(w->host.userdata, &e);
    free(utf8);
    zant_request(w);
}

static wchar_t *zant_ime_string(HIMC imc, DWORD kind, int *units) {
    const LONG bytes = ImmGetCompositionStringW(imc, kind, NULL, 0);
    *units = 0;
    if (bytes <= 0) return NULL;
    wchar_t *buf = (wchar_t *)malloc((size_t)bytes + sizeof(wchar_t));
    if (!buf) return NULL;
    ImmGetCompositionStringW(imc, kind, buf, (DWORD)bytes);
    *units = (int)(bytes / (LONG)sizeof(wchar_t));
    buf[*units] = 0;
    return buf;
}

static void zant_ime_composition(ZantWindow *w, LPARAM flags) {
    HIMC imc = ImmGetContext(w->hwnd);
    if (!imc) return;
    if (flags & GCS_RESULTSTR) {
        int units = 0;
        wchar_t *result = zant_ime_string(imc, GCS_RESULTSTR, &units);
        if (result && units > 0) zant_send_text(w, ZANT_EVENT_TEXT_INPUT, result, units, 0, 0);
        free(result);
    }
    if (flags & GCS_COMPSTR) {
        int units = 0;
        wchar_t *marked = zant_ime_string(imc, GCS_COMPSTR, &units);
        LONG cursor = ImmGetCompositionStringW(imc, GCS_CURSORPOS, NULL, 0);
        if (cursor < 0 || cursor > units) cursor = units;
        zant_send_text(w, ZANT_EVENT_COMPOSITION, marked, units, (size_t)cursor, (size_t)cursor);
        free(marked);
    } else if (!(flags & GCS_RESULTSTR)) {
        /* composition cancelled */
        zant_send_text(w, ZANT_EVENT_COMPOSITION, L"", 0, 0, 0);
    }
    ImmReleaseContext(w->hwnd, imc);
    zant_ime_position(w);
}

/* ---------- snapshot ---------- */

static void zant_take_snapshot(ZantWindow *w) {
    if (!w->renderer || !w->snapshot_path[0]) return;
    if (zant_renderer_is_gpu(w->renderer)) {
        zant_renderer_capture_next_png(w->renderer, w->snapshot_path);
        zant_render(w, NULL);
        fprintf(stderr, "[zant] snapshot: %s\n", w->snapshot_path);
    } else {
        zant_render(w, NULL);
        if (zant_renderer_write_png(w->renderer, w->snapshot_path)) {
            fprintf(stderr, "[zant] snapshot: %s\n", w->snapshot_path);
        } else {
            fprintf(stderr, "[zant] snapshot failed: %s\n", w->snapshot_path);
        }
    }
    fflush(stderr);
    const char *exit_flag = getenv("ZANT_SNAPSHOT_EXIT");
    if (exit_flag && exit_flag[0] == '1') SetTimer(w->hwnd, ZANT_TIMER_EXIT, 400, NULL);
}

/* ---------- window procedure ---------- */

static void zant_track_leave(ZantWindow *w, bool nonclient) {
    TRACKMOUSEEVENT tme;
    memset(&tme, 0, sizeof(tme));
    tme.cbSize = sizeof(tme);
    tme.dwFlags = TME_LEAVE | (nonclient ? TME_NONCLIENT : 0);
    tme.hwndTrack = w->hwnd;
    TrackMouseEvent(&tme);
    if (nonclient) w->tracking_nc = true;
    else w->tracking_mouse = true;
}

static void zant_pointer_move(ZantWindow *w, int x, int y) {
    if (!w->pointer_inside) {
        w->pointer_inside = true;
        zant_send_pointer(w, ZANT_EVENT_POINTER_ENTER, ZANT_POINTER_BUTTON_LEFT, x, y);
    }
    zant_send_pointer(w, ZANT_EVENT_POINTER_MOVE, ZANT_POINTER_BUTTON_LEFT, x, y);
}

static void zant_pointer_leave(ZantWindow *w) {
    if (!w->pointer_inside) return;
    w->pointer_inside = false;
    ZantEvent e = zant_event(w, ZANT_EVENT_POINTER_LEAVE);
    e.x = -1;
    e.y = -1;
    zant_send(w, &e);
}

static bool zant_close_vetoed(ZantWindow *w) {
    if (w->close_approved || w->closing) return false;
    ZantEvent e = zant_event(w, ZANT_EVENT_WINDOW_CLOSE_REQUEST);
    const int veto = w->host.event ? w->host.event(w->host.userdata, &e) : 0;
    zant_request(w);
    if (veto) return true;
    w->close_approved = true;
    return false;
}

static HCURSOR zant_cursor_handle(int cursor) {
    switch (cursor) {
        case ZANT_CURSOR_POINTING_HAND: return LoadCursorW(NULL, IDC_HAND);
        case ZANT_CURSOR_IBEAM: return LoadCursorW(NULL, IDC_IBEAM);
        case ZANT_CURSOR_CROSSHAIR: return LoadCursorW(NULL, IDC_CROSS);
        case ZANT_CURSOR_RESIZE_HORIZONTAL: return LoadCursorW(NULL, IDC_SIZEWE);
        case ZANT_CURSOR_RESIZE_VERTICAL: return LoadCursorW(NULL, IDC_SIZENS);
        case ZANT_CURSOR_GRAB:
        case ZANT_CURSOR_GRABBING: return LoadCursorW(NULL, IDC_SIZEALL);
        case ZANT_CURSOR_NOT_ALLOWED: return LoadCursorW(NULL, IDC_NO);
        default: return LoadCursorW(NULL, IDC_ARROW);
    }
}

static LRESULT CALLBACK zant_wndproc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
    ZantWindow *w = (ZantWindow *)GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if (!w) return DefWindowProcW(hwnd, msg, wp, lp);

    switch (msg) {
        case ZANT_WM_COVER:
            zant_cover_now(w, (int)wp);
            return 0;

        case WM_NCCALCSIZE:
            /* panel: the whole window is client area */
            if (wp && w->style == ZANT_WINDOW_PANEL) return 0;
            if (wp && w->custom_frame) {
                /* keep the system's left/right/bottom resize borders, drop the
                   caption (Windows Terminal's approach) */
                NCCALCSIZE_PARAMS *params = (NCCALCSIZE_PARAMS *)lp;
                const LONG top = params->rgrc[0].top;
                const LRESULT result = DefWindowProcW(hwnd, msg, wp, lp);
                params->rgrc[0].top = top;
                if (IsZoomed(hwnd)) params->rgrc[0].top += zant_frame_y(w);
                return result;
            }
            break;

        case WM_NCHITTEST:
            if (w->style == ZANT_WINDOW_PANEL || w->style == ZANT_WINDOW_OVERLAY) return HTCLIENT;
            if (w->custom_frame || w->movable_by_background || w->style == ZANT_WINDOW_BORDERLESS) {
                LRESULT hit = DefWindowProcW(hwnd, msg, wp, lp);
                if (hit != HTCLIENT) return hit;
                POINT pt = {GET_X_LPARAM(lp), GET_Y_LPARAM(lp)};
                ScreenToClient(hwnd, &pt);
                return zant_custom_hit(w, pt.x, pt.y);
            }
            break;

        /* the app-drawn maximize button keeps HTMAXBUTTON (Win11 snap
           layouts flyout) but is still a Zant button: forward the pointer */
        case WM_NCMOUSEMOVE:
            if (w->custom_frame) {
                POINT pt = {GET_X_LPARAM(lp), GET_Y_LPARAM(lp)};
                ScreenToClient(hwnd, &pt);
                if (!w->tracking_nc) zant_track_leave(w, true);
                if (wp == HTMAXBUTTON || wp == HTCAPTION) zant_pointer_move(w, pt.x, pt.y);
                else zant_pointer_leave(w);
            }
            break;
        case WM_NCMOUSELEAVE:
            w->tracking_nc = false;
            if (!w->tracking_mouse) zant_pointer_leave(w);
            break;
        case WM_NCLBUTTONDOWN:
        case WM_NCLBUTTONDBLCLK:
            if (w->custom_frame && wp == HTMAXBUTTON) {
                POINT pt = {GET_X_LPARAM(lp), GET_Y_LPARAM(lp)};
                ScreenToClient(hwnd, &pt);
                zant_send_pointer(w, ZANT_EVENT_POINTER_DOWN, ZANT_POINTER_BUTTON_LEFT, pt.x, pt.y);
                return 0;
            }
            break;
        case WM_NCLBUTTONUP:
            if (w->custom_frame && wp == HTMAXBUTTON) {
                POINT pt = {GET_X_LPARAM(lp), GET_Y_LPARAM(lp)};
                ScreenToClient(hwnd, &pt);
                zant_send_pointer(w, ZANT_EVENT_POINTER_UP, ZANT_POINTER_BUTTON_LEFT, pt.x, pt.y);
                return 0;
            }
            break;

        case WM_ERASEBKGND:
            return 1;

        case WM_PAINT: {
            PAINTSTRUCT ps;
            HDC dc = BeginPaint(hwnd, &ps);
            zant_render(w, dc);
            EndPaint(hwnd, &ps);
            return 0;
        }

        case ZANT_WM_HIDDEN_FRAME:
            w->hidden_frame_pending = false;
            if (zant_backgrounded(w)) zant_render(w, NULL);
            return 0;

        case ZANT_WM_WAKE:
            InterlockedExchange(&w->wake_pending, 0);
            if (!w->closing && w->host.wake) w->host.wake(w->host.userdata);
            zant_request(w);
            return 0;

        case WM_TIMER:
            if (wp == ZANT_TIMER_REDRAW) {
                KillTimer(hwnd, ZANT_TIMER_REDRAW);
                zant_request(w);
            } else if (wp == ZANT_TIMER_SNAPSHOT) {
                KillTimer(hwnd, ZANT_TIMER_SNAPSHOT);
                zant_take_snapshot(w);
            } else if (wp == ZANT_TIMER_EXIT) {
                KillTimer(hwnd, ZANT_TIMER_EXIT);
                w->close_approved = true;
                DestroyWindow(hwnd);
            } else if (wp == ZANT_TIMER_MODAL) {
                /* live resize / move runs its own loop: keep animations going */
                if (w->animating) {
                    InvalidateRect(hwnd, NULL, FALSE);
                    UpdateWindow(hwnd);
                }
            }
            return 0;

        case WM_ENTERSIZEMOVE:
            w->in_size_move = true;
            SetTimer(hwnd, ZANT_TIMER_MODAL, 16, NULL);
            return 0;
        case WM_EXITSIZEMOVE:
            w->in_size_move = false;
            KillTimer(hwnd, ZANT_TIMER_MODAL);
            /* remembered as soon as the user moved it: a process ended by
               sign-out or shutdown never reaches WM_CLOSE */
            zant_save_placement(w);
            zant_request(w);
            return 0;

        case WM_SIZE: {
            /* minimized: the client area is empty, the layout keeps its size */
            if (wp == SIZE_MINIMIZED) return 0;
            const int previous_w = w->width_px, previous_h = w->height_px;
            zant_update_client_size(w);
            if (wp != SIZE_MINIMIZED && (previous_w != w->width_px || previous_h != w->height_px)) {
                zant_send_simple(w, ZANT_EVENT_WINDOW_RESIZE);
                /* draw now so live resize never shows stretched content */
                InvalidateRect(hwnd, NULL, FALSE);
                UpdateWindow(hwnd);
            } else if (wp == SIZE_MAXIMIZED || wp == SIZE_RESTORED) {
                zant_send_simple(w, ZANT_EVENT_WINDOW_RESIZE);
            }
            return 0;
        }

        case WM_DPICHANGED: {
            zant_set_dpi(w, HIWORD(wp));
            if (w->style == ZANT_WINDOW_OVERLAY) {
                /* keeps covering its monitor (zant_window_cover_screen) */
                zant_update_client_size(w);
                zant_send_simple(w, ZANT_EVENT_WINDOW_RESIZE);
                zant_request(w);
                return 0;
            }
            const RECT *suggested = (const RECT *)lp;
            SetWindowPos(hwnd, NULL, suggested->left, suggested->top, suggested->right - suggested->left,
                         suggested->bottom - suggested->top, SWP_NOZORDER | SWP_NOACTIVATE);
            zant_update_client_size(w);
            zant_send_simple(w, ZANT_EVENT_WINDOW_RESIZE);
            zant_request(w);
            return 0;
        }

        case WM_GETMINMAXINFO: {
            MINMAXINFO *info = (MINMAXINFO *)lp;
            RECT rc = {0, 0, (LONG)lroundf(w->min_width * w->scale), (LONG)lroundf(w->min_height * w->scale)};
            if (!w->custom_frame) {
                AdjustWindowRectExForDpi(&rc, zant_window_style_bits(w), FALSE, 0, w->dpi);
            } else {
                rc.right += zant_frame_x(w) * 2;
                rc.bottom += zant_frame_y(w);
            }
            info->ptMinTrackSize.x = rc.right - rc.left;
            info->ptMinTrackSize.y = rc.bottom - rc.top;
            return 0;
        }

        case WM_ACTIVATE: {
            const bool active = LOWORD(wp) != WA_INACTIVE;
            if (active != w->active) {
                w->active = active;
                zant_send_simple(w, active ? ZANT_EVENT_WINDOW_FOCUS : ZANT_EVENT_WINDOW_BLUR);
                zant_request(w);
            }
            break;
        }

        case WM_SETCURSOR:
            if (LOWORD(lp) == HTCLIENT) {
                SetCursor(zant_cursor_handle(w->cursor));
                return TRUE;
            }
            break;

        case WM_MOUSEMOVE:
            if (!w->tracking_mouse) zant_track_leave(w, false);
            zant_pointer_move(w, GET_X_LPARAM(lp), GET_Y_LPARAM(lp));
            return 0;
        case WM_MOUSELEAVE:
            w->tracking_mouse = false;
            zant_pointer_leave(w);
            return 0;

        case WM_LBUTTONDOWN:
        case WM_RBUTTONDOWN:
        case WM_MBUTTONDOWN: {
            const int button = msg == WM_LBUTTONDOWN ? ZANT_POINTER_BUTTON_LEFT
                               : msg == WM_RBUTTONDOWN ? ZANT_POINTER_BUTTON_RIGHT
                                                       : ZANT_POINTER_BUTTON_MIDDLE;
            SetFocus(hwnd);
            SetCapture(hwnd);
            zant_send_pointer(w, ZANT_EVENT_POINTER_DOWN, button, GET_X_LPARAM(lp), GET_Y_LPARAM(lp));
            return 0;
        }
        case WM_LBUTTONUP:
        case WM_RBUTTONUP:
        case WM_MBUTTONUP: {
            const int button = msg == WM_LBUTTONUP ? ZANT_POINTER_BUTTON_LEFT
                               : msg == WM_RBUTTONUP ? ZANT_POINTER_BUTTON_RIGHT
                                                     : ZANT_POINTER_BUTTON_MIDDLE;
            if (!(wp & (MK_LBUTTON | MK_RBUTTON | MK_MBUTTON))) ReleaseCapture();
            zant_send_pointer(w, ZANT_EVENT_POINTER_UP, button, GET_X_LPARAM(lp), GET_Y_LPARAM(lp));
            return 0;
        }

        case WM_MOUSEWHEEL:
        case WM_MOUSEHWHEEL: {
            POINT pt = {GET_X_LPARAM(lp), GET_Y_LPARAM(lp)};
            ScreenToClient(hwnd, &pt);
            UINT lines = 3;
            SystemParametersInfoW(SPI_GETWHEELSCROLLLINES, 0, &lines, 0);
            if (lines == WHEEL_PAGESCROLL || lines == 0) lines = 3;
            /* one notch = `lines` lines of ~20pt; precision touchpads send
               fractions of a notch */
            const float notches = (float)GET_WHEEL_DELTA_WPARAM(wp) / (float)WHEEL_DELTA;
            const float distance = notches * (float)lines * 20.0f;
            ZantEvent e = zant_event(w, ZANT_EVENT_SCROLL);
            e.x = (float)pt.x / w->scale;
            e.y = (float)pt.y / w->scale;
            e.modifiers = zant_modifiers();
            if (msg == WM_MOUSEWHEEL) {
                if (e.modifiers & ZANT_MOD_SHIFT) e.scroll_x = distance;
                else e.scroll_y = distance;
            } else {
                e.scroll_x = -distance;
            }
            zant_send(w, &e);
            return 0;
        }

        case WM_KEYDOWN:
        case WM_SYSKEYDOWN: {
            w->suppress_char = false;
            if (wp == VK_PROCESSKEY) break; /* the IME is handling it */
            if (wp == VK_SHIFT || wp == VK_CONTROL || wp == VK_MENU || wp == VK_LWIN || wp == VK_RWIN ||
                wp == VK_CAPITAL) {
                break;
            }
            ZantEvent e = zant_event(w, ZANT_EVENT_KEY_DOWN);
            e.key = zant_key_from_vk(wp);
            e.modifiers = zant_modifiers();
            e.is_repeat = (lp & (1 << 30)) ? 1 : 0;
            if (e.key != 0 && zant_send(w, &e)) {
                w->suppress_char = true;
                return 0;
            }
            /* Ctrl+W closes the window like the AppKit menu's ⌘W */
            if (msg == WM_KEYDOWN && wp == 'W' && (e.modifiers & ZANT_MOD_CONTROL) &&
                !(e.modifiers & (ZANT_MOD_ALT | ZANT_MOD_SHIFT))) {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
                w->suppress_char = true;
                return 0;
            }
            break;
        }
        case WM_KEYUP:
        case WM_SYSKEYUP: {
            ZantEvent e = zant_event(w, ZANT_EVENT_KEY_UP);
            e.key = zant_key_from_vk(wp);
            e.modifiers = zant_modifiers();
            if (e.key != 0 && w->host.event) w->host.event(w->host.userdata, &e);
            break;
        }
        case WM_CHAR: {
            if (w->suppress_char) return 0;
            const WCHAR c = (WCHAR)wp;
            if (c >= 0xD800 && c <= 0xDBFF) {
                w->high_surrogate = c;
                return 0;
            }
            WCHAR units[2];
            int count = 0;
            if (c >= 0xDC00 && c <= 0xDFFF) {
                if (!w->high_surrogate) return 0;
                units[count++] = w->high_surrogate;
                units[count++] = c;
                w->high_surrogate = 0;
            } else {
                if (c < 0x20 || c == 0x7F) return 0; /* Enter, Tab, Ctrl+X... are keys */
                units[count++] = c;
            }
            zant_send_text(w, ZANT_EVENT_TEXT_INPUT, units, count, 0, 0);
            return 0;
        }
        case WM_SYSCHAR:
            if (wp == ' ') break; /* Alt+Space: system menu */
            return 0;             /* no beep for unhandled Alt+key */
        case WM_SYSCOMMAND:
            /* a lone Alt / F10 would enter menu mode (there is no menu) */
            if ((wp & 0xFFF0) == SC_KEYMENU && lp == 0) return 0;
            break;

        case WM_IME_SETCONTEXT:
            lp &= ~ISC_SHOWUICOMPOSITIONWINDOW; /* we draw the marked text */
            return DefWindowProcW(hwnd, msg, wp, lp);
        case WM_IME_STARTCOMPOSITION:
            zant_ime_position(w);
            return 0;
        case WM_IME_COMPOSITION:
            zant_ime_composition(w, lp);
            return 0;
        case WM_IME_ENDCOMPOSITION:
            zant_send_text(w, ZANT_EVENT_COMPOSITION, L"", 0, 0, 0);
            return 0;

        case WM_CLOSE:
            if (zant_close_vetoed(w)) return 0;
            zant_save_placement(w);
            DestroyWindow(hwnd);
            return 0;

        case WM_SETTINGCHANGE:
            /* light / dark app mode changed in Settings */
            if (lp && wcscmp((const wchar_t *)lp, L"ImmersiveColorSet") == 0) {
                zant_send_simple(w, ZANT_EVENT_APPEARANCE_CHANGE);
                zant_request(w);
            }
            break;

        case WM_DESTROY:
            zant_save_placement(w);
            w->closing = true;
            KillTimer(hwnd, ZANT_TIMER_REDRAW);
            KillTimer(hwnd, ZANT_TIMER_MODAL);
            zant_send_simple(w, ZANT_EVENT_WINDOW_CLOSE);
            /* the main window ends the app; other windows just close */
            if (w == g_main) PostQuitMessage(0);
            return 0;

        default:
            break;
    }
    return DefWindowProcW(hwnd, msg, wp, lp);
}

/* ---------- public API ---------- */

static void zant_process_setup(void) {
    static bool done = false;
    if (done) return;
    done = true;
    /* the manifest selects PerMonitorV2; this covers exes without one (tests) */
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    /* UTF-8 console output for the framework's logs */
    SetConsoleOutputCP(CP_UTF8);
    /* STA for the shell's file dialogs */
    CoInitializeEx(NULL, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
    INITCOMMONCONTROLSEX icc = {sizeof(icc), ICC_STANDARD_CLASSES};
    InitCommonControlsEx(&icc);
}

ZantWindow *zant_window_create(const char *title, const ZantWindowOptions *options, const ZantHost *host) {
    zant_process_setup();
    HINSTANCE instance = GetModuleHandleW(NULL);

    WNDCLASSEXW wc;
    memset(&wc, 0, sizeof(wc));
    wc.cbSize = sizeof(wc);
    wc.style = CS_HREDRAW | CS_VREDRAW;
    wc.lpfnWndProc = zant_wndproc;
    wc.hInstance = instance;
    wc.hCursor = LoadCursorW(NULL, IDC_ARROW);
    /* icon resource 1: the app icon built from assets/icons/app-windows.png
       (configureApp); large for Alt+Tab / taskbar, small for the title bar */
    wc.hIcon = (HICON)LoadImageW(instance, MAKEINTRESOURCEW(1), IMAGE_ICON, GetSystemMetrics(SM_CXICON),
                                 GetSystemMetrics(SM_CYICON), LR_DEFAULTCOLOR);
    wc.hIconSm = (HICON)LoadImageW(instance, MAKEINTRESOURCEW(1), IMAGE_ICON, GetSystemMetrics(SM_CXSMICON),
                                   GetSystemMetrics(SM_CYSMICON), LR_DEFAULTCOLOR);
    if (!wc.hIcon) wc.hIcon = LoadIconW(NULL, IDI_APPLICATION);
    wc.lpszClassName = ZANT_CLASS_NAME;
    RegisterClassExW(&wc);

    ZantWindow *w = (ZantWindow *)calloc(1, sizeof(ZantWindow));
    if (!w) return NULL;
    w->host = *host;
    w->style = options->style;
    w->custom_frame = options->style == ZANT_WINDOW_HIDDEN_TITLEBAR;
    w->resizable = options->resizable != 0;
    w->movable_by_background = options->movable_by_background != 0;
    w->titlebar_height = options->titlebar_height;
    w->min_width = options->min_width;
    w->min_height = options->min_height;
    w->background = options->background;
    w->cursor = ZANT_CURSOR_ARROW;
    w->active = true;
    snprintf(w->backend_name, sizeof(w->backend_name), "Skia Raster (CPU)");
    wchar_t *wide_title = zant_widen(title && title[0] ? title : "Zant UI");
    if (wide_title) {
        wcsncpy(w->title, wide_title, sizeof(w->title) / sizeof(w->title[0]) - 1);
        free(wide_title);
    }

    /* size for the monitor the window opens on (the primary one) */
    const POINT origin = {0, 0};
    HMONITOR monitor = MonitorFromPoint(origin, MONITOR_DEFAULTTOPRIMARY);
    zant_set_dpi(w, zant_monitor_dpi(monitor));

    const float width = options->width > 0 ? options->width : 900;
    const float height = options->height > 0 ? options->height : 640;
    const DWORD style = zant_window_style_bits(w);
    DWORD ex_style = options->always_on_top ? WS_EX_TOPMOST : 0;
    /* panel: above other windows, no taskbar button */
    if (options->style == ZANT_WINDOW_PANEL || options->style == ZANT_WINDOW_OVERLAY)
        ex_style |= WS_EX_TOPMOST | WS_EX_TOOLWINDOW;
    RECT rc = {0, 0, (LONG)lroundf(width * w->scale), (LONG)lroundf(height * w->scale)};
    if (w->custom_frame) {
        rc.right += zant_frame_x(w) * 2;
        rc.bottom += zant_frame_y(w);
    } else if (w->style == ZANT_WINDOW_PANEL || w->style == ZANT_WINDOW_OVERLAY) {
        /* no non-client area (WM_NCCALCSIZE / WS_POPUP) */
    } else {
        AdjustWindowRectExForDpi(&rc, style, FALSE, ex_style, w->dpi);
    }
    const int outer_w = rc.right - rc.left, outer_h = rc.bottom - rc.top;
    int x = CW_USEDEFAULT, y = CW_USEDEFAULT;
    if (options->center) {
        MONITORINFO mi = {sizeof(mi)};
        GetMonitorInfoW(monitor, &mi);
        x = mi.rcWork.left + ((mi.rcWork.right - mi.rcWork.left) - outer_w) / 2;
        y = mi.rcWork.top + ((mi.rcWork.bottom - mi.rcWork.top) - outer_h) / 2;
        if (y < mi.rcWork.top) y = mi.rcWork.top;
    }

    HWND hwnd = CreateWindowExW(ex_style, ZANT_CLASS_NAME, w->title, style, x, y, outer_w, outer_h, NULL, NULL,
                                instance, NULL);
    if (!hwnd) {
        free(w);
        return NULL;
    }
    w->hwnd = hwnd;
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, (LONG_PTR)w);
    zant_set_dpi(w, GetDpiForWindow(hwnd));
    zant_apply_dark_frame(w);
    if (w->style == ZANT_WINDOW_PANEL) {
        const DWORD round = 2; /* DWMWCP_ROUND (Windows 11; ignored before) */
        DwmSetWindowAttribute(hwnd, 33 /* DWMWA_WINDOW_CORNER_PREFERENCE */, &round, sizeof(round));
        SetWindowPos(hwnd, NULL, 0, 0, 0, 0, SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER);
    }
    if (w->custom_frame) {
        /* recompute the frame with the caption removed */
        SetWindowPos(hwnd, NULL, 0, 0, 0, 0, SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER);
    }
    zant_update_client_size(w);

    const char *requested = getenv("ZANT_RENDERER");
    const bool force_raster = requested && strcmp(requested, "raster") == 0;
    if (!force_raster) {
        w->renderer = zant_renderer_create_d3d(hwnd, w->width_px, w->height_px);
        if (!w->renderer) fprintf(stderr, "[zant] Direct3D 12 unavailable; falling back to raster\n");
    }
    if (!w->renderer) w->renderer = zant_renderer_create_raster(w->width_px, w->height_px);
    if (!w->renderer) {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        DestroyWindow(hwnd);
        free(w);
        return NULL;
    }
    snprintf(w->backend_name, sizeof(w->backend_name), "%s", zant_renderer_backend_name(w->renderer));

    w->next = g_windows;
    g_windows = w;
    if (!g_main) g_main = w;
    /* opened at login: a tray app starts in the background */
    const bool hidden = options->hidden || (options->hidden_at_login && zant_app_launched_at_login());
    if (!hidden) {
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        SetFocus(hwnd);
    }
    zant_request(w);
    return w;
}

void zant_tray_remove(void);
static void zant_app_emit(int type, int id);

void zant_window_run(ZantWindow *window) {
    (void)window;
    MSG msg;
    while (GetMessageW(&msg, NULL, 0, 0) > 0) {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    /* like [NSApp terminate:] on macOS: closing the window ends the process
       here, so both platforms behave the same after run(); the app hears
       it first (ZANT_APP_EVENT_EXIT) to stop its workers and save */
    zant_app_emit(ZANT_APP_EVENT_EXIT, 0);
    zant_tray_remove();
    /* PostQuitMessage / the main close also exits with secondary HWNDs alive. */
    for (ZantWindow *w = g_windows; w; w = w->next) zant_save_placement(w);
    fflush(stdout);
    fflush(stderr);
    exit((int)msg.wParam);
}

void zant_window_destroy(ZantWindow *window) {
    if (!window) return;
    zant_save_placement(window);
    window->closing = true;
    if (window->renderer) {
        zant_renderer_destroy(window->renderer);
        window->renderer = NULL;
    }
    if (window->background_renderer) {
        zant_renderer_destroy(window->background_renderer);
        window->background_renderer = NULL;
    }
    if (window->hwnd && IsWindow(window->hwnd)) {
        SetWindowLongPtrW(window->hwnd, GWLP_USERDATA, 0);
        DestroyWindow(window->hwnd);
    }
    for (ZantWindow **it = &g_windows; *it; it = &(*it)->next) {
        if (*it == window) {
            *it = window->next;
            break;
        }
    }
    if (g_main == window) g_main = NULL;
    free(window->autosave_path);
    free(window);
}


void zant_window_post_wake(ZantWindow *window) {
    if (!window || !window->hwnd) return;
    /* coalesce: one message per burst of wakes */
    if (InterlockedExchange(&window->wake_pending, 1)) return;
    if (!PostMessageW(window->hwnd, ZANT_WM_WAKE, 0, 0)) InterlockedExchange(&window->wake_pending, 0);
}

void zant_window_request_redraw(ZantWindow *window, int delay_ms) {
    if (!window || window->closing) return;
    if (delay_ms <= 0) {
        zant_request(window);
    } else if (!window->animating) {
        SetTimer(window->hwnd, ZANT_TIMER_REDRAW, (UINT)delay_ms, NULL);
    }
}

void zant_window_set_cursor(ZantWindow *window, int cursor) {
    if (!window) return;
    window->cursor = cursor;
    /* only while the pointer is over the client area */
    POINT pt;
    GetCursorPos(&pt);
    if (WindowFromPoint(pt) == window->hwnd &&
        SendMessageW(window->hwnd, WM_NCHITTEST, 0, MAKELPARAM(pt.x, pt.y)) == HTCLIENT) {
        SetCursor(zant_cursor_handle(cursor));
    }
}

void zant_window_set_background(ZantWindow *window, uint32_t argb) {
    if (!window || window->background == argb) return;
    window->background = argb;
    zant_apply_dark_frame(window);
}

int zant_window_reduce_motion(void) {
    BOOL animations = TRUE;
    if (SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, &animations, 0)) return animations ? 0 : 1;
    return 0;
}

void zant_window_set_document_edited(ZantWindow *window, int edited) {
    if (!window) return;
    window->edited = edited != 0;
    zant_update_title(window);
}

const char *zant_window_renderer_name(ZantWindow *window) {
    return window ? window->backend_name : "none";
}

void zant_window_schedule_snapshot(ZantWindow *window, const char *path, int delay_ms) {
    if (!window || !path) return;
    snprintf(window->snapshot_path, sizeof(window->snapshot_path), "%s", path);
    SetTimer(window->hwnd, ZANT_TIMER_SNAPSHOT, (UINT)(delay_ms > 0 ? delay_ms : 1), NULL);
}

void zant_window_minimize(ZantWindow *window) {
    if (window) ShowWindow(window->hwnd, SW_MINIMIZE);
}

void zant_window_toggle_maximize(ZantWindow *window) {
    if (!window) return;
    ShowWindow(window->hwnd, IsZoomed(window->hwnd) ? SW_RESTORE : SW_MAXIMIZE);
}

int zant_window_is_maximized(ZantWindow *window) {
    return window && IsZoomed(window->hwnd) ? 1 : 0;
}

void zant_window_close(ZantWindow *window) {
    if (window) PostMessageW(window->hwnd, WM_CLOSE, 0, 0);
}

int zant_window_is_active(ZantWindow *window) {
    return window && window->active ? 1 : 0;
}

/* ---------- clipboard ---------- */

int zant_clipboard_set_text(const char *text, size_t length) {
    if (!text) return 0;
    const int units = MultiByteToWideChar(CP_UTF8, 0, text, (int)length, NULL, 0);
    if (units < 0) return 0;
    wchar_t *wide = (wchar_t *)malloc(((size_t)units + 1) * sizeof(wchar_t));
    if (!wide) return 0;
    if (units > 0) MultiByteToWideChar(CP_UTF8, 0, text, (int)length, wide, units);
    /* the Windows clipboard convention is CRLF (zant_clipboard_get_text maps back) */
    size_t lone_lf = 0;
    for (int i = 0; i < units; i++) {
        if (wide[i] == L'\n' && (i == 0 || wide[i - 1] != L'\r')) lone_lf++;
    }
    HGLOBAL memory = GlobalAlloc(GMEM_MOVEABLE, ((size_t)units + lone_lf + 1) * sizeof(wchar_t));
    if (!memory) {
        free(wide);
        return 0;
    }
    wchar_t *dst = (wchar_t *)GlobalLock(memory);
    size_t n = 0;
    for (int i = 0; i < units; i++) {
        if (wide[i] == L'\n' && (i == 0 || wide[i - 1] != L'\r')) dst[n++] = L'\r';
        dst[n++] = wide[i];
    }
    dst[n] = 0;
    GlobalUnlock(memory);
    free(wide);
    HWND owner = g_main ? g_main->hwnd : NULL;
    for (int attempt = 0; attempt < 5; attempt++) {
        if (OpenClipboard(owner)) {
            EmptyClipboard();
            const bool ok = SetClipboardData(CF_UNICODETEXT, memory) != NULL;
            CloseClipboard();
            if (!ok) GlobalFree(memory);
            return ok ? 1 : 0;
        }
        Sleep(10);
    }
    GlobalFree(memory);
    return 0;
}

size_t zant_clipboard_get_text(char *out, size_t capacity) {
    HWND owner = g_main ? g_main->hwnd : NULL;
    size_t required = 1;
    bool opened = false;
    for (int attempt = 0; attempt < 5 && !opened; attempt++) {
        opened = OpenClipboard(owner) != 0;
        if (!opened) Sleep(10);
    }
    char *utf8 = NULL;
    int len = 0;
    if (opened) {
        HANDLE data = GetClipboardData(CF_UNICODETEXT);
        const wchar_t *wide = data ? (const wchar_t *)GlobalLock(data) : NULL;
        if (wide) {
            utf8 = zant_narrow(wide, -1, &len);
            GlobalUnlock(data);
        }
        CloseClipboard();
    }
    /* CRLF -> LF, like text copied on macOS */
    if (utf8) {
        int j = 0;
        for (int i = 0; i < len; i++) {
            if (utf8[i] == '\r' && i + 1 < len && utf8[i + 1] == '\n') continue;
            utf8[j++] = utf8[i];
        }
        len = j;
        utf8[len] = 0;
        required = (size_t)len + 1;
    }
    if (out && capacity > 0) {
        const size_t copy = utf8 ? ((size_t)len < capacity - 1 ? (size_t)len : capacity - 1) : 0;
        if (copy) memcpy(out, utf8, copy);
        out[copy] = 0;
    }
    free(utf8);
    return required;
}

/* ---------- dialogs ---------- */

typedef struct ZantFilters {
    COMDLG_FILTERSPEC specs[2];
    int count;
    wchar_t *pattern; /* "*.md;*.markdown" */
    wchar_t *first_ext;
} ZantFilters;

static void zant_filters_init(ZantFilters *f, const char *extensions) {
    memset(f, 0, sizeof(*f));
    if (!extensions || !extensions[0]) return;
    wchar_t *wide = zant_widen(extensions);
    if (!wide) return;
    const size_t n = wcslen(wide);
    f->pattern = (wchar_t *)calloc(n * 3 + 8, sizeof(wchar_t));
    f->first_ext = (wchar_t *)calloc(n + 1, sizeof(wchar_t));
    if (!f->pattern || !f->first_ext) {
        free(wide);
        return;
    }
    wchar_t *context = NULL;
    for (wchar_t *part = wcstok(wide, L",", &context); part; part = wcstok(NULL, L",", &context)) {
        while (*part == L' ' || *part == L'.') part++;
        size_t len = wcslen(part);
        while (len > 0 && part[len - 1] == L' ') part[--len] = 0;
        if (len == 0) continue;
        if (!f->first_ext[0]) wcscpy(f->first_ext, part);
        if (f->pattern[0]) wcscat(f->pattern, L";");
        wcscat(f->pattern, L"*.");
        wcscat(f->pattern, part);
    }
    free(wide);
    if (f->pattern[0]) {
        f->specs[0].pszName = f->pattern; /* the pattern doubles as the label */
        f->specs[0].pszSpec = f->pattern;
        f->specs[1].pszName = L"*.*";
        f->specs[1].pszSpec = L"*.*";
        f->count = 2;
    }
}

static void zant_filters_free(ZantFilters *f) {
    free(f->pattern);
    free(f->first_ext);
}

static char *zant_dialog_result(IFileDialog *dialog) {
    IShellItem *item = NULL;
    char *result = NULL;
    if (SUCCEEDED(IFileDialog_GetResult(dialog, &item)) && item) {
        PWSTR path = NULL;
        if (SUCCEEDED(IShellItem_GetDisplayName(item, SIGDN_FILESYSPATH, &path)) && path) {
            result = zant_narrow(path, -1, NULL);
            CoTaskMemFree(path);
        }
        IShellItem_Release(item);
    }
    return result;
}

static HWND zant_dialog_owner(void) {
    return g_main ? g_main->hwnd : NULL;
}

char *zant_open_file_dialog_ext(const char *extensions) {
    zant_process_setup();
    IFileOpenDialog *dialog = NULL;
    if (FAILED(CoCreateInstance(&CLSID_FileOpenDialog, NULL, CLSCTX_INPROC_SERVER, &IID_IFileOpenDialog,
                                (void **)&dialog))) {
        return NULL;
    }
    ZantFilters filters;
    zant_filters_init(&filters, extensions);
    if (filters.count) IFileOpenDialog_SetFileTypes(dialog, (UINT)filters.count, filters.specs);
    FILEOPENDIALOGOPTIONS options = 0;
    IFileOpenDialog_GetOptions(dialog, &options);
    IFileOpenDialog_SetOptions(dialog, options | FOS_FORCEFILESYSTEM | FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST);
    char *result = NULL;
    if (SUCCEEDED(IFileOpenDialog_Show(dialog, zant_dialog_owner()))) result = zant_dialog_result((IFileDialog *)dialog);
    IFileOpenDialog_Release(dialog);
    zant_filters_free(&filters);
    return result;
}

char *zant_save_file_dialog_ext(const char *default_name, const char *extensions) {
    zant_process_setup();
    IFileSaveDialog *dialog = NULL;
    if (FAILED(CoCreateInstance(&CLSID_FileSaveDialog, NULL, CLSCTX_INPROC_SERVER, &IID_IFileSaveDialog,
                                (void **)&dialog))) {
        return NULL;
    }
    ZantFilters filters;
    zant_filters_init(&filters, extensions);
    if (filters.count) {
        IFileSaveDialog_SetFileTypes(dialog, (UINT)filters.count, filters.specs);
        IFileSaveDialog_SetDefaultExtension(dialog, filters.first_ext);
    }
    if (default_name && default_name[0]) {
        wchar_t *name = zant_widen(default_name);
        if (name) {
            IFileSaveDialog_SetFileName(dialog, name);
            free(name);
        }
    }
    FILEOPENDIALOGOPTIONS options = 0;
    IFileSaveDialog_GetOptions(dialog, &options);
    IFileSaveDialog_SetOptions(dialog, options | FOS_FORCEFILESYSTEM | FOS_OVERWRITEPROMPT);
    char *result = NULL;
    if (SUCCEEDED(IFileSaveDialog_Show(dialog, zant_dialog_owner()))) result = zant_dialog_result((IFileDialog *)dialog);
    IFileSaveDialog_Release(dialog);
    zant_filters_free(&filters);
    return result;
}

void zant_dialog_free(char *value) {
    free(value);
}

typedef HRESULT(WINAPI *TaskDialogIndirectFn)(const TASKDIALOGCONFIG *, int *, int *, BOOL *);

int zant_confirm_dialog(const char *title, const char *message, const char *first, const char *second,
                        const char *third) {
    zant_process_setup();
    wchar_t *w_title = zant_widen(title ? title : "");
    wchar_t *w_message = zant_widen(message ? message : "");
    wchar_t *labels[3] = {zant_widen(first), zant_widen(second), zant_widen(third)};
    wchar_t *caption = g_main ? g_main->title : L"";
    int answer = 2;

    /* comctl32 v6 (the framework's manifest) has TaskDialogIndirect */
    HMODULE comctl = LoadLibraryW(L"comctl32.dll");
    TaskDialogIndirectFn task_dialog = comctl ? (TaskDialogIndirectFn)(void *)GetProcAddress(comctl, "TaskDialogIndirect") : NULL;
    if (task_dialog) {
        TASKDIALOG_BUTTON buttons[3];
        UINT count = 0;
        for (int i = 0; i < 3; i++) {
            if (!labels[i]) continue;
            buttons[count].nButtonID = 100 + i;
            buttons[count].pszButtonText = labels[i];
            count++;
        }
        TASKDIALOGCONFIG config;
        memset(&config, 0, sizeof(config));
        config.cbSize = sizeof(config);
        config.hwndParent = zant_dialog_owner();
        config.dwFlags = TDF_POSITION_RELATIVE_TO_WINDOW | (labels[2] ? TDF_ALLOW_DIALOG_CANCELLATION : 0);
        config.pszWindowTitle = caption;
        config.pszMainInstruction = w_title;
        config.pszContent = w_message;
        config.pButtons = buttons;
        config.cButtons = count;
        config.nDefaultButton = 100;
        int pressed = 0;
        if (SUCCEEDED(task_dialog(&config, &pressed, NULL, NULL))) {
            if (pressed >= 100 && pressed <= 102) answer = pressed - 100;
            else answer = labels[2] ? 2 : (labels[1] ? 1 : 0); /* Esc / close: the last (cancel) button */
        }
    } else {
        size_t n = wcslen(w_title ? w_title : L"") + wcslen(w_message ? w_message : L"") + 4;
        wchar_t *text = (wchar_t *)calloc(n, sizeof(wchar_t));
        if (text) {
            _snwprintf(text, n - 1, L"%ls\n\n%ls", w_title ? w_title : L"", w_message ? w_message : L"");
            const UINT type = labels[2] ? MB_YESNOCANCEL : (labels[1] ? MB_OKCANCEL : MB_OK);
            const int r = MessageBoxW(zant_dialog_owner(), text, caption, type | MB_ICONWARNING);
            answer = (r == IDYES || r == IDOK) ? 0 : (r == IDNO ? 1 : (labels[2] ? 2 : 1));
            free(text);
        }
    }
    free(w_title);
    free(w_message);
    for (int i = 0; i < 3; i++) free(labels[i]);
    return answer;
}

int zant_open_url(const char *url) {
    if (!url || !url[0]) return 0;
    zant_process_setup();
    /* scheme-less links open as https, like on macOS */
    const char *colon = strchr(url, ':');
    const char *slash = strchr(url, '/');
    char *full = NULL;
    if (!colon || (slash && slash < colon)) {
        const size_t n = strlen(url) + 9;
        full = (char *)malloc(n);
        if (!full) return 0;
        snprintf(full, n, "https://%s", url);
    }
    wchar_t *wide = zant_widen(full ? full : url);
    free(full);
    if (!wide) return 0;
    const HINSTANCE r = ShellExecuteW(zant_dialog_owner(), L"open", wide, NULL, NULL, SW_SHOWNORMAL);
    free(wide);
    return (INT_PTR)r > 32 ? 1 : 0;
}

int zant_reveal_path(const char *path) {
    if (!path || !path[0]) return 0;
    zant_process_setup();
    wchar_t *wide = zant_widen(path);
    if (!wide) return 0;
    for (wchar_t *c = wide; *c; c++) {
        if (*c == L'/') *c = L'\\';
    }
    /* the shell's own "show in folder": opens (or reuses) an Explorer window
       with the item selected */
    int ok = 0;
    PIDLIST_ABSOLUTE item = ILCreateFromPathW(wide);
    if (item) {
        const HRESULT init = CoInitializeEx(NULL, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        ok = SUCCEEDED(SHOpenFolderAndSelectItems(item, 0, NULL, 0)) ? 1 : 0;
        if (SUCCEEDED(init)) CoUninitialize();
        ILFree(item);
    }
    free(wide);
    return ok;
}

/* ---------- showing, hiding and placing windows ---------- */

void zant_window_set_visible(ZantWindow *window, int visible) {
    if (!window || !window->hwnd || window->closing) return;
    const bool had_focus = GetForegroundWindow() == window->hwnd;
    ShowWindow(window->hwnd, visible ? SW_SHOWNOACTIVATE : SW_HIDE);
    if (visible) zant_request(window);
    if (!visible && window->style == ZANT_WINDOW_OVERLAY) {
        HWND back = window->restore_focus;
        window->restore_focus = NULL;
        if (had_focus && back && IsWindow(back) && IsWindowVisible(back)) SetForegroundWindow(back);
    }
}

int zant_window_is_visible(ZantWindow *window) {
    return window && window->hwnd && IsWindowVisible(window->hwnd) ? 1 : 0;
}

int zant_window_is_minimized(ZantWindow *window) {
    return window && window->hwnd && IsIconic(window->hwnd) ? 1 : 0;
}

void zant_window_activate(ZantWindow *window) {
    if (!window || !window->hwnd || window->closing) return;
    ShowWindow(window->hwnd, IsIconic(window->hwnd) ? SW_RESTORE : SW_SHOW);
    /* usually allowed (the call follows a hotkey, a tray click or our own
       input), but the foreground lock can still refuse it, e.g. right after
       another app took the foreground: then share input with the foreground
       thread for the switch, the common way launchers bring their popup up */
    if (!SetForegroundWindow(window->hwnd) || GetForegroundWindow() != window->hwnd) {
        HWND fg = GetForegroundWindow();
        const DWORD fg_thread = fg ? GetWindowThreadProcessId(fg, NULL) : 0;
        const DWORD self = GetCurrentThreadId();
        if (fg_thread && fg_thread != self && AttachThreadInput(fg_thread, self, TRUE)) {
            BringWindowToTop(window->hwnd);
            SetForegroundWindow(window->hwnd);
            AttachThreadInput(fg_thread, self, FALSE);
        }
    }
    SetFocus(window->hwnd);
    zant_request(window);
}

void zant_window_place(ZantWindow *window, int place, float margin) {
    if (!window || !window->hwnd) return;
    POINT cursor;
    GetCursorPos(&cursor);
    HMONITOR monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTOPRIMARY);
    MONITORINFO mi = {sizeof(mi)};
    if (!GetMonitorInfoW(monitor, &mi)) return;
    const RECT area = mi.rcWork;
    if (place == ZANT_PLACE_POINTER) {
        RECT pr;
        GetWindowRect(window->hwnd, &pr);
        const int w = pr.right - pr.left, h = pr.bottom - pr.top;
        const int gap = (int)lroundf(margin * window->scale);
        int x = cursor.x + gap, y = cursor.y + gap;
        if (x + w > area.right) x = cursor.x - gap - w;
        if (y + h > area.bottom) y = cursor.y - gap - h;
        if (x > area.right - w) x = area.right - w;
        if (x < area.left) x = area.left;
        if (y > area.bottom - h) y = area.bottom - h;
        if (y < area.top) y = area.top;
        SetWindowPos(window->hwnd, NULL, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
        return;
    }
    if (place == ZANT_PLACE_TRAY_CORNER) {
        /* the notification area sits at the end of the taskbar */
        if (area.top > mi.rcMonitor.top) place = ZANT_PLACE_TOP_RIGHT;
        else if (area.left > mi.rcMonitor.left) place = ZANT_PLACE_BOTTOM_LEFT;
        else place = ZANT_PLACE_BOTTOM_RIGHT;
    }
    RECT wr;
    GetWindowRect(window->hwnd, &wr);
    const int width = wr.right - wr.left, height = wr.bottom - wr.top;
    const int m = (int)lroundf(margin * window->scale);
    int x = area.left + ((area.right - area.left) - width) / 2;
    int y = area.top + ((area.bottom - area.top) - height) / 2;
    if (place == ZANT_PLACE_TOP_LEFT || place == ZANT_PLACE_BOTTOM_LEFT) x = area.left + m;
    if (place == ZANT_PLACE_TOP_RIGHT || place == ZANT_PLACE_BOTTOM_RIGHT) x = area.right - width - m;
    if (place == ZANT_PLACE_TOP_LEFT || place == ZANT_PLACE_TOP_RIGHT) y = area.top + m;
    if (place == ZANT_PLACE_BOTTOM_LEFT || place == ZANT_PLACE_BOTTOM_RIGHT) y = area.bottom - height - m;
    SetWindowPos(window->hwnd, NULL, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
}

static bool zant_is_overlay_hwnd(HWND hwnd) {
    for (ZantWindow *w = g_windows; w; w = w->next) {
        if (w->hwnd == hwnd && w->style == ZANT_WINDOW_OVERLAY) return true;
    }
    return false;
}

typedef struct {
    int target, index;
    MONITORINFO info;
    bool found;
} ZantMonitorPick;

static BOOL CALLBACK zant_pick_monitor(HMONITOR monitor, HDC dc, LPRECT rect, LPARAM data) {
    (void)dc;
    (void)rect;
    ZantMonitorPick *pick = (ZantMonitorPick *)data;
    if (pick->index++ != pick->target) return TRUE;
    pick->info.cbSize = sizeof(pick->info);
    pick->found = GetMonitorInfoW(monitor, &pick->info) != 0;
    return FALSE;
}

/* Deferred: moving / resizing / activating sends WM_SIZE, WM_DPICHANGED and
   focus messages synchronously, which must not reach the runtime while the
   caller (an app callback) holds the app state. */
void zant_window_cover_screen(ZantWindow *window, int index) {
    if (!window || !window->hwnd || window->closing) return;
    PostMessageW(window->hwnd, ZANT_WM_COVER, (WPARAM)index, 0);
}

static void zant_cover_now(ZantWindow *window, int index) {
    if (!window || !window->hwnd || window->closing) return;
    ZantMonitorPick pick = {index, 0, {0}, false};
    EnumDisplayMonitors(NULL, NULL, zant_pick_monitor, (LPARAM)&pick);
    if (!pick.found) return;
    HWND fg = GetForegroundWindow();
    if (fg && !zant_is_overlay_hwnd(fg)) window->restore_focus = fg;
    const RECT r = pick.info.rcMonitor;
    /* twice: moving to a monitor with another DPI resizes the window once */
    for (int i = 0; i < 2; i++) {
        SetWindowPos(window->hwnd, HWND_TOPMOST, r.left, r.top, r.right - r.left, r.bottom - r.top,
                     SWP_SHOWWINDOW | SWP_NOACTIVATE);
    }
    zant_update_client_size(window);
    zant_send_simple(window, ZANT_EVENT_WINDOW_RESIZE);
    zant_window_activate(window);
}

/* ---------- app ---------- */

static void (*g_app_handler)(void *, const ZantAppEvent *) = NULL;
static void *g_app_handler_userdata = NULL;

void zant_app_set_handler(void (*handler)(void *userdata, const ZantAppEvent *event), void *userdata) {
    g_app_handler = handler;
    g_app_handler_userdata = userdata;
}

static void zant_app_emit(int type, int id) {
    if (!g_app_handler) return;
    ZantAppEvent event = {type, id};
    g_app_handler(g_app_handler_userdata, &event);
}

static UINT g_activate_message = 0;
static HWND zant_app_window(void);

int zant_app_single_instance(const char *app_id) {
    if (!app_id || !app_id[0]) return 1;
    wchar_t name[300];
    _snwprintf(name, 300, L"Local\\%hs.instance", app_id);
    CreateMutexW(NULL, TRUE, name);
    wchar_t message[300];
    _snwprintf(message, 300, L"%hs.activate", app_id);
    g_activate_message = RegisterWindowMessageW(message);
    if (GetLastError() == ERROR_ALREADY_EXISTS) {
        /* the first instance's message window (any top-level window) brings
           it forward; let it take the foreground */
        AllowSetForegroundWindow(ASFW_ANY);
        PostMessageW(HWND_BROADCAST, g_activate_message, 0, 0);
        return 0;
    }
    zant_app_window(); /* receives the broadcast */
    return 1;
}

void zant_app_quit(void) {
    PostQuitMessage(0);
}

/* ---- launch at login: the user's Run key, the app started with --zant-login ---- */

#define ZANT_RUN_KEY L"Software\\Microsoft\\Windows\\CurrentVersion\\Run"
#define ZANT_LOGIN_ARG L"--zant-login"

int zant_app_launched_at_login(void) {
    static int known = -1;
    if (known < 0) {
        int argc = 0;
        wchar_t **argv = CommandLineToArgvW(GetCommandLineW(), &argc);
        known = 0;
        for (int i = 1; argv && i < argc; i++) {
            if (wcscmp(argv[i], ZANT_LOGIN_ARG) == 0) known = 1;
        }
        if (argv) LocalFree(argv);
    }
    return known;
}

/* What Settings > Apps > Startup and Task Manager's Startup tab record for a
   Run entry: 12 bytes, the first one odd when the user turned it off (the Run
   value stays, Windows skips it at logon). */
#define ZANT_STARTUP_APPROVED_KEY L"Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run"

static bool zant_startup_disabled_by_user(const wchar_t *name) {
    BYTE data[12] = {0};
    DWORD size = sizeof(data);
    if (RegGetValueW(HKEY_CURRENT_USER, ZANT_STARTUP_APPROVED_KEY, name, RRF_RT_REG_BINARY, NULL, data, &size) !=
            ERROR_SUCCESS ||
        size < 1)
        return false;
    return (data[0] & 1) != 0;
}

/* The executable a Run command starts ("path" args, or path args). */
static bool zant_run_command_is_self(const wchar_t *command) {
    wchar_t exe[MAX_PATH * 4];
    const DWORD n = GetModuleFileNameW(NULL, exe, (DWORD)(sizeof(exe) / sizeof(exe[0])));
    if (n == 0 || n >= sizeof(exe) / sizeof(exe[0])) return false;
    const wchar_t *start = command;
    while (*start == L' ') start++;
    const wchar_t *end;
    if (*start == L'"') {
        start++;
        end = wcschr(start, L'"');
    } else {
        end = wcschr(start, L' ');
    }
    const size_t length = end ? (size_t)(end - start) : wcslen(start);
    return length == wcslen(exe) &&
           CompareStringOrdinal(start, (int)length, exe, (int)length, TRUE) == CSTR_EQUAL;
}

int zant_launch_at_login_supported(void) {
    return 1;
}

int zant_launch_at_login_get(const char *app_id) {
    wchar_t *name = zant_widen(app_id);
    if (!name) return 0;
    wchar_t command[MAX_PATH * 4 + 64];
    DWORD size = sizeof(command);
    int state = 0;
    if (RegGetValueW(HKEY_CURRENT_USER, ZANT_RUN_KEY, name, RRF_RT_REG_SZ, NULL, command, &size) == ERROR_SUCCESS &&
        zant_run_command_is_self(command)) {
        /* an entry of another copy of the app (moved, an older install)
           does not start this one: off */
        state = zant_startup_disabled_by_user(name) ? 2 : 1;
    }
    free(name);
    return state;
}

int zant_launch_at_login_set(const char *app_id, int on) {
    wchar_t *name = zant_widen(app_id);
    if (!name) return 0;
    HKEY key = NULL;
    int ok = 0;
    if (RegCreateKeyExW(HKEY_CURRENT_USER, ZANT_RUN_KEY, 0, NULL, 0, KEY_SET_VALUE, NULL, &key, NULL) == ERROR_SUCCESS) {
        if (on) {
            wchar_t exe[MAX_PATH * 4];
            const DWORD n = GetModuleFileNameW(NULL, exe, (DWORD)(sizeof(exe) / sizeof(exe[0])));
            wchar_t command[MAX_PATH * 4 + 64];
            if (n > 0 && n < sizeof(exe) / sizeof(exe[0])) {
                swprintf(command, sizeof(command) / sizeof(command[0]), L"\"%ls\" %ls", exe, ZANT_LOGIN_ARG);
                ok = RegSetValueExW(key, name, 0, REG_SZ, (const BYTE *)command,
                                    (DWORD)((wcslen(command) + 1) * sizeof(wchar_t))) == ERROR_SUCCESS;
            }
        } else {
            const LSTATUS rc = RegDeleteValueW(key, name);
            ok = rc == ERROR_SUCCESS || rc == ERROR_FILE_NOT_FOUND;
        }
        RegCloseKey(key);
    }
    /* The user turned the switch on in the app: an earlier "Disabled" from
       Task Manager would keep Windows from starting it (the usual way:
       mark it enabled). Off removes the record with the entry. */
    HKEY approved = NULL;
    if (ok && RegOpenKeyExW(HKEY_CURRENT_USER, ZANT_STARTUP_APPROVED_KEY, 0, KEY_SET_VALUE, &approved) ==
                  ERROR_SUCCESS) {
        if (!on) {
            RegDeleteValueW(approved, name);
        } else if (zant_startup_disabled_by_user(name)) {
            const BYTE enabled[12] = {0x02};
            ok = RegSetValueExW(approved, name, 0, REG_BINARY, enabled, sizeof(enabled)) == ERROR_SUCCESS;
        }
        RegCloseKey(approved);
    } else if (ok && on && zant_startup_disabled_by_user(name)) {
        /* still "Disabled" and that cannot be undone: Windows will not start it */
        ok = 0;
    }
    free(name);
    return ok;
}

void zant_open_login_item_settings(void) {
    ShellExecuteW(NULL, L"open", L"ms-settings:startupapps", NULL, NULL, SW_SHOWNORMAL);
}

size_t zant_device_name(char *out, size_t capacity) {
    wchar_t name[256];
    DWORD len = 256;
    if (!GetComputerNameExW(ComputerNamePhysicalDnsHostname, name, &len)) {
        len = 256;
        if (!GetComputerNameW(name, &len)) len = 0;
    }
    name[len] = 0;
    const int needed = WideCharToMultiByte(CP_UTF8, 0, name, -1, NULL, 0, NULL, NULL);
    if (out && capacity > 0) {
        if (WideCharToMultiByte(CP_UTF8, 0, name, -1, out, (int)capacity, NULL, NULL) == 0) out[capacity - 1] = 0;
    }
    return needed > 0 ? (size_t)needed : 1;
}

int zant_system_dark_mode(void) {
    DWORD light = 1, size = sizeof(light);
    if (RegGetValueW(HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
                     L"AppsUseLightTheme", RRF_RT_REG_DWORD, NULL, &light, &size) != ERROR_SUCCESS)
        return 0;
    return light == 0 ? 1 : 0;
}

/* The clipboard can be held briefly by the app that just wrote to it. */
static bool zant_open_clipboard(HWND owner) {
    for (int i = 0; i < 10; i++) {
        if (OpenClipboard(owner)) return true;
        Sleep(20);
    }
    return false;
}

/* A bitmap (BITMAPINFOHEADER / BITMAPV5HEADER + pixels) as PNG, through Skia. */
static uint8_t *zant_dib_to_png(const uint8_t *dib, size_t size, size_t *out_len) {
    if (size < sizeof(BITMAPINFOHEADER)) return NULL;
    const BITMAPINFOHEADER *bi = (const BITMAPINFOHEADER *)dib;
    const int width = bi->biWidth;
    const int height = bi->biHeight < 0 ? -bi->biHeight : bi->biHeight;
    const bool top_down = bi->biHeight < 0;
    const int bpp = bi->biBitCount;
    if (width <= 0 || height <= 0 || (bpp != 24 && bpp != 32) || width > 16384 || height > 16384) return NULL;
    if (bi->biCompression != BI_RGB && bi->biCompression != BI_BITFIELDS) return NULL;
    size_t offset = bi->biSize;
    if (bi->biCompression == BI_BITFIELDS && bi->biSize == sizeof(BITMAPINFOHEADER)) offset += 12; /* masks */
    offset += (size_t)bi->biClrUsed * 4;
    const size_t stride = (((size_t)width * (size_t)bpp + 31) / 32) * 4;
    if (offset + stride * (size_t)height > size) return NULL;
    uint8_t *pixels = (uint8_t *)malloc((size_t)width * (size_t)height * 4);
    if (!pixels) return NULL;
    /* 32-bit bitmaps often leave alpha at 0: then the image is opaque */
    bool any_alpha = false;
    if (bpp == 32) {
        for (int y = 0; y < height && !any_alpha; y++) {
            const uint8_t *row = dib + offset + stride * (size_t)y;
            for (int x = 0; x < width; x++) if (row[x * 4 + 3]) { any_alpha = true; break; }
        }
    }
    for (int y = 0; y < height; y++) {
        const uint8_t *src = dib + offset + stride * (size_t)(top_down ? y : height - 1 - y);
        uint8_t *dst = pixels + (size_t)y * (size_t)width * 4;
        for (int x = 0; x < width; x++) {
            const uint8_t b = src[x * (bpp / 8)], g = src[x * (bpp / 8) + 1], r = src[x * (bpp / 8) + 2];
            const uint8_t a = (bpp == 32 && any_alpha) ? src[x * 4 + 3] : 255;
            /* Skia N32 premultiplied: BGRA on Windows */
            dst[x * 4 + 0] = (uint8_t)(b * a / 255);
            dst[x * 4 + 1] = (uint8_t)(g * a / 255);
            dst[x * 4 + 2] = (uint8_t)(r * a / 255);
            dst[x * 4 + 3] = a;
        }
    }
    ZantImage *image = zant_image_from_pixels(pixels, width, height, width * 4);
    free(pixels);
    if (!image) return NULL;
    size_t png_len = 0;
    void *png = zant_image_encode_png(image, &png_len);
    zant_image_destroy(image);
    if (!png) return NULL;
    uint8_t *copy = (uint8_t *)malloc(png_len);
    if (copy) memcpy(copy, png, png_len);
    zant_bytes_free(png);
    if (copy && out_len) *out_len = png_len;
    return copy;
}

uint8_t *zant_clipboard_get_image_png(size_t *out_len) {
    if (out_len) *out_len = 0;
    const UINT png_format = RegisterClipboardFormatW(L"PNG");
    if (!IsClipboardFormatAvailable(png_format) && !IsClipboardFormatAvailable(CF_DIBV5) &&
        !IsClipboardFormatAvailable(CF_DIB))
        return NULL;
    if (!zant_open_clipboard(zant_app_window())) return NULL;
    uint8_t *result = NULL;
    HANDLE handle = GetClipboardData(png_format);
    if (handle) {
        const SIZE_T size = GlobalSize(handle);
        const void *data = GlobalLock(handle);
        if (data && size > 8) {
            result = (uint8_t *)malloc(size);
            if (result) {
                memcpy(result, data, size);
                if (out_len) *out_len = size;
            }
        }
        if (data) GlobalUnlock(handle);
    }
    /* a bitmap: copied out, then converted with the clipboard closed (a large
       one takes a while to encode; other apps must not wait for it) */
    uint8_t *dib = NULL;
    SIZE_T dib_size = 0;
    if (!result) {
        handle = GetClipboardData(CF_DIBV5);
        if (!handle) handle = GetClipboardData(CF_DIB);
        if (handle) {
            dib_size = GlobalSize(handle);
            const uint8_t *data = (const uint8_t *)GlobalLock(handle);
            if (data) {
                dib = (uint8_t *)malloc(dib_size);
                if (dib) memcpy(dib, data, dib_size);
                GlobalUnlock(handle);
            }
        }
    }
    CloseClipboard();
    if (dib) {
        result = zant_dib_to_png(dib, dib_size, out_len);
        free(dib);
    }
    return result;
}

/* ---- files on the clipboard (CF_HDROP, as Explorer copies them) ---- */

char *zant_clipboard_get_files(size_t *out_len) {
    if (out_len) *out_len = 0;
    if (!IsClipboardFormatAvailable(CF_HDROP)) return NULL;
    if (!zant_open_clipboard(zant_app_window())) return NULL;
    char *result = NULL;
    size_t used = 0, capacity = 0;
    HANDLE handle = GetClipboardData(CF_HDROP);
    HDROP drop = handle ? (HDROP)GlobalLock(handle) : NULL;
    if (drop) {
        const UINT count = DragQueryFileW(drop, 0xFFFFFFFF, NULL, 0);
        for (UINT i = 0; i < count; i++) {
            const UINT n = DragQueryFileW(drop, i, NULL, 0);
            wchar_t *wide = (wchar_t *)malloc(((size_t)n + 1) * sizeof(wchar_t));
            if (!wide) continue;
            DragQueryFileW(drop, i, wide, n + 1);
            int len = 0;
            char *utf8 = zant_narrow(wide, (int)n, &len);
            free(wide);
            if (!utf8) continue;
            if (used + (size_t)len + 2 > capacity) {
                capacity = (used + (size_t)len + 2) * 2;
                char *grown = (char *)realloc(result, capacity);
                if (!grown) {
                    free(utf8);
                    break;
                }
                result = grown;
            }
            if (used > 0) result[used++] = '\n';
            memcpy(result + used, utf8, (size_t)len);
            used += (size_t)len;
            result[used] = '\0';
            free(utf8);
        }
        GlobalUnlock(handle);
    }
    CloseClipboard();
    if (!result || used == 0) {
        free(result);
        return NULL;
    }
    if (out_len) *out_len = used;
    return result;
}

int zant_clipboard_set_files(const char *paths, size_t len) {
    if (!paths || len == 0) return 0;
    /* DROPFILES + each path as UTF-16 with its NUL + a final NUL */
    char *copy = (char *)malloc(len + 1);
    if (!copy) return 0;
    memcpy(copy, paths, len);
    copy[len] = '\0';
    /* one NUL-terminated path per line (empty lines are skipped) */
    for (size_t i = 0; i < len; i++) {
        if (copy[i] == '\n' || copy[i] == '\r') copy[i] = '\0';
    }
    size_t wide_units = 1;
    for (size_t at = 0; at < len; at += strlen(copy + at) + 1) {
        const int n = MultiByteToWideChar(CP_UTF8, 0, copy + at, -1, NULL, 0);
        if (n > 1) wide_units += (size_t)n;
    }
    if (wide_units == 1) {
        free(copy);
        return 0;
    }
    const size_t bytes = sizeof(DROPFILES) + wide_units * sizeof(wchar_t);
    HGLOBAL mem = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, bytes);
    if (!mem) {
        free(copy);
        return 0;
    }
    DROPFILES *drop = (DROPFILES *)GlobalLock(mem);
    drop->pFiles = sizeof(DROPFILES);
    drop->fWide = TRUE;
    wchar_t *out = (wchar_t *)((char *)drop + sizeof(DROPFILES));
    for (size_t at = 0; at < len;) {
        const char *line = copy + at;
        const size_t line_len = strlen(line);
        const int n = MultiByteToWideChar(CP_UTF8, 0, line, -1, NULL, 0);
        if (n > 1) {
            MultiByteToWideChar(CP_UTF8, 0, line, -1, out, n);
            out += n;
        }
        at += line_len + 1;
    }
    GlobalUnlock(mem);
    free(copy);
    /* paste copies (not moves) the files */
    HGLOBAL effect = GlobalAlloc(GMEM_MOVEABLE, sizeof(DWORD));
    if (effect) {
        *(DWORD *)GlobalLock(effect) = DROPEFFECT_COPY;
        GlobalUnlock(effect);
    }
    if (!zant_open_clipboard(zant_app_window())) {
        GlobalFree(mem);
        if (effect) GlobalFree(effect);
        return 0;
    }
    EmptyClipboard();
    const int ok = SetClipboardData(CF_HDROP, mem) != NULL;
    if (!ok) GlobalFree(mem);
    if (effect && !SetClipboardData(RegisterClipboardFormatW(CFSTR_PREFERREDDROPEFFECT), effect)) GlobalFree(effect);
    CloseClipboard();
    return ok;
}

size_t zant_downloads_dir(char *out, size_t capacity) {
    /* FOLDERID_Downloads */
    static const GUID downloads = {0x374DE290, 0x123F, 0x4565, {0x91, 0x64, 0x39, 0xC4, 0x92, 0x5E, 0x46, 0x7B}};
    PWSTR wide = NULL;
    size_t result = 0;
    if (SUCCEEDED(SHGetKnownFolderPath(&downloads, 0, NULL, &wide)) && wide) {
        int len = 0;
        char *utf8 = zant_narrow(wide, -1, &len);
        if (utf8) {
            result = (size_t)len;
            if (out && capacity > 0) memcpy(out, utf8, (size_t)len < capacity ? (size_t)len : capacity);
            free(utf8);
        }
    }
    if (wide) CoTaskMemFree(wide);
    return result;
}

int zant_clipboard_set_image_png(const uint8_t *png, size_t len) {
    if (!png || len == 0) return 0;
    ZantImage *image = zant_image_from_encoded(png, len);
    if (!image) return 0;
    const int width = zant_image_width(image), height = zant_image_height(image);
    const size_t pixel_bytes = (size_t)width * (size_t)height * 4;
    uint8_t *rgba = (uint8_t *)malloc(pixel_bytes);
    const bool read = rgba && zant_image_read_rgba(image, rgba, pixel_bytes);
    zant_image_destroy(image);
    /* "PNG" keeps transparency for apps that read it; CF_DIBV5 is what every
       app pastes (the system derives CF_DIB / CF_BITMAP from it) */
    HGLOBAL png_mem = GlobalAlloc(GMEM_MOVEABLE, len);
    HGLOBAL dib_mem = read ? GlobalAlloc(GMEM_MOVEABLE, sizeof(BITMAPV5HEADER) + pixel_bytes) : NULL;
    if (png_mem) {
        void *p = GlobalLock(png_mem);
        memcpy(p, png, len);
        GlobalUnlock(png_mem);
    }
    if (dib_mem) {
        uint8_t *p = (uint8_t *)GlobalLock(dib_mem);
        BITMAPV5HEADER *h = (BITMAPV5HEADER *)p;
        memset(h, 0, sizeof(*h));
        h->bV5Size = sizeof(*h);
        h->bV5Width = width;
        h->bV5Height = height; /* bottom-up, as most readers expect */
        h->bV5Planes = 1;
        h->bV5BitCount = 32;
        h->bV5Compression = BI_BITFIELDS;
        h->bV5RedMask = 0x00FF0000;
        h->bV5GreenMask = 0x0000FF00;
        h->bV5BlueMask = 0x000000FF;
        h->bV5AlphaMask = 0xFF000000;
        h->bV5CSType = LCS_sRGB;
        h->bV5Intent = LCS_GM_IMAGES;
        uint8_t *dst = p + sizeof(*h);
        for (int y = 0; y < height; y++) {
            const uint8_t *src = rgba + (size_t)(height - 1 - y) * (size_t)width * 4;
            uint8_t *row = dst + (size_t)y * (size_t)width * 4;
            for (int x = 0; x < width; x++) {
                row[x * 4 + 0] = src[x * 4 + 2];
                row[x * 4 + 1] = src[x * 4 + 1];
                row[x * 4 + 2] = src[x * 4 + 0];
                row[x * 4 + 3] = src[x * 4 + 3];
            }
        }
        GlobalUnlock(dib_mem);
    }
    free(rgba);
    int ok = 0;
    if ((png_mem || dib_mem) && zant_open_clipboard(zant_app_window())) {
        EmptyClipboard();
        if (png_mem && SetClipboardData(RegisterClipboardFormatW(L"PNG"), png_mem)) { png_mem = NULL; ok = 1; }
        if (dib_mem && SetClipboardData(CF_DIBV5, dib_mem)) { dib_mem = NULL; ok = 1; }
        CloseClipboard();
    }
    if (png_mem) GlobalFree(png_mem);
    if (dib_mem) GlobalFree(dib_mem);
    return ok;
}

void zant_clipboard_free(void *bytes) {
    free(bytes);
}

uint64_t zant_clipboard_change_count(void) {
    return (uint64_t)GetClipboardSequenceNumber();
}

/* A DWORD clipboard format set to 0 (CanIncludeInClipboardHistory and
   friends); the clipboard is open. */
static bool zant_clipboard_dword_zero(const wchar_t *name) {
    const UINT format = RegisterClipboardFormatW(name);
    if (!format || !IsClipboardFormatAvailable(format)) return false;
    HANDLE data = GetClipboardData(format);
    if (!data || GlobalSize(data) < sizeof(DWORD)) return false;
    const DWORD *value = (const DWORD *)GlobalLock(data);
    if (!value) return false;
    const bool zero = *value == 0;
    GlobalUnlock(data);
    return zero;
}

int zant_clipboard_privacy(void) {
    /* https://learn.microsoft.com/windows/win32/dataxchg/clipboard-formats
       (cloud clipboard and clipboard history formats) */
    const UINT exclude = RegisterClipboardFormatW(L"ExcludeClipboardContentFromMonitorProcessing");
    const UINT ignore = RegisterClipboardFormatW(L"Clipboard Viewer Ignore");
    if ((exclude && IsClipboardFormatAvailable(exclude)) || (ignore && IsClipboardFormatAvailable(ignore)))
        return ZANT_CLIPBOARD_NO_HISTORY | ZANT_CLIPBOARD_NO_SYNC;
    if (!zant_open_clipboard(NULL)) return 0;
    int flags = 0;
    if (zant_clipboard_dword_zero(L"CanIncludeInClipboardHistory")) flags |= ZANT_CLIPBOARD_NO_HISTORY | ZANT_CLIPBOARD_NO_SYNC;
    if (zant_clipboard_dword_zero(L"CanUploadToCloudClipboard")) flags |= ZANT_CLIPBOARD_NO_SYNC;
    CloseClipboard();
    return flags;
}

/* ---------- tray icon and hotkeys: a hidden message window ---------- */

#define ZANT_APP_CLASS_NAME L"ZantAppMessages"
static HWND g_app_hwnd = NULL;
static UINT g_taskbar_created = 0;
static NOTIFYICONDATAW g_tray;
static bool g_tray_added = false;
/* NIM_ADD failed (right after logon the taskbar may not take icons yet):
   retried on a timer and when Explorer announces TaskbarCreated */
static bool g_tray_pending = false;
static int g_tray_retries = 0;
#define ZANT_TIMER_TRAY_RETRY 1
#define ZANT_TRAY_RETRY_MS 2000
#define ZANT_TRAY_RETRY_FAST 90
#define ZANT_TRAY_RETRY_SLOW_MS 30000
/* the icon for a dark and for a light taskbar (NULL: same as dark) */
static HICON g_tray_icon_dark = NULL;
static HICON g_tray_icon_light = NULL;

/* Windows "Choose your default Windows mode" (taskbar, Start), separate
   from the app mode */
static bool zant_taskbar_light(void) {
    DWORD light = 0, size = sizeof(light);
    if (RegGetValueW(HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
                     L"SystemUsesLightTheme", RRF_RT_REG_DWORD, NULL, &light, &size) != ERROR_SUCCESS)
        return false;
    return light != 0;
}

static void zant_tray_pick_icon(void) {
    HICON icon = (zant_taskbar_light() && g_tray_icon_light) ? g_tray_icon_light : g_tray_icon_dark;
    g_tray.hIcon = icon;
    if (icon) g_tray.uFlags |= NIF_ICON;
}
static HMENU g_tray_menu = NULL;

static void zant_tray_show_menu(void) {
    if (!g_tray_menu) return;
    POINT pt;
    GetCursorPos(&pt);
    /* the menu closes when the user clicks elsewhere only if our window is
       foreground (documented TrackPopupMenu quirk) */
    SetForegroundWindow(g_app_hwnd);
    const UINT flags = TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY |
                       (GetSystemMetrics(SM_MENUDROPALIGNMENT) ? TPM_RIGHTALIGN : TPM_LEFTALIGN);
    const int cmd = (int)TrackPopupMenu(g_tray_menu, flags, pt.x, pt.y, 0, g_app_hwnd, NULL);
    PostMessageW(g_app_hwnd, WM_NULL, 0, 0);
    if (cmd > 0) zant_app_emit(ZANT_APP_EVENT_TRAY_MENU, cmd - 1);
}

/* Adds the icon now, or keeps retrying: true once it is (or will be) shown. */
static bool zant_tray_add(void) {
    /* everything the icon needs (an earlier icon-only update left NIF_ICON) */
    g_tray.uFlags = NIF_MESSAGE | NIF_TIP | (g_tray.hIcon ? NIF_ICON : 0);
    if (Shell_NotifyIconW(NIM_ADD, &g_tray)) {
        g_tray_added = true;
        g_tray_pending = false;
        KillTimer(g_app_hwnd, ZANT_TIMER_TRAY_RETRY);
        return true;
    }
    /* the shell may still hold an icon of ours (e.g. NIM_ADD timed out but
       went through): an update then succeeds */
    if (Shell_NotifyIconW(NIM_MODIFY, &g_tray)) {
        g_tray_added = true;
        g_tray_pending = false;
        KillTimer(g_app_hwnd, ZANT_TIMER_TRAY_RETRY);
        return true;
    }
    if (!g_tray_pending) g_tray_retries = 0;
    g_tray_pending = true;
    /* every 2 s for the first minutes after logon, then every 30 s: a tray
       app started hidden has no other way in */
    SetTimer(g_app_hwnd, ZANT_TIMER_TRAY_RETRY,
             g_tray_retries < ZANT_TRAY_RETRY_FAST ? ZANT_TRAY_RETRY_MS : ZANT_TRAY_RETRY_SLOW_MS, NULL);
    return true;
}

static LRESULT CALLBACK zant_app_wndproc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
    if (msg == WM_TIMER && wp == ZANT_TIMER_TRAY_RETRY) {
        KillTimer(hwnd, ZANT_TIMER_TRAY_RETRY);
        if (g_tray_pending) {
            g_tray_retries++;
            zant_tray_add();
        }
        return 0;
    }
    if (msg == WM_CLIPBOARDUPDATE) {
        zant_app_emit(ZANT_APP_EVENT_CLIPBOARD, 0);
        return 0;
    }
    if (msg == WM_HOTKEY) {
        zant_app_emit(ZANT_APP_EVENT_HOTKEY, (int)wp);
        return 0;
    }
    if (msg == ZANT_WM_TRAY) {
        switch (LOWORD(lp)) {
            case WM_LBUTTONUP: zant_app_emit(ZANT_APP_EVENT_TRAY_CLICK, 0); break;
            case WM_RBUTTONUP:
            case WM_CONTEXTMENU: zant_tray_show_menu(); break;
            default: break;
        }
        return 0;
    }
    if (g_activate_message && msg == g_activate_message) {
        if (g_app_handler) zant_app_emit(ZANT_APP_EVENT_REOPEN, 0);
        else if (g_main && !g_main->closing) zant_window_activate(g_main);
        return 0;
    }
    /* taskbar switched light / dark: the matching tray icon */
    if (msg == WM_SETTINGCHANGE && lp && wcscmp((const wchar_t *)lp, L"ImmersiveColorSet") == 0 && g_tray_added) {
        g_tray.uFlags = NIF_ICON;
        zant_tray_pick_icon();
        Shell_NotifyIconW(NIM_MODIFY, &g_tray);
        return 0;
    }
    /* Explorer (re)started: the notification area forgot our icon, or is
       ready for the one that failed at logon */
    if (g_taskbar_created && msg == g_taskbar_created && (g_tray_added || g_tray_pending)) {
        g_tray_added = false;
        zant_tray_add();
        return 0;
    }
    return DefWindowProcW(hwnd, msg, wp, lp);
}

static HWND zant_app_window(void) {
    if (g_app_hwnd) return g_app_hwnd;
    zant_process_setup();
    WNDCLASSEXW wc;
    memset(&wc, 0, sizeof(wc));
    wc.cbSize = sizeof(wc);
    wc.lpfnWndProc = zant_app_wndproc;
    wc.hInstance = GetModuleHandleW(NULL);
    wc.lpszClassName = ZANT_APP_CLASS_NAME;
    RegisterClassExW(&wc);
    /* a hidden top-level window (not HWND_MESSAGE): TrackPopupMenu needs a
       window that can be foreground */
    g_app_hwnd = CreateWindowExW(WS_EX_TOOLWINDOW, ZANT_APP_CLASS_NAME, L"", WS_POPUP, 0, 0, 0, 0, NULL, NULL,
                                 wc.hInstance, NULL);
    g_taskbar_created = RegisterWindowMessageW(L"TaskbarCreated");
    return g_app_hwnd;
}

int zant_tray_icon_pixels(void) {
    zant_process_setup();
    return GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem());
}

static HICON zant_icon_from_rgba(const uint8_t *rgba, int width, int height) {
    BITMAPV5HEADER bi;
    memset(&bi, 0, sizeof(bi));
    bi.bV5Size = sizeof(bi);
    bi.bV5Width = width;
    bi.bV5Height = -height; /* top-down */
    bi.bV5Planes = 1;
    bi.bV5BitCount = 32;
    bi.bV5Compression = BI_BITFIELDS;
    bi.bV5RedMask = 0x00FF0000;
    bi.bV5GreenMask = 0x0000FF00;
    bi.bV5BlueMask = 0x000000FF;
    bi.bV5AlphaMask = 0xFF000000;
    void *bits = NULL;
    HDC dc = GetDC(NULL);
    HBITMAP color = CreateDIBSection(dc, (BITMAPINFO *)&bi, DIB_RGB_COLORS, &bits, NULL, 0);
    ReleaseDC(NULL, dc);
    if (!color || !bits) return NULL;
    uint8_t *dst = (uint8_t *)bits;
    for (int i = 0; i < width * height; i++) { /* RGBA -> BGRA, straight alpha */
        dst[i * 4 + 0] = rgba[i * 4 + 2];
        dst[i * 4 + 1] = rgba[i * 4 + 1];
        dst[i * 4 + 2] = rgba[i * 4 + 0];
        dst[i * 4 + 3] = rgba[i * 4 + 3];
    }
    HBITMAP mask = CreateBitmap(width, height, 1, 1, NULL);
    ICONINFO info = {TRUE, 0, 0, mask, color};
    HICON icon = CreateIconIndirect(&info);
    DeleteObject(color);
    DeleteObject(mask);
    return icon;
}

static void zant_tray_apply_icon(const uint8_t *rgba, const uint8_t *rgba_light, int width, int height,
                                 const char *tooltip) {
    if (rgba && width > 0 && height > 0) {
        HICON dark = zant_icon_from_rgba(rgba, width, height);
        HICON light = rgba_light ? zant_icon_from_rgba(rgba_light, width, height) : NULL;
        if (dark) {
            if (g_tray_icon_dark) DestroyIcon(g_tray_icon_dark);
            if (g_tray_icon_light) DestroyIcon(g_tray_icon_light);
            g_tray_icon_dark = dark;
            g_tray_icon_light = light;
        }
    }
    zant_tray_pick_icon();
    g_tray.szTip[0] = 0;
    if (tooltip) MultiByteToWideChar(CP_UTF8, 0, tooltip, -1, g_tray.szTip, (int)(sizeof(g_tray.szTip) / sizeof(WCHAR)) - 1);
}

int zant_tray_set(const uint8_t *rgba, const uint8_t *rgba_light, int width, int height, float scale,
                  int template_image, const char *tooltip, const ZantTrayItem *items, int count) {
    (void)scale;
    (void)template_image;
    HWND hwnd = zant_app_window();
    if (!hwnd) return 0;
    if (!g_tray_added && !g_tray_pending) {
        memset(&g_tray, 0, sizeof(g_tray));
        g_tray.cbSize = sizeof(g_tray);
        g_tray.hWnd = hwnd;
        g_tray.uID = 1;
        g_tray.uCallbackMessage = ZANT_WM_TRAY;
    }
    g_tray.uFlags = NIF_MESSAGE | NIF_TIP;
    zant_tray_apply_icon(rgba, rgba_light, width, height, tooltip);
    if (g_tray_menu) DestroyMenu(g_tray_menu);
    g_tray_menu = count > 0 ? CreatePopupMenu() : NULL;
    for (int i = 0; i < count; i++) {
        const ZantTrayItem *it = &items[i];
        if (it->separator) {
            AppendMenuW(g_tray_menu, MF_SEPARATOR, 0, NULL);
            continue;
        }
        wchar_t *text = zant_widen(it->title ? it->title : "");
        UINT flags = MF_STRING | (it->checked ? MF_CHECKED : 0) | (it->disabled ? MF_GRAYED : 0);
        AppendMenuW(g_tray_menu, flags, (UINT_PTR)(it->id + 1), text ? text : L"");
        free(text);
    }
    if (g_tray_added) return Shell_NotifyIconW(NIM_MODIFY, &g_tray) ? 1 : 0;
    return zant_tray_add() ? 1 : 0;
}

/* Icon and tooltip only: the menu (possibly open right now) stays. */
int zant_tray_set_icon(const uint8_t *rgba, const uint8_t *rgba_light, int width, int height, float scale,
                       int template_image, const char *tooltip) {
    (void)scale;
    (void)template_image;
    if (!g_tray_added && !g_tray_pending) return 0;
    g_tray.uFlags = NIF_MESSAGE | NIF_TIP;
    zant_tray_apply_icon(rgba, rgba_light, width, height, tooltip);
    /* still waiting for the taskbar: the retry adds the new look */
    if (!g_tray_added) return 1;
    return Shell_NotifyIconW(NIM_MODIFY, &g_tray) ? 1 : 0;
}

void zant_tray_remove(void) {
    if (g_tray_added) Shell_NotifyIconW(NIM_DELETE, &g_tray);
    g_tray_added = false;
    g_tray_pending = false;
    if (g_app_hwnd) KillTimer(g_app_hwnd, ZANT_TIMER_TRAY_RETRY);
    if (g_tray_icon_dark) DestroyIcon(g_tray_icon_dark);
    if (g_tray_icon_light) DestroyIcon(g_tray_icon_light);
    g_tray_icon_dark = g_tray_icon_light = NULL;
    g_tray.hIcon = NULL;
    if (g_tray_menu) DestroyMenu(g_tray_menu);
    g_tray_menu = NULL;
}

static UINT zant_vk_from_key(int key) {
    if (key >= 'a' && key <= 'z') return (UINT)('A' + (key - 'a'));
    if (key >= 'A' && key <= 'Z') return (UINT)key;
    if (key >= '0' && key <= '9') return (UINT)key;
    if (key >= ZANT_KEY_F1 && key <= ZANT_KEY_F12) return (UINT)(VK_F1 + (key - ZANT_KEY_F1));
    switch (key) {
        case ZANT_KEY_SPACE: return VK_SPACE;
        case ZANT_KEY_ENTER: return VK_RETURN;
        case ZANT_KEY_TAB: return VK_TAB;
        case ZANT_KEY_ESCAPE: return VK_ESCAPE;
        case ZANT_KEY_LEFT: return VK_LEFT;
        case ZANT_KEY_RIGHT: return VK_RIGHT;
        case ZANT_KEY_UP: return VK_UP;
        case ZANT_KEY_DOWN: return VK_DOWN;
        default: break;
    }
    if (key > 0x20 && key < 0x7f) {
        const SHORT scan = VkKeyScanW((WCHAR)key);
        if (scan != -1 && !(scan & 0x0100)) return (UINT)(scan & 0xff); /* no Shift needed */
    }
    return 0;
}

int zant_hotkey_register(int id, int key, int modifiers) {
    HWND hwnd = zant_app_window();
    if (!hwnd) return 0;
    UnregisterHotKey(hwnd, id);
    if (!(modifiers & (ZANT_MOD_CONTROL | ZANT_MOD_ALT | ZANT_MOD_META))) return 0;
    const UINT vk = zant_vk_from_key(key);
    if (!vk) return 0;
    UINT mods = MOD_NOREPEAT;
    if (modifiers & ZANT_MOD_CONTROL) mods |= MOD_CONTROL;
    if (modifiers & ZANT_MOD_ALT) mods |= MOD_ALT;
    if (modifiers & ZANT_MOD_SHIFT) mods |= MOD_SHIFT;
    if (modifiers & ZANT_MOD_META) mods |= MOD_WIN;
    return RegisterHotKey(hwnd, id, mods, vk) ? 1 : 0;
}

void zant_hotkey_unregister(int id) {
    if (g_app_hwnd) UnregisterHotKey(g_app_hwnd, id);
}

void zant_clipboard_watch(int enable) {
    static bool watching = false;
    HWND hwnd = zant_app_window();
    if (!hwnd || (enable != 0) == watching) return;
    watching = enable != 0;
    if (watching) AddClipboardFormatListener(hwnd);
    else RemoveClipboardFormatListener(hwnd);
}
