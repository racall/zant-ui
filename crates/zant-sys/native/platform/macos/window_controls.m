#import "window_internal.h"

#include <float.h>
#include <math.h>

void zant_window_set_background(ZantWindow *window, uint32_t bg) {
    if (!window || !window->window) return;
    window->window.backgroundColor = [NSColor colorWithSRGBRed:((bg >> 16) & 0xff) / 255.0
                                                         green:((bg >> 8) & 0xff) / 255.0
                                                          blue:(bg & 0xff) / 255.0
                                                         alpha:((bg >> 24) & 0xff) / 255.0];
}

void zant_window_set_cursor(ZantWindow *window, int cursor) {
    (void)window;
    switch (cursor) {
        case ZANT_CURSOR_POINTING_HAND:
            [[NSCursor pointingHandCursor] set];
            break;
        case ZANT_CURSOR_IBEAM:
            [[NSCursor IBeamCursor] set];
            break;
        case ZANT_CURSOR_CROSSHAIR:
            [[NSCursor crosshairCursor] set];
            break;
        case ZANT_CURSOR_RESIZE_HORIZONTAL:
            [[NSCursor resizeLeftRightCursor] set];
            break;
        case ZANT_CURSOR_RESIZE_VERTICAL:
            [[NSCursor resizeUpDownCursor] set];
            break;
        case ZANT_CURSOR_GRAB:
            [[NSCursor openHandCursor] set];
            break;
        case ZANT_CURSOR_GRABBING:
            [[NSCursor closedHandCursor] set];
            break;
        case ZANT_CURSOR_NOT_ALLOWED:
            [[NSCursor operationNotAllowedCursor] set];
            break;
        default:
            [[NSCursor arrowCursor] set];
            break;
    }
}

void zant_window_minimize(ZantWindow *window) {
    if (window && window->window) [window->window miniaturize:nil];
}

void zant_window_toggle_maximize(ZantWindow *window) {
    if (window && window->window) [window->window zoom:nil];
}

int zant_window_is_maximized(ZantWindow *window) {
    return window && window->window && window->window.isZoomed ? 1 : 0;
}

void zant_window_set_title(ZantWindow *window, const char *title) {
    if (!window || !window->window || !title) return;
    NSString *text = [NSString stringWithUTF8String:title];
    if (text && ![window->window.title isEqualToString:text]) window->window.title = text;
}

void zant_window_set_size(ZantWindow *window, float width, float height) {
    if (!window || !window->window || width <= 0 || height <= 0) return;
    NSWindow *win = window->window;
    if (win.styleMask & NSWindowStyleMaskFullScreen) return;
    const NSSize min = win.contentMinSize;
    const NSSize max = win.contentMaxSize;
    width = (float)fmax(width, min.width);
    height = (float)fmax(height, min.height);
    if (max.width > 0) width = (float)fmin(width, max.width);
    if (max.height > 0) height = (float)fmin(height, max.height);
    /* AppKit's origin is the bottom-left: keep the top edge where it is */
    NSRect content = [win contentRectForFrameRect:win.frame];
    const CGFloat top = NSMaxY(content);
    content.size = NSMakeSize(width, height);
    content.origin.y = top - height;
    [win setFrame:[win frameRectForContentRect:content] display:YES animate:NO];
}

void zant_window_set_min_size(ZantWindow *window, float width, float height) {
    if (!window || !window->window) return;
    window->window.contentMinSize = NSMakeSize(fmax(width, 0), fmax(height, 0));
}

void zant_window_set_max_size(ZantWindow *window, float width, float height) {
    if (!window || !window->window) return;
    window->window.contentMaxSize = NSMakeSize(width > 0 ? width : FLT_MAX, height > 0 ? height : FLT_MAX);
}

void zant_window_set_always_on_top(ZantWindow *window, int on) {
    if (!window || !window->window || zant_style_is_panel(window->style)) return;
    window->window.level = on ? NSFloatingWindowLevel : NSNormalWindowLevel;
}

void zant_window_set_fullscreen(ZantWindow *window, int on) {
    if (!window || !window->window || zant_style_is_panel(window->style) ||
        window->style == ZANT_WINDOW_OVERLAY) {
        return;
    }
    NSWindow *win = window->window;
    const BOOL full = (win.styleMask & NSWindowStyleMaskFullScreen) != 0;
    if ((on != 0) == full) return;
    win.collectionBehavior |= NSWindowCollectionBehaviorFullScreenPrimary;
    [win toggleFullScreen:nil];
}

int zant_window_is_fullscreen(ZantWindow *window) {
    return window && window->window && (window->window.styleMask & NSWindowStyleMaskFullScreen) ? 1 : 0;
}

int zant_window_is_active(ZantWindow *window) {
    return window && window->window && window->window.isKeyWindow ? 1 : 0;
}
