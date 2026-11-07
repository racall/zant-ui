/* Zant UI platform C ABI, implemented by src/platform/macos/window.m
 * (AppKit) and src/platform/windows/window.c (Win32). An app has a main
 * window (the first one created; closing it ends the app) and may open more
 * (panels, tool windows) on the UI thread; zant_window_run runs the event
 * loop for all of them. Coordinates are logical points; dpr converts to
 * pixels. */
#ifndef ZANT_PLATFORM_H
#define ZANT_PLATFORM_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct ZantCanvas ZantCanvas;
typedef struct ZantWindow ZantWindow;

enum {
    ZANT_EVENT_POINTER_MOVE = 1,
    ZANT_EVENT_POINTER_DOWN = 2,
    ZANT_EVENT_POINTER_UP = 3,
    ZANT_EVENT_POINTER_ENTER = 4,
    ZANT_EVENT_POINTER_LEAVE = 5,
    ZANT_EVENT_KEY_DOWN = 6,
    ZANT_EVENT_KEY_UP = 7,
    ZANT_EVENT_TEXT_INPUT = 8,
    ZANT_EVENT_COMPOSITION = 9,
    ZANT_EVENT_WINDOW_RESIZE = 10,
    ZANT_EVENT_WINDOW_FOCUS = 11,
    ZANT_EVENT_WINDOW_BLUR = 12,
    ZANT_EVENT_WINDOW_CLOSE = 13,
    ZANT_EVENT_SCROLL = 14,
    /* Close button / ⌘W / Quit / Alt+F4: the handler returns non-zero to veto.
       ZantEvent.button is 1 when the whole app is asked to quit (⌘Q, the
       Dock menu, logout) rather than this window closed. */
    ZANT_EVENT_WINDOW_CLOSE_REQUEST = 15,
    /* the system switched between light and dark appearance */
    ZANT_EVENT_APPEARANCE_CHANGE = 16,
};

enum {
    ZANT_KEY_LEFT = 0x1101,
    ZANT_KEY_RIGHT = 0x1102,
    ZANT_KEY_UP = 0x1103,
    ZANT_KEY_DOWN = 0x1104,
    ZANT_KEY_HOME = 0x1105,
    ZANT_KEY_END = 0x1106,
    ZANT_KEY_BACKSPACE = 0x1107,
    ZANT_KEY_DELETE = 0x1108,
    ZANT_KEY_ESCAPE = 0x1109,
    ZANT_KEY_TAB = 0x110A,
    ZANT_KEY_ENTER = 0x110B,
    ZANT_KEY_PAGE_UP = 0x110C,
    ZANT_KEY_PAGE_DOWN = 0x110D,
    ZANT_KEY_SPACE = 0x20, /* also delivered as ' ' */
    ZANT_KEY_F1 = 0x1110,  /* F1 … F12 = ZANT_KEY_F1 + 0 … 11 */
    ZANT_KEY_F12 = 0x111B,
};

enum {
    ZANT_MOD_SHIFT = 1 << 0,
    ZANT_MOD_CONTROL = 1 << 1,
    ZANT_MOD_ALT = 1 << 2,
    ZANT_MOD_META = 1 << 3,
    ZANT_MOD_CAPS_LOCK = 1 << 4,
};

enum {
    ZANT_POINTER_BUTTON_LEFT = 0,
    ZANT_POINTER_BUTTON_RIGHT = 1,
    ZANT_POINTER_BUTTON_MIDDLE = 2,
};

enum {
    ZANT_WINDOW_NATIVE = 0,
    ZANT_WINDOW_HIDDEN_TITLEBAR = 1,
    ZANT_WINDOW_BORDERLESS = 2,
    /* Floating panel (a quick-access popup): no title bar, system rounded
       corners and shadow, above normal windows, no Dock / taskbar entry; on
       macOS it takes keyboard focus without activating the app, so the app
       the user was in stays frontmost. */
    ZANT_WINDOW_PANEL = 3,
};

/* zant_window_place: where in the work area (screen minus menu bar, Dock and
   taskbar) of the screen under the pointer. */
enum {
    ZANT_PLACE_CENTER = 0,
    ZANT_PLACE_TOP_LEFT = 1,
    ZANT_PLACE_TOP_RIGHT = 2,
    ZANT_PLACE_BOTTOM_LEFT = 3,
    ZANT_PLACE_BOTTOM_RIGHT = 4,
    /* the corner by the status bar / tray icons: top right on macOS (under
       the menu bar), bottom right on Windows (above the taskbar; follows a
       taskbar moved to another edge) */
    ZANT_PLACE_TRAY_CORNER = 5,
};

enum {
    ZANT_CURSOR_ARROW = 0,
    ZANT_CURSOR_POINTING_HAND = 1,
    ZANT_CURSOR_IBEAM = 2,
    ZANT_CURSOR_CROSSHAIR = 3,
    ZANT_CURSOR_RESIZE_HORIZONTAL = 4, /* splitters: drag left / right */
    ZANT_CURSOR_RESIZE_VERTICAL = 5,   /* drag up / down */
    ZANT_CURSOR_GRAB = 6,              /* open hand (pannable content) */
    ZANT_CURSOR_GRABBING = 7,          /* closed hand (panning) */
    ZANT_CURSOR_NOT_ALLOWED = 8,
};

/* Answers of ZantHost.titlebar_hit (app-drawn title bars on Windows). */
enum {
    ZANT_HIT_CLIENT = 0,
    ZANT_HIT_CAPTION = 1,  /* drag to move, double click to maximize */
    ZANT_HIT_MAXIMIZE = 2, /* the app-drawn maximize button (Win11 snap layouts) */
};

typedef struct ZantWindowOptions {
    int style;
    float width;
    float height;
    float min_width;
    float min_height;
    uint32_t background;
    int resizable;
    int always_on_top;
    int movable_by_background;
    int center;
    /* hidden_titlebar only: height of the app's own title bar (0 = system
       default); the traffic lights are centred in it on macOS, the drawn
       caption buttons fill it on Windows. */
    float titlebar_height;
    /* create without showing it (zant_window_set_visible / activate later) */
    int hidden;
} ZantWindowOptions;

typedef struct ZantEvent {
    int type;
    float x;
    float y;
    float scroll_x;
    float scroll_y;
    int button;
    int key;
    int modifiers;
    int is_repeat;
    const char *text;
    size_t text_len;
    size_t selection_start;
    size_t selection_end;
    int width;
    int height;
    float dpr;
} ZantEvent;

typedef struct ZantHost {
    void *userdata;
    void (*draw)(void *userdata, ZantCanvas *canvas, float width, float height, float dpr);
    int (*event)(void *userdata, const ZantEvent *event);
    int (*ime_rect)(void *userdata, float *x, float *y, float *width, float *height);
    int (*next_frame_delay)(void *userdata);
    /* ZANT_HIT_* for a point in logical window coordinates (may be NULL). */
    int (*titlebar_hit)(void *userdata, float x, float y);
    /* Called on the UI thread after zant_window_post_wake (may be NULL). */
    void (*wake)(void *userdata);
} ZantHost;

ZantWindow *zant_window_create(const char *title, const ZantWindowOptions *options,
                               const ZantHost *host);
void zant_window_run(ZantWindow *window);
void zant_window_destroy(ZantWindow *window);
void zant_window_request_redraw(ZantWindow *window, int delay_ms);
/* Thread-safe: may be called from any thread. Asks the UI thread to call
   ZantHost.wake once (coalesced) and redraw; used by background work
   (PDF page rendering, decoding) to hand results to the UI. */
void zant_window_post_wake(ZantWindow *window);
void zant_window_set_cursor(ZantWindow *window, int cursor);
/* Window background colour (0xAARRGGBB); on Windows it also picks a light or
   dark system title bar. */
void zant_window_set_background(ZantWindow *window, uint32_t argb);
/* 1 when the system asks for less motion (macOS "Reduce motion", Windows
   "Animation effects" off). */
int zant_window_reduce_motion(void);
/* Unsaved-changes state (dot in the close button on macOS, "*" before the
   title on Windows). */
void zant_window_set_document_edited(ZantWindow *window, int edited);
const char *zant_window_renderer_name(ZantWindow *window);
void zant_window_schedule_snapshot(ZantWindow *window, const char *path, int delay_ms);
/* Window controls for app-drawn title bars. */
void zant_window_minimize(ZantWindow *window);
void zant_window_toggle_maximize(ZantWindow *window);
int zant_window_is_maximized(ZantWindow *window);
/* Asks to close (goes through ZANT_EVENT_WINDOW_CLOSE_REQUEST). */
void zant_window_close(ZantWindow *window);
/* 1 while the window is the active (key) window. */
int zant_window_is_active(ZantWindow *window);

/* Shown / hidden without closing (the window keeps its state). */
void zant_window_set_visible(ZantWindow *window, int visible);
int zant_window_is_visible(ZantWindow *window);
/* Shows the window, brings it to the front and gives it keyboard focus
   (activating the app, except for ZANT_WINDOW_PANEL on macOS). */
void zant_window_activate(ZantWindow *window);
/* Moves the window to a ZANT_PLACE_* spot, `margin` points from the edges. */
void zant_window_place(ZantWindow *window, int place, float margin);

/* Ends the event loop (all windows close) without asking the app again. */
void zant_app_quit(void);
/* macOS: Dock icon and menu bar (regular app) or neither (status bar app);
   Windows: no-op (the taskbar shows visible windows). */
void zant_app_set_dock_visible(int visible);

/* App-level events, delivered on the UI thread. */
enum {
    ZANT_APP_EVENT_TRAY_CLICK = 1, /* Windows: left click on the tray icon */
    ZANT_APP_EVENT_TRAY_MENU = 2,  /* a tray / status bar menu item: id */
    ZANT_APP_EVENT_HOTKEY = 3,     /* a global hotkey: id */
    ZANT_APP_EVENT_REOPEN = 4,     /* macOS: Dock icon clicked (or app reopened) */
    ZANT_APP_EVENT_CLIPBOARD = 5,  /* the clipboard changed (zant_clipboard_watch) */
};
typedef struct ZantAppEvent {
    int type;
    int id;
} ZantAppEvent;
void zant_app_set_handler(void (*handler)(void *userdata, const ZantAppEvent *event), void *userdata);

/* Tray icon (Windows notification area) / status bar item (macOS menu bar).
   rgba: straight-alpha RGBA, width × height pixels, `scale` pixels per
   point (macOS draws it `width / scale` points wide). template_image
   (macOS): only alpha is used, tinted for the menu bar. Items make the menu
   (macOS: shown on click; Windows: on right click; a left click sends
   ZANT_APP_EVENT_TRAY_CLICK). Calling it again replaces icon and menu. */
typedef struct ZantTrayItem {
    const char *title; /* UTF-8; NULL or "" with separator */
    int id;
    int separator;
    int checked;
    int disabled;
} ZantTrayItem;
int zant_tray_set(const uint8_t *rgba, int width, int height, float scale, int template_image,
                  const char *tooltip, const ZantTrayItem *items, int count);
void zant_tray_remove(void);
/* Pixel size of the tray icon on this display (Windows: the small icon
   size, 16 at 100%; macOS: 44, a 22 pt menu bar icon at @2x). */
int zant_tray_icon_pixels(void);

/* System-wide hotkey (works while other apps are frontmost): key is a
   ZANT_KEY_* or a lowercase ASCII letter / digit, modifiers ZANT_MOD_*
   (at least one of control / alt / meta). Returns 0 when the combination is
   taken by another app or not supported. */
int zant_hotkey_register(int id, int key, int modifiers);
void zant_hotkey_unregister(int id);

/* 1 when the system uses the dark appearance (macOS Dark Mode, Windows
   "Choose your app mode: Dark"). */
int zant_system_dark_mode(void);

int zant_clipboard_set_text(const char *text, size_t length);
/* Changes each time anything (any app) writes to the clipboard. */
uint64_t zant_clipboard_change_count(void);
/* Sends ZANT_APP_EVENT_CLIPBOARD after each clipboard change, including
   this app's own writes (Windows: clipboard listener; macOS: the change
   count checked on the UI thread every 0.3 s). */
void zant_clipboard_watch(int enable);
/* Returns the number of UTF-8 bytes required, including the trailing NUL. When
 * out is non-NULL, copies up to capacity bytes including the NUL. */
size_t zant_clipboard_get_text(char *out, size_t capacity);

/* Native dialogs. Paths are UTF-8; free results with zant_dialog_free.
 * extensions: comma separated ("md,markdown,txt"); NULL or "" allows any. */
char *zant_open_file_dialog_ext(const char *extensions);
char *zant_save_file_dialog_ext(const char *default_name, const char *extensions);
void zant_dialog_free(char *value);
/* Returns 0 for the first button, 1 for the second, 2 for the third. */
int zant_confirm_dialog(const char *title, const char *message, const char *first,
                        const char *second, const char *third);
int zant_open_url(const char *url);

#ifdef __cplusplus
}
#endif

#endif
