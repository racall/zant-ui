#include "window_internal.h"

#include <shellapi.h>
#include <commctrl.h>
#include <objbase.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>

#define ZANT_WM_TRAY (WM_APP + 8)

void zant_process_setup(void) {
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

HWND zant_app_window(void) {
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
