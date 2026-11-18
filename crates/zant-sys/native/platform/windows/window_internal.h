#ifndef ZANT_WINDOWS_WINDOW_INTERNAL_H
#define ZANT_WINDOWS_WINDOW_INTERNAL_H

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

#include <windows.h>
#include <stdbool.h>

#include "platform/platform.h"
#include "skia/skia_bridge.h"

#define ZANT_TIMER_REDRAW 1
#define ZANT_TIMER_SNAPSHOT 2
#define ZANT_TIMER_EXIT 3

/* zant_window_post_wake: hands background results to the UI thread. */
#define ZANT_WM_WAKE (WM_APP + 7)

/* zant_window_cover_screen, run on the next loop turn (wParam: screen index) */
#define ZANT_WM_COVER (WM_APP + 9)
/* a frame for a hidden window: Windows sends no WM_PAINT to it, yet the app
   still builds (first build, effects, tray, timers) while it sits in the tray */
#define ZANT_WM_HIDDEN_FRAME (WM_APP + 10)

struct ZantWindow;
void zant_cover_now(struct ZantWindow *window, int index);

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
    /* 0: no limit */
    float max_width;
    float max_height;
    /* zant_window_set_fullscreen: the style and placement to restore */
    bool fullscreen;
    DWORD saved_style;
    WINDOWPLACEMENT saved_placement;
    UINT dpi;
    float scale;
    int width_px;
    int height_px;
    bool closing;
    bool in_draw;
    bool animating;
    bool in_size_move;
    /* zant_now_us() of the last drawn frame: a live resize draws at each
       WM_SIZE, and the modal timer adds animation frames only between them */
    long long last_frame_us;
    /* zant_now_us() of the last WM_SIZE that changed the size */
    long long last_resize_us;
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

extern ZantWindow *g_main;
extern ZantWindow *g_windows;

void zant_process_setup(void);
HWND zant_app_window(void);

wchar_t *zant_widen(const char *utf8);
char *zant_narrow(const wchar_t *wide, int length, int *out_len);

ZantEvent zant_event(ZantWindow *w, int type);
void zant_send_simple(ZantWindow *w, int type);
LRESULT CALLBACK zant_wndproc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp);

UINT zant_monitor_dpi(HMONITOR monitor);
void zant_save_placement(ZantWindow *w);
float zant_logical_width(ZantWindow *w);
float zant_logical_height(ZantWindow *w);
void zant_update_client_size(ZantWindow *w);
void zant_set_dpi(ZantWindow *w, UINT dpi);
int zant_frame_x(ZantWindow *w);
int zant_frame_y(ZantWindow *w);
void zant_outer_size(ZantWindow *w, float width, float height, int *outer_w, int *outer_h);
DWORD zant_window_style_bits(ZantWindow *w);

void zant_ime_position(ZantWindow *w);
void zant_send_text(ZantWindow *w, int type, const wchar_t *text, int units, size_t sel_start,
                           size_t sel_end);
void zant_ime_composition(ZantWindow *w, LPARAM flags);

LRESULT zant_custom_hit(ZantWindow *w, int x, int y);
HCURSOR zant_cursor_handle(int cursor);

bool zant_backgrounded(ZantWindow *w);
long long zant_now_us(void);
void zant_request(ZantWindow *w);
void zant_render(ZantWindow *w, HDC dc);
void zant_take_snapshot(ZantWindow *w);

#endif
