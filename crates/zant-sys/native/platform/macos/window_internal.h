#ifndef ZANT_MACOS_WINDOW_INTERNAL_H
#define ZANT_MACOS_WINDOW_INTERNAL_H

#import <AppKit/AppKit.h>
#import "window.h"
#import "skia/skia_bridge.h"

#include <stdatomic.h>
#include <stdbool.h>

@interface ZantView : NSView <NSTextInputClient>
@property(nonatomic, assign) ZantWindow *macWindow;
@end

/* One per window (an app may open several: a main window, panels...). */
struct ZantWindow {
    ZantHost host;
    ZantRenderer *renderer;
    NSWindow *window;
    ZantView *view;
    id delegate;
    int width_px;
    int height_px;
    int style;
    bool closing;
    bool in_draw;
    /* asked the app (ZANT_EVENT_WINDOW_CLOSE_REQUEST) and it agreed */
    bool close_approved;
    NSTimer *redraw_timer;
    unsigned long redraw_generation;
    bool gpu_render_pending;
    /* CADisplayLink (macOS 14+) paces animation frames to the display;
       created lazily, paused whenever nothing animates. Falls back to NSTimer. */
    id display_link;
    bool link_active;
    /* Custom title bar height (hidden_titlebar); 0 keeps the system layout. */
    CGFloat titlebar_height;
    atomic_bool wake_pending;
    char backend_name[64];
    ZantWindow *next;
    /* hidden_at_login: shown once launching finished, unless opened at login */
    bool show_after_launch;
};

extern ZantWindow *g_main;
extern ZantWindow *g_windows;
extern bool g_launched;

void zant_app_init(const char *title);
void zant_app_emit(int type, int id);
bool zant_close_vetoed(ZantWindow *window, int quit);
void zant_update_dock(void);
bool zant_style_is_panel(int style);
void zant_send_window_event(ZantWindow *window, int type);
void zant_link_invalidate(ZantWindow *window);
void zant_render_window(ZantWindow *window);
void zant_request_view_redraw(ZantView *view);

#endif
