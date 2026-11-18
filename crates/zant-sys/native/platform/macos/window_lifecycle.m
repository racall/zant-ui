#import "window_internal.h"
#import <Metal/Metal.h>
#import <QuartzCore/QuartzCore.h>
#import <dispatch/dispatch.h>

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* The first window: closing it ends the app; app-level requests (Quit, the
   Dock icon) go to it. */
ZantWindow *g_main = NULL;
ZantWindow *g_windows = NULL;

/* Centres the traffic lights in the app's taller title bar (the approach
   Electron uses for trafficLightPosition). AppKit re-lays the title bar out
   on resize and full screen, so this runs again after those. */
static void zant_layout_traffic_lights(ZantWindow *w) {
    if (!w || !w->window || w->titlebar_height <= 0) return;
    NSWindow *win = w->window;
    const CGFloat titlebar_height = w->titlebar_height;
    if (win.styleMask & NSWindowStyleMaskFullScreen) return;
    NSButton *close = [win standardWindowButton:NSWindowCloseButton];
    NSButton *mini = [win standardWindowButton:NSWindowMiniaturizeButton];
    NSButton *zoom = [win standardWindowButton:NSWindowZoomButton];
    if (!close || !close.superview || !close.superview.superview) return;
    NSView *titlebar = close.superview;
    NSView *container = titlebar.superview;
    NSRect c = container.frame;
    c.size.height = titlebar_height;
    c.origin.y = NSHeight(win.frame) - titlebar_height;
    container.frame = c;
    NSRect t = titlebar.frame;
    t.origin.y = 0;
    t.size.height = titlebar_height;
    titlebar.frame = t;
    NSButton *buttons[3] = {close, mini, zoom};
    CGFloat x = 20;
    for (int i = 0; i < 3; i++) {
        NSButton *b = buttons[i];
        if (!b) continue;
        NSRect f = b.frame;
        [b setFrameOrigin:NSMakePoint(x, round((titlebar_height - NSHeight(f)) * 0.5))];
        x += NSWidth(f) + 6;
    }
}

void zant_send_window_event(ZantWindow *window, int type) {
    if (!window || !window->host.event) return;
    ZantEvent event;
    memset(&event, 0, sizeof(event));
    event.type = type;
    event.width = (int)lround(NSWidth(window->view.bounds));
    event.height = (int)lround(NSHeight(window->view.bounds));
    event.dpr = (float)window->view.window.backingScaleFactor;
    window->host.event(window->host.userdata, &event);
}

/* Asks the app (ZANT_EVENT_WINDOW_CLOSE_REQUEST) once per close; returns true
   when it vetoed. quit = 1 when the whole app is asked to quit. */
bool zant_close_vetoed(ZantWindow *window, int quit) {
    if (!window || window->close_approved) return false;
    if (!window->host.event || window->closing) return false;
    ZantEvent event;
    memset(&event, 0, sizeof(event));
    event.type = ZANT_EVENT_WINDOW_CLOSE_REQUEST;
    event.button = quit;
    event.width = (int)lround(NSWidth(window->view.bounds));
    event.height = (int)lround(NSHeight(window->view.bounds));
    event.dpr = (float)window->window.backingScaleFactor;
    const int veto = window->host.event(window->host.userdata, &event);
    zant_request_view_redraw(window->view);
    if (veto) return true;
    window->close_approved = true;
    return false;
}

@interface ZantWindowDelegate : NSObject <NSWindowDelegate>
@property(nonatomic, assign) ZantWindow *zantWindow;
@end

@implementation ZantWindowDelegate

- (BOOL)windowShouldClose:(NSWindow *)sender {
    (void)sender;
    return zant_close_vetoed(self.zantWindow, 0) ? NO : YES;
}

- (void)windowWillClose:(NSNotification *)notification {
    (void)notification;
    ZantWindow *window = self.zantWindow;
    if (!window || window->closing) return;
    window->closing = true;
    zant_send_window_event(window, ZANT_EVENT_WINDOW_CLOSE);
    zant_link_invalidate(window);
    /* closing the main window ends the app; other windows just close */
    if (window == g_main) [NSApp terminate:nil];
    dispatch_async(dispatch_get_main_queue(), ^{ zant_update_dock(); });
}

- (void)windowDidResignKey:(NSNotification *)notification {
    (void)notification;
    ZantWindow *window = self.zantWindow;
    if (!window || window->closing) return;
    zant_send_window_event(window, ZANT_EVENT_WINDOW_BLUR);
    zant_request_view_redraw(window->view);
}

- (void)windowDidBecomeKey:(NSNotification *)notification {
    (void)notification;
    ZantWindow *window = self.zantWindow;
    if (!window || window->closing) return;
    zant_send_window_event(window, ZANT_EVENT_WINDOW_FOCUS);
}

- (void)windowDidExitFullScreen:(NSNotification *)notification {
    (void)notification;
    zant_layout_traffic_lights(self.zantWindow);
}

- (void)windowDidResize:(NSNotification *)notification {
    (void)notification;
    ZantWindow *window = self.zantWindow;
    if (!window || window->closing) return;
    zant_layout_traffic_lights(window);
    zant_send_window_event(window, ZANT_EVENT_WINDOW_RESIZE);
    zant_request_view_redraw(window->view);
}

@end

/* Borderless / panel windows refuse key status by default; a panel must take
   the keyboard (arrows, Esc) while another app stays active. */
@interface ZantPanel : NSPanel
@end

@implementation ZantPanel
- (BOOL)canBecomeKeyWindow {
    return YES;
}
- (BOOL)canBecomeMainWindow {
    return NO;
}
/* an overlay covers the menu bar too: no constraining below it */
- (NSRect)constrainFrameRect:(NSRect)frameRect toScreen:(NSScreen *)screen {
    if (self.level >= NSScreenSaverWindowLevel) return frameRect;
    return [super constrainFrameRect:frameRect toScreen:screen];
}
@end

/* Panels and overlays: NSPanels that take the keyboard without activating
   the app and have no Dock presence. */
bool zant_style_is_panel(int style) {
    return style == ZANT_WINDOW_PANEL || style == ZANT_WINDOW_OVERLAY;
}

/* Offscreen snapshot sessions can have no NSScreen. AppKit's default
   constraint then shrinks every window to its minimum frame. Keep the
   requested frame when there is no screen to constrain it to. */
@interface ZantNativeWindow : NSWindow
@end
@implementation ZantNativeWindow
- (NSRect)constrainFrameRect:(NSRect)frameRect toScreen:(NSScreen *)screen {
    if (!screen && NSScreen.screens.count == 0) return frameRect;
    return [super constrainFrameRect:frameRect toScreen:screen];
}
@end

ZantWindow *zant_window_create(const char *title, const ZantWindowOptions *options,
                               const ZantHost *host) {
    @autoreleasepool {
        zant_app_init(title);

        ZantWindow *window = calloc(1, sizeof(ZantWindow));
        if (!window) return NULL;
        window->host = *host;
        window->style = options->style;
        atomic_init(&window->wake_pending, false);
        snprintf(window->backend_name, sizeof(window->backend_name), "Skia Raster (CPU)");

        const int style = options->style;
        const float width = options->width;
        const float height = options->height;
        NSRect frame = NSMakeRect(0, 0, width > 0 ? width : 900, height > 0 ? height : 640);
        NSWindowStyleMask mask = NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                                 NSWindowStyleMaskMiniaturizable;
        if (options->resizable) mask |= NSWindowStyleMaskResizable;
        if (style == ZANT_WINDOW_HIDDEN_TITLEBAR) {
            mask |= NSWindowStyleMaskFullSizeContentView;
        } else if (style == ZANT_WINDOW_BORDERLESS) {
            mask = NSWindowStyleMaskBorderless | NSWindowStyleMaskResizable;
        } else if (style == ZANT_WINDOW_OVERLAY) {
            mask = NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel;
        } else if (style == ZANT_WINDOW_PANEL) {
            /* titled with a hidden title bar: the system's rounded corners
               and shadow; non-activating: keyboard focus without bringing
               the app forward */
            mask = NSWindowStyleMaskTitled | NSWindowStyleMaskFullSizeContentView |
                   NSWindowStyleMaskNonactivatingPanel;
            if (options->resizable) mask |= NSWindowStyleMaskResizable;
        }
        NSWindow *win = nil;
        if (style == ZANT_WINDOW_OVERLAY) {
            ZantPanel *overlay = [[ZantPanel alloc] initWithContentRect:frame
                                                              styleMask:mask
                                                                backing:NSBackingStoreBuffered
                                                                  defer:NO];
            overlay.floatingPanel = YES;
            overlay.hidesOnDeactivate = NO;
            overlay.becomesKeyOnlyIfNeeded = NO;
            overlay.level = NSScreenSaverWindowLevel;
            overlay.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces |
                                         NSWindowCollectionBehaviorFullScreenAuxiliary |
                                         NSWindowCollectionBehaviorStationary |
                                         NSWindowCollectionBehaviorIgnoresCycle;
            overlay.movable = NO;
            overlay.animationBehavior = NSWindowAnimationBehaviorNone;
            win = overlay;
        } else if (style == ZANT_WINDOW_PANEL) {
            ZantPanel *panel = [[ZantPanel alloc] initWithContentRect:frame
                                                            styleMask:mask
                                                              backing:NSBackingStoreBuffered
                                                                defer:NO];
            panel.floatingPanel = YES;
            panel.hidesOnDeactivate = NO;
            panel.becomesKeyOnlyIfNeeded = NO;
            panel.level = NSStatusWindowLevel;
            panel.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces |
                                       NSWindowCollectionBehaviorFullScreenAuxiliary |
                                       NSWindowCollectionBehaviorTransient;
            panel.titlebarAppearsTransparent = YES;
            panel.titleVisibility = NSWindowTitleHidden;
            panel.movable = NO;
            for (NSWindowButton b = NSWindowCloseButton; b <= NSWindowZoomButton; b++) {
                [[panel standardWindowButton:b] setHidden:YES];
            }
            win = panel;
        } else {
            win = [[ZantNativeWindow alloc] initWithContentRect:frame
                                              styleMask:mask
                                                backing:NSBackingStoreBuffered
                                                  defer:NO];
        }
        if (!win) {
            free(window);
            return NULL;
        }
        win.title = [NSString stringWithUTF8String:title ? title : "Zant UI"];
        win.minSize = NSMakeSize(options->min_width, options->min_height);
        win.releasedWhenClosed = NO;
        win.hasShadow = style != ZANT_WINDOW_OVERLAY;
        const uint32_t bg = options->background;
        win.backgroundColor = [NSColor colorWithSRGBRed:((bg >> 16) & 0xff) / 255.0
                                                  green:((bg >> 8) & 0xff) / 255.0
                                                   blue:(bg & 0xff) / 255.0
                                                  alpha:((bg >> 24) & 0xff) / 255.0];
        win.opaque = ((bg >> 24) & 0xff) == 0xff;
        if (options->movable_by_background) win.movableByWindowBackground = YES;
        if (options->always_on_top && !zant_style_is_panel(style)) [win setLevel:NSFloatingWindowLevel];
        if (style == ZANT_WINDOW_HIDDEN_TITLEBAR) {
            win.titlebarAppearsTransparent = YES;
            win.titleVisibility = NSWindowTitleHidden;
            window->titlebar_height = options->titlebar_height > 0 ? options->titlebar_height : 0;
        } else if (style == ZANT_WINDOW_BORDERLESS) {
            win.titleVisibility = NSWindowTitleHidden;
            if (options->movable_by_background) win.movableByWindowBackground = YES;
        } else if (style == ZANT_WINDOW_NATIVE) {
            win.titlebarAppearsTransparent = NO;
        }

        ZantView *view = [[ZantView alloc] initWithFrame:frame];
        view.macWindow = window;
        win.contentView = view;
        if (options->center) [win center];

        ZantWindowDelegate *delegate = [[ZantWindowDelegate alloc] init];
        delegate.zantWindow = window;
        win.delegate = delegate;
        window->delegate = delegate;
        window->window = win;
        window->view = view;

        const CGFloat scale = win.backingScaleFactor;
        window->width_px = (int)lround(frame.size.width * scale);
        window->height_px = (int)lround(frame.size.height * scale);

        const char *requested = getenv("ZANT_RENDERER");
        const bool force_raster = requested && strcmp(requested, "raster") == 0;
        if (!force_raster) {
            view.wantsLayer = YES;
            CAMetalLayer *metal_layer = (CAMetalLayer *)view.layer;
            metal_layer.device = MTLCreateSystemDefaultDevice();
            metal_layer.pixelFormat = MTLPixelFormatBGRA8Unorm;
            metal_layer.framebufferOnly = YES;
            metal_layer.opaque = YES;
            metal_layer.contentsScale = scale;
            metal_layer.drawableSize = CGSizeMake((CGFloat)window->width_px, (CGFloat)window->height_px);
            window->renderer = zant_renderer_create_metal((__bridge void *)metal_layer,
                                                          window->width_px, window->height_px);
            if (!window->renderer) {
                fprintf(stderr, "[zant] Metal backend unavailable; falling back to raster\n");
                ZantView *fallback_view = [[ZantView alloc] initWithFrame:frame];
                fallback_view.macWindow = window;
                win.contentView = fallback_view;
                window->view = fallback_view;
            }
        }
        if (!window->renderer) {
            window->renderer = zant_renderer_create_raster(window->width_px, window->height_px);
        }
        if (!window->renderer) {
            win.delegate = nil;
            window->window = nil;
            window->view = nil;
            window->delegate = nil;
            free(window);
            return NULL;
        }
        snprintf(window->backend_name, sizeof(window->backend_name), "%s",
                 zant_renderer_backend_name(window->renderer));

        window->next = g_windows;
        g_windows = window;
        if (!g_main) g_main = window;

        const bool wait_for_launch = options->hidden_at_login && !g_launched;
        if (wait_for_launch) window->show_after_launch = true;
        if (!options->hidden && !wait_for_launch) {
            if (zant_style_is_panel(style)) {
                [win orderFrontRegardless];
                [win makeKeyWindow];
            } else {
                [win makeKeyAndOrderFront:nil];
                [NSApp activateIgnoringOtherApps:YES];
            }
        }
        zant_layout_traffic_lights(window);
        [win makeFirstResponder:window->view];
        zant_request_view_redraw(window->view);
        return window;
    }
}

void zant_window_set_autosave(ZantWindow *window, const char *name) {
    if (!window || !window->window) return;
    NSString *key = name && name[0] ? [NSString stringWithUTF8String:name] : @"";
    if (!key || [window->window.frameAutosaveName isEqualToString:key]) return;
    [window->window setFrameAutosaveName:key];
}

void zant_window_destroy(ZantWindow *window) {
    if (!window) return;
    [window->redraw_timer invalidate];
    window->redraw_timer = nil;
    zant_link_invalidate(window);
    window->closing = true;
    if (window->renderer) {
        zant_renderer_destroy(window->renderer);
        window->renderer = NULL;
    }
    if (window->window) {
        window->window.delegate = nil;
        [window->window orderOut:nil];
        [window->window close];
    }
    for (ZantWindow **it = &g_windows; *it; it = &(*it)->next) {
        if (*it == window) {
            *it = window->next;
            break;
        }
    }
    if (g_main == window) g_main = NULL;
    window->view.macWindow = NULL;
    window->window = nil;
    window->view = nil;
    window->delegate = nil;
    window->display_link = nil;
    free(window);
}

/* ---- showing, hiding and placing windows ---- */

void zant_window_set_visible(ZantWindow *window, int visible) {
    if (!window || !window->window || window->closing) return;
    window->show_after_launch = false;
    if (!visible) {
        [window->window orderOut:nil];
        zant_update_dock();
        return;
    }
    if (zant_style_is_panel(window->style)) {
        [window->window orderFrontRegardless];
    } else {
        [window->window orderFront:nil];
        zant_update_dock();
    }
    zant_request_view_redraw(window->view);
}

int zant_window_is_visible(ZantWindow *window) {
    return window && window->window && window->window.isVisible ? 1 : 0;
}

int zant_window_is_minimized(ZantWindow *window) {
    return window && window->window && window->window.isMiniaturized ? 1 : 0;
}

void zant_window_activate(ZantWindow *window) {
    if (!window || !window->window || window->closing) return;
    if (zant_style_is_panel(window->style)) {
        [window->window orderFrontRegardless];
        [window->window makeKeyWindow];
    } else {
        window->show_after_launch = false;
        if (window->window.isMiniaturized) [window->window deminiaturize:nil];
        [NSApp activateIgnoringOtherApps:YES];
        [window->window makeKeyAndOrderFront:nil];
        zant_update_dock();
    }
    [window->window makeFirstResponder:window->view];
    zant_request_view_redraw(window->view);
}

void zant_window_place(ZantWindow *window, int place, float margin) {
    if (!window || !window->window) return;
    NSPoint mouse = [NSEvent mouseLocation];
    NSScreen *screen = [NSScreen mainScreen];
    for (NSScreen *candidate in [NSScreen screens]) {
        if (NSMouseInRect(mouse, candidate.frame, NO)) {
            screen = candidate;
            break;
        }
    }
    if (!screen) return;
    /* Cocoa: origin at the bottom left; visibleFrame excludes menu bar and Dock */
    const NSRect area = screen.visibleFrame;
    const NSSize size = window->window.frame.size;
    if (place == ZANT_PLACE_POINTER) {
        /* Cocoa's y grows upwards: "below the pointer" is a smaller y */
        CGFloat x = mouse.x + margin;
        CGFloat top = mouse.y - margin;
        if (x + size.width > NSMaxX(area)) x = mouse.x - margin - size.width;
        if (top - size.height < NSMinY(area)) top = mouse.y + margin + size.height;
        x = fmin(fmax(x, NSMinX(area)), NSMaxX(area) - size.width);
        top = fmax(fmin(top, NSMaxY(area)), NSMinY(area) + size.height);
        [window->window setFrameOrigin:NSMakePoint(round(x), round(top - size.height))];
        return;
    }
    if (place == ZANT_PLACE_TRAY_CORNER) place = ZANT_PLACE_TOP_RIGHT;
    CGFloat x = NSMidX(area) - size.width * 0.5;
    CGFloat y = NSMidY(area) - size.height * 0.5;
    if (place == ZANT_PLACE_TOP_LEFT || place == ZANT_PLACE_BOTTOM_LEFT) x = NSMinX(area) + margin;
    if (place == ZANT_PLACE_TOP_RIGHT || place == ZANT_PLACE_BOTTOM_RIGHT) x = NSMaxX(area) - size.width - margin;
    if (place == ZANT_PLACE_TOP_LEFT || place == ZANT_PLACE_TOP_RIGHT) y = NSMaxY(area) - size.height - margin;
    if (place == ZANT_PLACE_BOTTOM_LEFT || place == ZANT_PLACE_BOTTOM_RIGHT) y = NSMinY(area) + margin;
    [window->window setFrameOrigin:NSMakePoint(round(x), round(y))];
}

void zant_window_cover_screen(ZantWindow *window, int index) {
    if (!window || !window->window || window->closing) return;
    NSArray<NSScreen *> *screens = [NSScreen screens];
    if (index < 0 || (NSUInteger)index >= screens.count) return;
    window->show_after_launch = false;
    [window->window setFrame:screens[(NSUInteger)index].frame display:NO];
    [window->window orderFrontRegardless];
    [window->window makeKeyWindow];
    [window->window makeFirstResponder:window->view];
    zant_request_view_redraw(window->view);
}

void zant_window_set_document_edited(ZantWindow *window, int edited) {
    if (!window || !window->window) return;
    window->window.documentEdited = edited ? YES : NO;
    /* AppKit may re-lay the title bar when the close button changes */
    zant_layout_traffic_lights(window);
}

void zant_window_close(ZantWindow *window) {
    if (!window || !window->window || window->closing) return;
    /* performClose: needs a close button (panels and borderless windows have none) */
    if (zant_close_vetoed(window, 0)) return;
    [window->window close];
}
