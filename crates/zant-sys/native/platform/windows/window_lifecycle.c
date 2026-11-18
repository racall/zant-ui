#include "window_internal.h"

#include <dwmapi.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>

#define ZANT_CLASS_NAME L"ZantWindow"

/* app-drawn title bar height when the app does not say (Windows 11) */
#define ZANT_DEFAULT_CAPTION 32.0f

/* The first window: closing it ends the app; dialogs are owned by it. */
ZantWindow *g_main = NULL;
ZantWindow *g_windows = NULL;

/* ---------- title bar ---------- */

static float zant_caption_height(ZantWindow *w) {
    return w->titlebar_height > 0 ? w->titlebar_height : ZANT_DEFAULT_CAPTION;
}

/* Hit test of the app-drawn title bar / movable background (client px). */
LRESULT zant_custom_hit(ZantWindow *w, int x, int y) {
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

HCURSOR zant_cursor_handle(int cursor) {
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

/* ---------- public API ---------- */

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

void zant_window_set_document_edited(ZantWindow *window, int edited) {
    if (!window) return;
    window->edited = edited != 0;
    zant_update_title(window);
}

void zant_window_set_title(ZantWindow *window, const char *title) {
    if (!window || !title) return;
    wchar_t *wide = zant_widen(title);
    if (!wide) return;
    if (wcscmp(wide, window->title) != 0) {
        wcsncpy(window->title, wide, sizeof(window->title) / sizeof(window->title[0]) - 1);
        window->title[sizeof(window->title) / sizeof(window->title[0]) - 1] = 0;
        zant_update_title(window);
    }
    free(wide);
}

void zant_window_close(ZantWindow *window) {
    if (window) PostMessageW(window->hwnd, WM_CLOSE, 0, 0);
}

int zant_window_is_active(ZantWindow *window) {
    return window && window->active ? 1 : 0;
}
