/* Win32 backend of src/platform/platform.h: one HWND, Skia via zant_skia.dll
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

#include <math.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "src/platform/platform.h"
#include "src/renderer/skia/skia_bridge.h"

#define ZANT_CLASS_NAME L"ZantWindow"
#define ZANT_TIMER_REDRAW 1
#define ZANT_TIMER_SNAPSHOT 2
#define ZANT_TIMER_EXIT 3
#define ZANT_TIMER_MODAL 4
/* app-drawn title bar height when the app does not say (Windows 11) */
#define ZANT_DEFAULT_CAPTION 32.0f

struct ZantWindow {
    ZantHost host;
    ZantRenderer *renderer;
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
    char backend_name[64];
    char snapshot_path[1024];
};

static ZantWindow *g_window = NULL;

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

static void zant_request(ZantWindow *w) {
    if (w && w->hwnd && !w->closing) InvalidateRect(w->hwnd, NULL, FALSE);
}

static int zant_send(ZantWindow *w, ZantEvent *e) {
    if (!w || !w->host.event) return 0;
    const int handled = w->host.event(w->host.userdata, e);
    if (handled) zant_request(w);
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
    if (w->animating) {
        zant_request(w);
    } else if (delay > 0) {
        SetTimer(w->hwnd, ZANT_TIMER_REDRAW, (UINT)delay, NULL);
    } else if (delay == 0) {
        zant_request(w);
    }
}

static void zant_render(ZantWindow *w, HDC dc) {
    if (!w || w->closing || w->in_draw || !w->host.draw || !w->renderer) return;
    w->in_draw = true;
    zant_update_client_size(w);
    zant_renderer_resize(w->renderer, w->width_px, w->height_px);
    ZantCanvas *canvas = zant_renderer_begin(w->renderer, w->scale);
    if (canvas) {
        w->host.draw(w->host.userdata, canvas, zant_logical_width(w), zant_logical_height(w), w->scale);
    }
    zant_renderer_end(w->renderer);
    const bool gpu = zant_renderer_is_gpu(w->renderer);
    if (!gpu) {
        HDC target = dc ? dc : GetDC(w->hwnd);
        zant_present_raster(w, target);
        if (!dc) ReleaseDC(w->hwnd, target);
    }
    w->in_draw = false;
    zant_schedule_next(w);
    /* raster animation frames wait for the next composition (vsync) */
    if (w->animating && !gpu && !w->in_size_move) DwmFlush();
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
        default: return LoadCursorW(NULL, IDC_ARROW);
    }
}

static LRESULT CALLBACK zant_wndproc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
    ZantWindow *w = (ZantWindow *)GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if (!w) return DefWindowProcW(hwnd, msg, wp, lp);

    switch (msg) {
        case WM_NCCALCSIZE:
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
            zant_request(w);
            return 0;

        case WM_SIZE: {
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
            DestroyWindow(hwnd);
            return 0;

        case WM_DESTROY:
            w->closing = true;
            KillTimer(hwnd, ZANT_TIMER_REDRAW);
            KillTimer(hwnd, ZANT_TIMER_MODAL);
            zant_send_simple(w, ZANT_EVENT_WINDOW_CLOSE);
            PostQuitMessage(0);
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
    wc.hIcon = LoadIconW(instance, MAKEINTRESOURCEW(1));
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
    UINT dpi_x = 96, dpi_y = 96;
    {
        typedef HRESULT(WINAPI * GetDpiForMonitorFn)(HMONITOR, int, UINT *, UINT *);
        HMODULE shcore = LoadLibraryW(L"shcore.dll");
        GetDpiForMonitorFn get_dpi = shcore ? (GetDpiForMonitorFn)(void *)GetProcAddress(shcore, "GetDpiForMonitor") : NULL;
        if (!get_dpi || FAILED(get_dpi(monitor, 0, &dpi_x, &dpi_y))) dpi_x = GetDpiForSystem();
    }
    zant_set_dpi(w, dpi_x);

    const float width = options->width > 0 ? options->width : 900;
    const float height = options->height > 0 ? options->height : 640;
    const DWORD style = zant_window_style_bits(w);
    const DWORD ex_style = options->always_on_top ? WS_EX_TOPMOST : 0;
    RECT rc = {0, 0, (LONG)lroundf(width * w->scale), (LONG)lroundf(height * w->scale)};
    if (w->custom_frame) {
        rc.right += zant_frame_x(w) * 2;
        rc.bottom += zant_frame_y(w);
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
    g_window = w;
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, (LONG_PTR)w);
    zant_set_dpi(w, GetDpiForWindow(hwnd));
    zant_apply_dark_frame(w);
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
        DestroyWindow(hwnd);
        g_window = NULL;
        free(w);
        return NULL;
    }
    snprintf(w->backend_name, sizeof(w->backend_name), "%s", zant_renderer_backend_name(w->renderer));

    ShowWindow(hwnd, SW_SHOW);
    SetForegroundWindow(hwnd);
    SetFocus(hwnd);
    zant_request(w);
    return w;
}

void zant_window_run(ZantWindow *window) {
    (void)window;
    MSG msg;
    while (GetMessageW(&msg, NULL, 0, 0) > 0) {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    /* like [NSApp terminate:] on macOS: closing the window ends the process
       here, so both platforms behave the same after run() */
    fflush(stdout);
    fflush(stderr);
    exit((int)msg.wParam);
}

void zant_window_destroy(ZantWindow *window) {
    if (!window) return;
    window->closing = true;
    if (window->renderer) {
        zant_renderer_destroy(window->renderer);
        window->renderer = NULL;
    }
    if (window->hwnd && IsWindow(window->hwnd)) {
        SetWindowLongPtrW(window->hwnd, GWLP_USERDATA, 0);
        DestroyWindow(window->hwnd);
    }
    if (g_window == window) g_window = NULL;
    free(window);
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
    HWND owner = g_window ? g_window->hwnd : NULL;
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
    HWND owner = g_window ? g_window->hwnd : NULL;
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
    return g_window ? g_window->hwnd : NULL;
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
    wchar_t *caption = g_window ? g_window->title : L"";
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
