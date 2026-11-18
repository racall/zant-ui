#include "window_internal.h"

#include <math.h>
#include <stdlib.h>
#include <wchar.h>

/* ---------- persistent window placement ---------- */

/* The DPI of a monitor (shcore, Windows 8.1+); the system DPI before that. */
UINT zant_monitor_dpi(HMONITOR monitor) {
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

void zant_save_placement(ZantWindow *w) {
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

float zant_logical_width(ZantWindow *w) {
    return (float)w->width_px / w->scale;
}

float zant_logical_height(ZantWindow *w) {
    return (float)w->height_px / w->scale;
}

void zant_update_client_size(ZantWindow *w) {
    RECT rc;
    GetClientRect(w->hwnd, &rc);
    w->width_px = rc.right - rc.left > 0 ? rc.right - rc.left : 1;
    w->height_px = rc.bottom - rc.top > 0 ? rc.bottom - rc.top : 1;
}

void zant_set_dpi(ZantWindow *w, UINT dpi) {
    w->dpi = dpi ? dpi : 96;
    w->scale = (float)w->dpi / 96.0f;
}

int zant_frame_x(ZantWindow *w) {
    return GetSystemMetricsForDpi(SM_CXFRAME, w->dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, w->dpi);
}

int zant_frame_y(ZantWindow *w) {
    return GetSystemMetricsForDpi(SM_CYFRAME, w->dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, w->dpi);
}


/* The outer (frame) size in pixels of a `width` x `height` point content
   area, as the window is styled now. */
void zant_outer_size(ZantWindow *w, float width, float height, int *outer_w, int *outer_h) {
    RECT rc = {0, 0, (LONG)lroundf(width * w->scale), (LONG)lroundf(height * w->scale)};
    if (w->custom_frame) {
        rc.right += zant_frame_x(w) * 2;
        rc.bottom += zant_frame_y(w);
    } else if (w->style == ZANT_WINDOW_PANEL || w->style == ZANT_WINDOW_OVERLAY) {
        /* no non-client area (WM_NCCALCSIZE / WS_POPUP) */
    } else {
        AdjustWindowRectExForDpi(&rc, zant_window_style_bits(w), FALSE, 0, w->dpi);
    }
    *outer_w = rc.right - rc.left;
    *outer_h = rc.bottom - rc.top;
}

DWORD zant_window_style_bits(ZantWindow *w) {
    if (w->style == ZANT_WINDOW_BORDERLESS) return WS_POPUP | (w->resizable ? WS_THICKFRAME : 0) | WS_MINIMIZEBOX;
    /* panel: no caption; the thick frame (hidden by WM_NCCALCSIZE) gives the
       DWM shadow, DWMWA_WINDOW_CORNER_PREFERENCE the rounded corners */
    if (w->style == ZANT_WINDOW_PANEL) return WS_POPUP | WS_THICKFRAME;
    if (w->style == ZANT_WINDOW_OVERLAY) return WS_POPUP;
    DWORD style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
    if (w->resizable) style |= WS_THICKFRAME | WS_MAXIMIZEBOX;
    return style;
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

void zant_window_set_size(ZantWindow *window, float width, float height) {
    if (!window || window->fullscreen || width <= 0 || height <= 0) return;
    if (width < window->min_width) width = window->min_width;
    if (height < window->min_height) height = window->min_height;
    if (window->max_width > 0 && width > window->max_width) width = window->max_width;
    if (window->max_height > 0 && height > window->max_height) height = window->max_height;
    if (IsZoomed(window->hwnd)) ShowWindow(window->hwnd, SW_RESTORE);
    int outer_w = 0, outer_h = 0;
    zant_outer_size(window, width, height, &outer_w, &outer_h);
    SetWindowPos(window->hwnd, NULL, 0, 0, outer_w, outer_h, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
}

/* Grows or shrinks the window into its current limits. */
static void zant_clamp_size(ZantWindow *window) {
    RECT rc;
    if (window->fullscreen || IsZoomed(window->hwnd) || !GetClientRect(window->hwnd, &rc)) return;
    const float width = (float)(rc.right - rc.left) / window->scale;
    const float height = (float)(rc.bottom - rc.top) / window->scale;
    float clamped_w = width < window->min_width ? window->min_width : width;
    float clamped_h = height < window->min_height ? window->min_height : height;
    if (window->max_width > 0 && clamped_w > window->max_width) clamped_w = window->max_width;
    if (window->max_height > 0 && clamped_h > window->max_height) clamped_h = window->max_height;
    if (clamped_w != width || clamped_h != height) zant_window_set_size(window, clamped_w, clamped_h);
}

void zant_window_set_min_size(ZantWindow *window, float width, float height) {
    if (!window) return;
    window->min_width = width > 0 ? width : 0;
    window->min_height = height > 0 ? height : 0;
    zant_clamp_size(window);
}

void zant_window_set_max_size(ZantWindow *window, float width, float height) {
    if (!window) return;
    window->max_width = width > 0 ? width : 0;
    window->max_height = height > 0 ? height : 0;
    zant_clamp_size(window);
}

void zant_window_set_always_on_top(ZantWindow *window, int on) {
    if (!window || window->style == ZANT_WINDOW_PANEL || window->style == ZANT_WINDOW_OVERLAY) return;
    SetWindowPos(window->hwnd, on ? HWND_TOPMOST : HWND_NOTOPMOST, 0, 0, 0, 0,
                 SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
}

/* Borderless over the whole monitor, and back to the saved style and
   placement (Raymond Chen's approach). */
void zant_window_set_fullscreen(ZantWindow *window, int on) {
    if (!window || window->style == ZANT_WINDOW_PANEL || window->style == ZANT_WINDOW_OVERLAY) return;
    if ((on != 0) == window->fullscreen) return;
    HWND hwnd = window->hwnd;
    if (on) {
        MONITORINFO mi = {sizeof(mi)};
        window->saved_placement.length = sizeof(window->saved_placement);
        if (!GetWindowPlacement(hwnd, &window->saved_placement) ||
            !GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY), &mi)) {
            return;
        }
        window->saved_style = (DWORD)GetWindowLongPtrW(hwnd, GWL_STYLE);
        window->fullscreen = true;
        SetWindowLongPtrW(hwnd, GWL_STYLE, (LONG_PTR)((window->saved_style & ~WS_OVERLAPPEDWINDOW) | WS_POPUP));
        SetWindowPos(hwnd, HWND_TOP, mi.rcMonitor.left, mi.rcMonitor.top, mi.rcMonitor.right - mi.rcMonitor.left,
                     mi.rcMonitor.bottom - mi.rcMonitor.top, SWP_NOOWNERZORDER | SWP_FRAMECHANGED);
    } else {
        window->fullscreen = false;
        SetWindowLongPtrW(hwnd, GWL_STYLE, (LONG_PTR)window->saved_style);
        SetWindowPlacement(hwnd, &window->saved_placement);
        SetWindowPos(hwnd, NULL, 0, 0, 0, 0,
                     SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_FRAMECHANGED);
    }
    zant_request(window);
}

int zant_window_is_fullscreen(ZantWindow *window) {
    return window && window->fullscreen ? 1 : 0;
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

void zant_cover_now(ZantWindow *window, int index) {
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
