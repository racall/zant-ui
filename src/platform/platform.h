/* Zant UI platform C ABI: one window per process, implemented by
 * src/platform/macos/window.m (AppKit) and src/platform/windows/window.c
 * (Win32). Coordinates are logical points; dpr converts to pixels. */
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
    /* Close button / ⌘W / Quit / Alt+F4: the handler returns non-zero to veto. */
    ZANT_EVENT_WINDOW_CLOSE_REQUEST = 15,
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

int zant_clipboard_set_text(const char *text, size_t length);
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
