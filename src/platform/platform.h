#ifndef ZANT_PLATFORM_H
#define ZANT_PLATFORM_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct ZantCanvas ZantCanvas;
typedef struct ZantMacWindow ZantMacWindow;

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
} ZantHost;

ZantMacWindow *zant_macos_window_create(const char *title, const ZantWindowOptions *options,
                                       const ZantHost *host);
void zant_macos_run(ZantMacWindow *window);
void zant_macos_destroy(ZantMacWindow *window);
void zant_macos_request_redraw(ZantMacWindow *window, int delay_ms);
void zant_macos_set_cursor(ZantMacWindow *window, int cursor);
const char *zant_macos_renderer_name(ZantMacWindow *window);
void zant_macos_schedule_snapshot(ZantMacWindow *window, const char *path, int delay_ms);

int zant_clipboard_set_text(const char *text, size_t length);
/* Returns the number of UTF-8 bytes required, including the trailing NUL. When
 * out is non-NULL, copies up to capacity bytes including the NUL. */
size_t zant_clipboard_get_text(char *out, size_t capacity);

#ifdef __cplusplus
}
#endif

#endif
