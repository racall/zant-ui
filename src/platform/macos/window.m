#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>
#import <CoreGraphics/CoreGraphics.h>
#import <Metal/Metal.h>
#import <QuartzCore/QuartzCore.h>
#import <dispatch/dispatch.h>

#include "src/platform/macos/window.h"
#include "src/platform/platform.h"
#include "src/renderer/skia/skia_bridge.h"

#include <math.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct ZantWindow ZantWindow;

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
};

static id g_app_delegate = nil;
/* The first window: closing it ends the app; app-level requests (Quit, the
   Dock icon) go to it. */
static ZantWindow *g_main = NULL;
static ZantWindow *g_windows = NULL;
/* zant_app_quit: terminate without asking the main window again */
static bool g_quitting = false;

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
static int zant_event_modifiers(NSEventModifierFlags flags) {
    int mods = 0;
    if (flags & NSEventModifierFlagShift) mods |= ZANT_MOD_SHIFT;
    if (flags & NSEventModifierFlagControl) mods |= ZANT_MOD_CONTROL;
    if (flags & NSEventModifierFlagOption) mods |= ZANT_MOD_ALT;
    if (flags & NSEventModifierFlagCommand) mods |= ZANT_MOD_META;
    if (flags & NSEventModifierFlagCapsLock) mods |= ZANT_MOD_CAPS_LOCK;
    return mods;
}

static int zant_key_from_event(NSEvent *event) {
    switch (event.keyCode) {
        case 123: return ZANT_KEY_LEFT;
        case 124: return ZANT_KEY_RIGHT;
        case 125: return ZANT_KEY_DOWN;
        case 126: return ZANT_KEY_UP;
        case 115: return ZANT_KEY_HOME;
        case 119: return ZANT_KEY_END;
        case 116: return ZANT_KEY_PAGE_UP;
        case 121: return ZANT_KEY_PAGE_DOWN;
        case 51: return ZANT_KEY_BACKSPACE;
        case 117: return ZANT_KEY_DELETE;
        case 53: return ZANT_KEY_ESCAPE;
        case 48: return ZANT_KEY_TAB;
        case 36:
        case 76: return ZANT_KEY_ENTER;
        case 122: return ZANT_KEY_F1;
        case 120: return ZANT_KEY_F1 + 1;
        case 99: return ZANT_KEY_F1 + 2;
        case 118: return ZANT_KEY_F1 + 3;
        case 96: return ZANT_KEY_F1 + 4;
        case 97: return ZANT_KEY_F1 + 5;
        case 98: return ZANT_KEY_F1 + 6;
        case 100: return ZANT_KEY_F1 + 7;
        case 101: return ZANT_KEY_F1 + 8;
        case 109: return ZANT_KEY_F1 + 9;
        case 103: return ZANT_KEY_F1 + 10;
        case 111: return ZANT_KEY_F1 + 11;
        default: break;
    }
    NSString *chars = event.charactersIgnoringModifiers;
    if (chars.length == 0) return 0;
    unichar c = [chars characterAtIndex:0];
    if (c < 128) {
        if (c >= 'A' && c <= 'Z') c = (unichar)(c - 'A' + 'a');
        return (int)c;
    }
    return (int)c;
}

static void zant_send_window_event(ZantWindow *window, int type) {
    if (!window || !window->host.event) return;
    ZantEvent event;
    memset(&event, 0, sizeof(event));
    event.type = type;
    event.width = (int)lround(NSWidth(window->view.bounds));
    event.height = (int)lround(NSHeight(window->view.bounds));
    event.dpr = (float)window->view.window.backingScaleFactor;
    window->host.event(window->host.userdata, &event);
}

static void zant_render_window(ZantWindow *window);

static void zant_link_set_active(ZantWindow *window, bool active) {
    if (active && !window->display_link && window->view &&
        [window->view respondsToSelector:@selector(displayLinkWithTarget:selector:)]) {
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunguarded-availability-new"
        CADisplayLink *link = [window->view displayLinkWithTarget:window->view
                                                         selector:@selector(zantDisplayLinkFired:)];
        [link addToRunLoop:[NSRunLoop mainRunLoop] forMode:NSRunLoopCommonModes];
        link.paused = YES;
        window->display_link = link;
#pragma clang diagnostic pop
    }
    if (!window->display_link) {
        window->link_active = false;
        return;
    }
    if (window->link_active == active) return;
    window->link_active = active;
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunguarded-availability-new"
    [(CADisplayLink *)window->display_link setPaused:!active];
#pragma clang diagnostic pop
}

static void zant_link_invalidate(ZantWindow *window) {
    if (window->display_link) {
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunguarded-availability-new"
        [(CADisplayLink *)window->display_link invalidate];
#pragma clang diagnostic pop
    }
    window->display_link = nil;
    window->link_active = false;
}

static void zant_schedule_redraw(ZantWindow *window, int delay_ms) {
    if (!window || window->closing || !window->view) return;
    /* the running display link renders on the next vsync anyway */
    if (delay_ms <= 0 && window->link_active) return;
    [window->redraw_timer invalidate];
    window->redraw_timer = nil;
    window->redraw_generation++;
    if (delay_ms <= 0) {
        if (window->renderer && zant_renderer_is_gpu(window->renderer)) {
            if (!window->gpu_render_pending) {
                window->gpu_render_pending = true;
                dispatch_async(dispatch_get_main_queue(), ^{
                    window->gpu_render_pending = false;
                    if (!window->closing) zant_render_window(window);
                });
            }
        } else {
            [window->view setNeedsDisplay:YES];
        }
        return;
    }
    const unsigned long generation = window->redraw_generation;
    NSTimeInterval interval = (NSTimeInterval)delay_ms / 1000.0;
    if (interval < 0.001) interval = 0.001;
    window->redraw_timer = [NSTimer scheduledTimerWithTimeInterval:interval
                                                     repeats:NO
                                                       block:^(NSTimer *timer) {
        (void)timer;
        if (generation != window->redraw_generation) return;
        window->redraw_timer = nil;
        if (!window->closing && window->view) {
            if (window->renderer && zant_renderer_is_gpu(window->renderer)) {
                zant_render_window(window);
            } else {
                [window->view setNeedsDisplay:YES];
            }
        }
    }];
}

static CGImageRef zant_create_cgimage(ZantWindow *window, size_t *out_width,
                                     size_t *out_height) {
    int width = 0;
    int height = 0;
    int stride = 0;
    void *pixels = zant_renderer_pixels(window->renderer, &width, &height, &stride);
    if (!pixels || width <= 0 || height <= 0 || stride <= 0) return NULL;
    const size_t byte_count = (size_t)stride * (size_t)height;
    CGDataProviderRef provider = CGDataProviderCreateWithData(NULL, pixels, byte_count, NULL);
    if (!provider) return NULL;
    CGColorSpaceRef color_space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    CGBitmapInfo bitmap_info = kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big;
    CGImageRef image = CGImageCreate((size_t)width, (size_t)height, 8, 32, (size_t)stride,
                                     color_space, bitmap_info, provider, NULL, false,
                                     kCGRenderingIntentDefault);
    CGColorSpaceRelease(color_space);
    CGDataProviderRelease(provider);
    if (out_width) *out_width = (size_t)width;
    if (out_height) *out_height = (size_t)height;
    return image;
}

static void zant_render_window(ZantWindow *window) {
    if (!window || !window->host.draw || window->in_draw) return;
    window->in_draw = true;

    const CGFloat scale = window->view.window.backingScaleFactor;
    const CGFloat logical_width = NSWidth(window->view.bounds);
    const CGFloat logical_height = NSHeight(window->view.bounds);
    int pixel_width = (int)lround(logical_width * scale);
    int pixel_height = (int)lround(logical_height * scale);
    if (pixel_width < 1) pixel_width = 1;
    if (pixel_height < 1) pixel_height = 1;

    if (!window->renderer) {
        window->renderer = zant_renderer_create_raster(pixel_width, pixel_height);
        if (window->renderer) {
            snprintf(window->backend_name, sizeof(window->backend_name), "%s",
                     zant_renderer_backend_name(window->renderer));
        }
    } else if (pixel_width != window->width_px || pixel_height != window->height_px) {
        zant_renderer_resize(window->renderer, pixel_width, pixel_height);
    }
    window->width_px = pixel_width;
    window->height_px = pixel_height;

    if (window->renderer) {
        ZantCanvas *canvas = zant_renderer_begin(window->renderer, (float)scale);
        if (canvas) {
            window->host.draw(window->host.userdata, canvas, (float)logical_width,
                              (float)logical_height, (float)scale);
        }
        zant_renderer_end(window->renderer);

        if (!zant_renderer_is_gpu(window->renderer)) {
            size_t image_width = 0;
            size_t image_height = 0;
            CGImageRef image = zant_create_cgimage(window, &image_width, &image_height);
            if (image) {
                NSImage *ns_image = [[NSImage alloc] initWithCGImage:image
                                                                size:NSMakeSize(logical_width,
                                                                                logical_height)];
                [ns_image drawInRect:window->view.bounds
                            fromRect:NSZeroRect
                           operation:NSCompositingOperationCopy
                            fraction:1.0
                      respectFlipped:YES
                               hints:nil];
                CGImageRelease(image);
            }
        }
    }

    window->in_draw = false;
    if (window->host.next_frame_delay) {
        const int delay = window->host.next_frame_delay(window->host.userdata);
        /* animations run on the display link; slower work (caret blink,
           tooltip delay) on a one-shot timer */
        zant_link_set_active(window, delay > 0 && delay <= 16 && !window->closing);
        if (window->link_active) {
            [window->redraw_timer invalidate];
            window->redraw_timer = nil;
            window->redraw_generation++;
        } else if (delay >= 0) {
            zant_schedule_redraw(window, delay);
        }
    }
}

static void zant_request_view_redraw(ZantView *view) {
    if (!view || !view.macWindow) return;
    zant_schedule_redraw(view.macWindow, 0);
}

@implementation ZantView {
    BOOL _hasMarkedText;
    NSRange _markedSelection;
    NSTrackingArea *_trackingArea;
}

- (BOOL)acceptsFirstResponder {
    return YES;
}

- (void)zantDisplayLinkFired:(id)link {
    (void)link;
    ZantWindow *window = self.macWindow;
    if (!window || window->closing) return;
    if (window->renderer && zant_renderer_is_gpu(window->renderer)) {
        zant_render_window(window);
    } else {
        [self setNeedsDisplay:YES];
    }
}

- (BOOL)acceptsFirstMouse:(NSEvent *)event {
    (void)event;
    return YES;
}

- (BOOL)isFlipped {
    return YES;
}

- (CALayer *)makeBackingLayer {
    return [CAMetalLayer layer];
}

- (BOOL)becomeFirstResponder {
    BOOL ok = [super becomeFirstResponder];
    if (ok) {
        [[self inputContext] activate];
    }
    return ok;
}

- (BOOL)resignFirstResponder {
    [[self inputContext] deactivate];
    return [super resignFirstResponder];
}

- (void)updateTrackingAreas {
    [super updateTrackingAreas];
    if (_trackingArea) {
        [self removeTrackingArea:_trackingArea];
    }
    NSTrackingAreaOptions options = NSTrackingMouseEnteredAndExited | NSTrackingMouseMoved |
                                    NSTrackingActiveInKeyWindow | NSTrackingInVisibleRect;
    _trackingArea = [[NSTrackingArea alloc] initWithRect:self.bounds
                                                 options:options
                                                   owner:self
                                                userInfo:nil];
    [self addTrackingArea:_trackingArea];
}

- (void)viewDidMoveToWindow {
    [super viewDidMoveToWindow];
    if (self.window) {
        [self.window makeFirstResponder:self];
    }
}

- (void)setFrameSize:(NSSize)newSize {
    [super setFrameSize:newSize];
    zant_request_view_redraw(self);
}

- (void)viewDidChangeBackingProperties {
    [super viewDidChangeBackingProperties];
    zant_request_view_redraw(self);
}

- (NSPoint)zant_mouse_point:(NSEvent *)event {
    return [self convertPoint:event.locationInWindow fromView:nil];
}

- (ZantEvent)zant_pointer_event:(NSEvent *)event type:(int)type button:(int)button {
    ZantEvent z;
    memset(&z, 0, sizeof(z));
    NSPoint p = [self zant_mouse_point:event];
    z.type = type;
    z.x = (float)p.x;
    z.y = (float)p.y;
    z.button = button;
    z.modifiers = zant_event_modifiers(event.modifierFlags);
    z.width = (int)lround(NSWidth(self.bounds));
    z.height = (int)lround(NSHeight(self.bounds));
    z.dpr = (float)self.window.backingScaleFactor;
    return z;
}

- (void)sendPointer:(NSEvent *)event type:(int)type button:(int)button {
    if (!self.macWindow || !self.macWindow->host.event) return;
    ZantEvent z = [self zant_pointer_event:event type:type button:button];
    if (self.macWindow->host.event(self.macWindow->host.userdata, &z)) zant_request_view_redraw(self);
}

- (void)mouseEntered:(NSEvent *)event {
    [self sendPointer:event type:ZANT_EVENT_POINTER_ENTER button:ZANT_POINTER_BUTTON_LEFT];
}

- (void)mouseExited:(NSEvent *)event {
    [self sendPointer:event type:ZANT_EVENT_POINTER_LEAVE button:ZANT_POINTER_BUTTON_LEFT];
    if (self.macWindow) zant_window_set_cursor(self.macWindow, ZANT_CURSOR_ARROW);
}

- (void)mouseMoved:(NSEvent *)event {
    [self sendPointer:event type:ZANT_EVENT_POINTER_MOVE button:ZANT_POINTER_BUTTON_LEFT];
}

- (void)mouseDown:(NSEvent *)event {
    [self.window makeFirstResponder:self];
    [self sendPointer:event type:ZANT_EVENT_POINTER_DOWN button:ZANT_POINTER_BUTTON_LEFT];
}

- (void)mouseUp:(NSEvent *)event {
    [self sendPointer:event type:ZANT_EVENT_POINTER_UP button:ZANT_POINTER_BUTTON_LEFT];
}

- (void)mouseDragged:(NSEvent *)event {
    [self sendPointer:event type:ZANT_EVENT_POINTER_MOVE button:ZANT_POINTER_BUTTON_LEFT];
}

- (void)rightMouseDown:(NSEvent *)event {
    [self.window makeFirstResponder:self];
    [self sendPointer:event type:ZANT_EVENT_POINTER_DOWN button:ZANT_POINTER_BUTTON_RIGHT];
}

- (void)rightMouseUp:(NSEvent *)event {
    [self sendPointer:event type:ZANT_EVENT_POINTER_UP button:ZANT_POINTER_BUTTON_RIGHT];
}

- (void)scrollWheel:(NSEvent *)event {
    if (!self.macWindow || !self.macWindow->host.event) return;
    ZantEvent z;
    memset(&z, 0, sizeof(z));
    NSPoint p = [self zant_mouse_point:event];
    z.type = ZANT_EVENT_SCROLL;
    z.x = (float)p.x;
    z.y = (float)p.y;
    /* mouse wheels report lines; trackpads and Magic Mouse report points */
    const float line = event.hasPreciseScrollingDeltas ? 1.0f : 16.0f;
    z.scroll_x = (float)event.scrollingDeltaX * line;
    z.scroll_y = (float)event.scrollingDeltaY * line;
    z.modifiers = zant_event_modifiers(event.modifierFlags);
    z.width = (int)lround(NSWidth(self.bounds));
    z.height = (int)lround(NSHeight(self.bounds));
    z.dpr = (float)self.window.backingScaleFactor;
    if (self.macWindow->host.event(self.macWindow->host.userdata, &z)) zant_request_view_redraw(self);
}

/* Trackpad pinch: delivered like browsers do, as a Control + wheel scroll
   (scroll_y = 250 × magnification), so zoomable views handle both alike. */
- (void)magnifyWithEvent:(NSEvent *)event {
    if (!self.macWindow || !self.macWindow->host.event) return;
    ZantEvent z;
    memset(&z, 0, sizeof(z));
    NSPoint p = [self zant_mouse_point:event];
    z.type = ZANT_EVENT_SCROLL;
    z.x = (float)p.x;
    z.y = (float)p.y;
    z.scroll_y = (float)event.magnification * 250.0f;
    z.modifiers = zant_event_modifiers(event.modifierFlags) | ZANT_MOD_CONTROL;
    z.width = (int)lround(NSWidth(self.bounds));
    z.height = (int)lround(NSHeight(self.bounds));
    z.dpr = (float)self.window.backingScaleFactor;
    if (self.macWindow->host.event(self.macWindow->host.userdata, &z)) zant_request_view_redraw(self);
}

- (void)keyDown:(NSEvent *)event {
    if (!self.macWindow || !self.macWindow->host.event) return;
    const int modifiers = zant_event_modifiers(event.modifierFlags);
    ZantEvent z;
    memset(&z, 0, sizeof(z));
    z.type = ZANT_EVENT_KEY_DOWN;
    z.key = zant_key_from_event(event);
    z.modifiers = modifiers;
    z.is_repeat = event.isARepeat ? 1 : 0;
    z.width = (int)lround(NSWidth(self.bounds));
    z.height = (int)lround(NSHeight(self.bounds));
    z.dpr = (float)self.window.backingScaleFactor;

    if (_hasMarkedText) {
        if (modifiers & (ZANT_MOD_META | ZANT_MOD_CONTROL)) {
            if (self.macWindow->host.event(self.macWindow->host.userdata, &z)) {
                zant_request_view_redraw(self);
                return;
            }
        }
        [self interpretKeyEvents:@[ event ]];
        return;
    }

    if (self.macWindow->host.event(self.macWindow->host.userdata, &z)) {
        zant_request_view_redraw(self);
        return;
    }
    if (modifiers & (ZANT_MOD_META | ZANT_MOD_CONTROL)) {
        [super keyDown:event];
        return;
    }
    [self interpretKeyEvents:@[ event ]];
}

- (void)keyUp:(NSEvent *)event {
    if (!self.macWindow || !self.macWindow->host.event) return;
    ZantEvent z;
    memset(&z, 0, sizeof(z));
    z.type = ZANT_EVENT_KEY_UP;
    z.key = zant_key_from_event(event);
    z.modifiers = zant_event_modifiers(event.modifierFlags);
    z.width = (int)lround(NSWidth(self.bounds));
    z.height = (int)lround(NSHeight(self.bounds));
    z.dpr = (float)self.window.backingScaleFactor;
    self.macWindow->host.event(self.macWindow->host.userdata, &z);
}

- (void)flagsChanged:(NSEvent *)event {
    (void)event;
}

- (BOOL)hasMarkedText {
    return _hasMarkedText;
}

- (NSRange)markedRange {
    if (!_hasMarkedText) return NSMakeRange(NSNotFound, 0);
    return _markedSelection;
}

- (NSRange)selectedRange {
    if (_hasMarkedText) return _markedSelection;
    return NSMakeRange(NSNotFound, 0);
}

- (void)setMarkedText:(id)string
        selectedRange:(NSRange)selectedRange
     replacementRange:(NSRange)replacementRange {
    (void)replacementRange;
    NSString *text = nil;
    if ([string isKindOfClass:[NSAttributedString class]]) {
        text = [(NSAttributedString *)string string];
    } else if ([string isKindOfClass:[NSString class]]) {
        text = (NSString *)string;
    }
    _hasMarkedText = text.length > 0;
    _markedSelection = selectedRange;
    if (!self.macWindow || !self.macWindow->host.event) return;
    NSData *utf8 = [text dataUsingEncoding:NSUTF8StringEncoding];
    ZantEvent z;
    memset(&z, 0, sizeof(z));
    z.type = ZANT_EVENT_COMPOSITION;
    z.text = (const char *)utf8.bytes;
    z.text_len = (size_t)utf8.length;
    z.selection_start = (size_t)selectedRange.location;
    z.selection_end = (size_t)(selectedRange.location + selectedRange.length);
    z.width = (int)lround(NSWidth(self.bounds));
    z.height = (int)lround(NSHeight(self.bounds));
    z.dpr = (float)self.window.backingScaleFactor;
    self.macWindow->host.event(self.macWindow->host.userdata, &z);
    zant_request_view_redraw(self);
}

- (void)unmarkText {
    _hasMarkedText = NO;
    _markedSelection = NSMakeRange(NSNotFound, 0);
    if (!self.macWindow || !self.macWindow->host.event) return;
    ZantEvent z;
    memset(&z, 0, sizeof(z));
    z.type = ZANT_EVENT_COMPOSITION;
    z.width = (int)lround(NSWidth(self.bounds));
    z.height = (int)lround(NSHeight(self.bounds));
    z.dpr = (float)self.window.backingScaleFactor;
    self.macWindow->host.event(self.macWindow->host.userdata, &z);
    zant_request_view_redraw(self);
}

- (void)insertText:(id)string replacementRange:(NSRange)replacementRange {
    (void)replacementRange;
    NSString *text = nil;
    if ([string isKindOfClass:[NSAttributedString class]]) {
        text = [(NSAttributedString *)string string];
    } else if ([string isKindOfClass:[NSString class]]) {
        text = (NSString *)string;
    }
    if (text.length == 0) return;
    _hasMarkedText = NO;
    _markedSelection = NSMakeRange(NSNotFound, 0);
    if (!self.macWindow || !self.macWindow->host.event) return;
    NSData *utf8 = [text dataUsingEncoding:NSUTF8StringEncoding];
    ZantEvent z;
    memset(&z, 0, sizeof(z));
    z.type = ZANT_EVENT_TEXT_INPUT;
    z.text = (const char *)utf8.bytes;
    z.text_len = (size_t)utf8.length;
    z.width = (int)lround(NSWidth(self.bounds));
    z.height = (int)lround(NSHeight(self.bounds));
    z.dpr = (float)self.window.backingScaleFactor;
    self.macWindow->host.event(self.macWindow->host.userdata, &z);
    [[self inputContext] invalidateCharacterCoordinates];
    zant_request_view_redraw(self);
}

- (NSAttributedString *)attributedSubstringForProposedRange:(NSRange)range
                                               actualRange:(NSRangePointer)actualRange {
    (void)range;
    if (actualRange) *actualRange = NSMakeRange(NSNotFound, 0);
    return nil;
}

- (NSArray<NSAttributedStringKey> *)validAttributesForMarkedText {
    return @[ NSUnderlineStyleAttributeName ];
}

- (NSUInteger)characterIndexForPoint:(NSPoint)point {
    (void)point;
    return NSNotFound;
}

- (NSRect)firstRectForCharacterRange:(NSRange)range actualRange:(NSRangePointer)actualRange {
    if (actualRange) *actualRange = range;
    float x = 0;
    float y = 0;
    float w = 1;
    float h = 18;
    bool ok = false;
    if (self.macWindow && self.macWindow->host.ime_rect) {
        ok = self.macWindow->host.ime_rect(self.macWindow->host.userdata, &x, &y, &w, &h) != 0;
    }
    if (!ok) {
        x = (float)NSWidth(self.bounds) * 0.5f;
        y = (float)NSHeight(self.bounds) * 0.5f;
        w = 1;
        h = 18;
    }
    NSPoint window_point = [self convertPoint:NSMakePoint(x, y) toView:nil];
    NSRect window_rect = NSMakeRect(window_point.x, window_point.y, fmax(w, 1), fmax(h, 1));
    NSRect screen_rect = [self.window convertRectToScreen:window_rect];
    return screen_rect;
}

- (void)doCommandBySelector:(SEL)selector {
    (void)selector;
}

- (void)drawRect:(NSRect)dirtyRect {
    (void)dirtyRect;
    if (!self.macWindow) return;
    if (self.macWindow->renderer && zant_renderer_is_gpu(self.macWindow->renderer)) {
        return;
    }
    zant_render_window(self.macWindow);
}

@end

/* ---- app-level events (status bar, hotkeys, Dock) ---- */

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

/* Asks the app (ZANT_EVENT_WINDOW_CLOSE_REQUEST) once per close; returns true
   when it vetoed. quit = 1 when the whole app is asked to quit. */
static bool zant_close_vetoed(ZantWindow *window, int quit) {
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

@interface ZantAppDelegate : NSObject <NSApplicationDelegate>
@end

@implementation ZantAppDelegate

/* The main window ends the app when it closes (windowWillClose); hidden
   windows and status bar apps keep running. */
- (BOOL)applicationShouldTerminateAfterLastWindowClosed:(NSApplication *)sender {
    (void)sender;
    return NO;
}

- (NSApplicationTerminateReply)applicationShouldTerminate:(NSApplication *)sender {
    (void)sender;
    if (g_quitting) return NSTerminateNow;
    return zant_close_vetoed(g_main, 1) ? NSTerminateCancel : NSTerminateNow;
}

/* Dock icon clicked: the app decides (e.g. show its hidden main window);
   without a handler the main window is shown. */
- (BOOL)applicationShouldHandleReopen:(NSApplication *)sender hasVisibleWindows:(BOOL)visible {
    (void)sender;
    (void)visible;
    if (g_app_handler) {
        zant_app_emit(ZANT_APP_EVENT_REOPEN, 0);
    } else if (g_main && !g_main->closing) {
        zant_window_activate(g_main);
    }
    return YES;
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
@end

static void zant_install_menu(const char *title) {
    NSString *name = [NSString stringWithUTF8String:title && title[0] ? title : "Zant UI"];
    NSMenu *main_menu = [[NSMenu alloc] init];
    NSMenuItem *app_item = [[NSMenuItem alloc] init];
    [main_menu addItem:app_item];
    NSMenu *app_menu = [[NSMenu alloc] init];
    NSMenuItem *quit_item = [[NSMenuItem alloc] initWithTitle:[@"Quit " stringByAppendingString:name]
                                                        action:@selector(terminate:)
                                                 keyEquivalent:@"q"];
    [app_menu addItem:quit_item];
    app_item.submenu = app_menu;

    NSMenuItem *file_item = [[NSMenuItem alloc] init];
    [main_menu addItem:file_item];
    NSMenu *file_menu = [[NSMenu alloc] initWithTitle:@"File"];
    NSMenuItem *close_item = [[NSMenuItem alloc] initWithTitle:@"Close Window"
                                                         action:@selector(performClose:)
                                                  keyEquivalent:@"w"];
    [file_menu addItem:close_item];
    file_item.submenu = file_menu;
    [NSApp setMainMenu:main_menu];
}

/* Unbundled runs (`zant run`, zig-out/bin/app) have no AppIcon.icns: show
   the project's assets/icons/app-macos.png in the Dock instead, so the icon
   is visible while developing. Packaged apps use their bundle icon. */
static void zant_apply_dev_icon(void) {
    if ([[NSBundle mainBundle] objectForInfoDictionaryKey:@"CFBundleIconFile"]) return;
    NSString *exe = [[NSBundle mainBundle] executablePath];
    if (!exe) return;
    /* <project>/zig-out/bin/app -> <project>/assets/icons/app-macos.png */
    NSString *project = [[[exe stringByDeletingLastPathComponent] stringByDeletingLastPathComponent]
        stringByDeletingLastPathComponent];
    NSString *path = [project stringByAppendingPathComponent:@"assets/icons/app-macos.png"];
    NSImage *icon = [[NSImage alloc] initWithContentsOfFile:path];
    if (icon) NSApp.applicationIconImage = icon;
}

static void zant_app_init(const char *title) {
    static bool done = false;
    if (done) return;
    done = true;
    [NSApplication sharedApplication];
    [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
    zant_install_menu(title);
    zant_apply_dev_icon();
    g_app_delegate = [[ZantAppDelegate alloc] init];
    NSApp.delegate = g_app_delegate;
}

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
        } else if (style == ZANT_WINDOW_PANEL) {
            /* titled with a hidden title bar: the system's rounded corners
               and shadow; non-activating: keyboard focus without bringing
               the app forward */
            mask = NSWindowStyleMaskTitled | NSWindowStyleMaskFullSizeContentView |
                   NSWindowStyleMaskNonactivatingPanel;
            if (options->resizable) mask |= NSWindowStyleMaskResizable;
        }
        NSWindow *win = nil;
        if (style == ZANT_WINDOW_PANEL) {
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
            win = [[NSWindow alloc] initWithContentRect:frame
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
        win.hasShadow = YES;
        const uint32_t bg = options->background;
        win.backgroundColor = [NSColor colorWithSRGBRed:((bg >> 16) & 0xff) / 255.0
                                                  green:((bg >> 8) & 0xff) / 255.0
                                                   blue:(bg & 0xff) / 255.0
                                                  alpha:((bg >> 24) & 0xff) / 255.0];
        win.opaque = ((bg >> 24) & 0xff) == 0xff;
        if (options->movable_by_background) win.movableByWindowBackground = YES;
        if (options->always_on_top && style != ZANT_WINDOW_PANEL) [win setLevel:NSFloatingWindowLevel];
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

        if (!options->hidden) {
            if (style == ZANT_WINDOW_PANEL) {
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

void zant_window_run(ZantWindow *window) {
    (void)window;
    @autoreleasepool {
        [NSApp run];
    }
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

void zant_window_request_redraw(ZantWindow *window, int delay_ms) {
    zant_schedule_redraw(window, delay_ms);
}

void zant_window_post_wake(ZantWindow *window) {
    if (!window) return;
    /* coalesce: one main-queue block per burst of wakes */
    if (atomic_exchange(&window->wake_pending, true)) return;
    dispatch_async(dispatch_get_main_queue(), ^{
        atomic_store(&window->wake_pending, false);
        if (window->closing) return;
        if (window->host.wake) window->host.wake(window->host.userdata);
        zant_schedule_redraw(window, 0);
    });
}

/* ---- showing, hiding and placing windows ---- */

void zant_window_set_visible(ZantWindow *window, int visible) {
    if (!window || !window->window || window->closing) return;
    if (!visible) {
        [window->window orderOut:nil];
        return;
    }
    if (window->style == ZANT_WINDOW_PANEL) {
        [window->window orderFrontRegardless];
    } else {
        [window->window orderFront:nil];
    }
    zant_request_view_redraw(window->view);
}

int zant_window_is_visible(ZantWindow *window) {
    return window && window->window && window->window.isVisible ? 1 : 0;
}

void zant_window_activate(ZantWindow *window) {
    if (!window || !window->window || window->closing) return;
    if (window->style == ZANT_WINDOW_PANEL) {
        [window->window orderFrontRegardless];
        [window->window makeKeyWindow];
    } else {
        if (window->window.isMiniaturized) [window->window deminiaturize:nil];
        [NSApp activateIgnoringOtherApps:YES];
        [window->window makeKeyAndOrderFront:nil];
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
    if (place == ZANT_PLACE_TRAY_CORNER) place = ZANT_PLACE_TOP_RIGHT;
    CGFloat x = NSMidX(area) - size.width * 0.5;
    CGFloat y = NSMidY(area) - size.height * 0.5;
    if (place == ZANT_PLACE_TOP_LEFT || place == ZANT_PLACE_BOTTOM_LEFT) x = NSMinX(area) + margin;
    if (place == ZANT_PLACE_TOP_RIGHT || place == ZANT_PLACE_BOTTOM_RIGHT) x = NSMaxX(area) - size.width - margin;
    if (place == ZANT_PLACE_TOP_LEFT || place == ZANT_PLACE_TOP_RIGHT) y = NSMaxY(area) - size.height - margin;
    if (place == ZANT_PLACE_BOTTOM_LEFT || place == ZANT_PLACE_BOTTOM_RIGHT) y = NSMinY(area) + margin;
    [window->window setFrameOrigin:NSMakePoint(round(x), round(y))];
}

/* ---- app ---- */

void zant_app_quit(void) {
    g_quitting = true;
    [NSApp terminate:nil];
}

void zant_app_set_dock_visible(int visible) {
    const NSApplicationActivationPolicy policy =
        visible ? NSApplicationActivationPolicyRegular : NSApplicationActivationPolicyAccessory;
    if (NSApp.activationPolicy == policy) return;
    [NSApp setActivationPolicy:policy];
    /* back to a regular app: AppKit may leave it behind other apps */
    if (visible) [NSApp activateIgnoringOtherApps:YES];
}

/* ---- status bar item (menu bar extra) ---- */

@interface ZantTrayTarget : NSObject
@end

@implementation ZantTrayTarget
- (void)itemSelected:(NSMenuItem *)item {
    zant_app_emit(ZANT_APP_EVENT_TRAY_MENU, (int)item.tag);
}
@end

static NSStatusItem *g_status_item = nil;
static ZantTrayTarget *g_tray_target = nil;

int zant_tray_icon_pixels(void) {
    return 44;
}

int zant_tray_set(const uint8_t *rgba, int width, int height, float scale, int template_image,
                  const char *tooltip, const ZantTrayItem *items, int count) {
    @autoreleasepool {
        zant_app_init(NULL);
        if (!g_status_item) {
            g_status_item = [[NSStatusBar systemStatusBar] statusItemWithLength:NSSquareStatusItemLength];
            g_tray_target = [[ZantTrayTarget alloc] init];
        }
        if (rgba && width > 0 && height > 0) {
            NSData *data = [NSData dataWithBytes:rgba length:(size_t)width * (size_t)height * 4];
            CGDataProviderRef provider = CGDataProviderCreateWithCFData((__bridge CFDataRef)data);
            CGColorSpaceRef space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
            CGImageRef cg = CGImageCreate((size_t)width, (size_t)height, 8, 32, (size_t)width * 4, space,
                                          kCGImageAlphaLast | kCGBitmapByteOrder32Big, provider, NULL,
                                          true, kCGRenderingIntentDefault);
            CGColorSpaceRelease(space);
            CGDataProviderRelease(provider);
            if (cg) {
                const CGFloat s = scale > 0 ? scale : 1;
                NSImage *image = [[NSImage alloc] initWithCGImage:cg size:NSMakeSize(width / s, height / s)];
                CGImageRelease(cg);
                image.template = template_image ? YES : NO;
                g_status_item.button.image = image;
            }
        }
        g_status_item.button.toolTip = tooltip ? [NSString stringWithUTF8String:tooltip] : nil;
        NSMenu *menu = [[NSMenu alloc] init];
        menu.autoenablesItems = NO;
        for (int i = 0; i < count; i++) {
            const ZantTrayItem *it = &items[i];
            if (it->separator) {
                [menu addItem:[NSMenuItem separatorItem]];
                continue;
            }
            NSString *title_text = [NSString stringWithUTF8String:it->title ? it->title : ""];
            NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:title_text
                                                          action:@selector(itemSelected:)
                                                   keyEquivalent:@""];
            item.target = g_tray_target;
            item.tag = it->id;
            item.state = it->checked ? NSControlStateValueOn : NSControlStateValueOff;
            item.enabled = it->disabled ? NO : YES;
            [menu addItem:item];
        }
        g_status_item.menu = count > 0 ? menu : nil;
        return 1;
    }
}

void zant_tray_remove(void) {
    if (!g_status_item) return;
    [[NSStatusBar systemStatusBar] removeStatusItem:g_status_item];
    g_status_item = nil;
}

/* ---- global hotkeys (Carbon RegisterEventHotKey: no accessibility
   permission needed) ---- */

enum { ZANT_MAX_HOTKEYS = 16 };
static struct {
    int id;
    EventHotKeyRef ref;
} g_hotkeys[ZANT_MAX_HOTKEYS];
static int g_hotkey_count = 0;
static EventHandlerRef g_hotkey_handler = NULL;

static OSStatus zant_hotkey_pressed(EventHandlerCallRef next, EventRef event, void *userdata) {
    (void)next;
    (void)userdata;
    EventHotKeyID hotkey;
    if (GetEventParameter(event, kEventParamDirectObject, typeEventHotKeyID, NULL, sizeof(hotkey), NULL,
                          &hotkey) == noErr) {
        zant_app_emit(ZANT_APP_EVENT_HOTKEY, (int)hotkey.id);
    }
    return noErr;
}

/* Zant key -> macOS virtual key code (ANSI layout positions). */
static int zant_mac_keycode(int key) {
    static const int letters[26] = {0,  11, 8,  2,  14, 3,  5,  4,  34, 38, 40, 37, 46,
                                    45, 31, 35, 12, 15, 1,  17, 32, 9,  13, 7,  16, 6};
    static const int digits[10] = {29, 18, 19, 20, 21, 23, 22, 26, 28, 25};
    static const int fkeys[12] = {122, 120, 99, 118, 96, 97, 98, 100, 101, 109, 103, 111};
    if (key >= 'a' && key <= 'z') return letters[key - 'a'];
    if (key >= 'A' && key <= 'Z') return letters[key - 'A'];
    if (key >= '0' && key <= '9') return digits[key - '0'];
    if (key >= ZANT_KEY_F1 && key <= ZANT_KEY_F12) return fkeys[key - ZANT_KEY_F1];
    switch (key) {
        case ZANT_KEY_SPACE: return 49;
        case ZANT_KEY_ENTER: return 36;
        case ZANT_KEY_TAB: return 48;
        case ZANT_KEY_ESCAPE: return 53;
        case ZANT_KEY_LEFT: return 123;
        case ZANT_KEY_RIGHT: return 124;
        case ZANT_KEY_DOWN: return 125;
        case ZANT_KEY_UP: return 126;
        case '`': return 50;
        case '-': return 27;
        case '=': return 24;
        case '[': return 33;
        case ']': return 30;
        case ';': return 41;
        case '\'': return 39;
        case ',': return 43;
        case '.': return 47;
        case '/': return 44;
        case '\\': return 42;
        default: return -1;
    }
}

void zant_hotkey_unregister(int id) {
    for (int i = 0; i < g_hotkey_count; i++) {
        if (g_hotkeys[i].id != id) continue;
        UnregisterEventHotKey(g_hotkeys[i].ref);
        g_hotkeys[i] = g_hotkeys[--g_hotkey_count];
        return;
    }
}

int zant_hotkey_register(int id, int key, int modifiers) {
    zant_hotkey_unregister(id);
    if (!(modifiers & (ZANT_MOD_CONTROL | ZANT_MOD_ALT | ZANT_MOD_META))) return 0;
    const int code = zant_mac_keycode(key);
    if (code < 0 || g_hotkey_count >= ZANT_MAX_HOTKEYS) return 0;
    if (!g_hotkey_handler) {
        EventTypeSpec spec = {kEventClassKeyboard, kEventHotKeyPressed};
        InstallApplicationEventHandler(&zant_hotkey_pressed, 1, &spec, NULL, &g_hotkey_handler);
    }
    UInt32 mods = 0;
    if (modifiers & ZANT_MOD_META) mods |= cmdKey;
    if (modifiers & ZANT_MOD_ALT) mods |= optionKey;
    if (modifiers & ZANT_MOD_CONTROL) mods |= controlKey;
    if (modifiers & ZANT_MOD_SHIFT) mods |= shiftKey;
    EventHotKeyID hotkey = {'ZANT', (UInt32)id};
    EventHotKeyRef ref = NULL;
    if (RegisterEventHotKey((UInt32)code, mods, hotkey, GetApplicationEventTarget(), 0, &ref) != noErr) return 0;
    g_hotkeys[g_hotkey_count].id = id;
    g_hotkeys[g_hotkey_count].ref = ref;
    g_hotkey_count++;
    return 1;
}

uint64_t zant_clipboard_change_count(void) {
    return (uint64_t)[NSPasteboard generalPasteboard].changeCount;
}

void zant_window_set_document_edited(ZantWindow *window, int edited) {
    if (!window || !window->window) return;
    window->window.documentEdited = edited ? YES : NO;
    /* AppKit may re-lay the title bar when the close button changes */
    zant_layout_traffic_lights(window);
}

int zant_window_reduce_motion(void) {
    return NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceMotion ? 1 : 0;
}

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

const char *zant_window_renderer_name(ZantWindow *window) {
    if (!window) return "none";
    return window->backend_name;
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

void zant_window_close(ZantWindow *window) {
    if (!window || !window->window || window->closing) return;
    /* performClose: needs a close button (panels and borderless windows have none) */
    if (zant_close_vetoed(window, 0)) return;
    [window->window close];
}

int zant_window_is_active(ZantWindow *window) {
    return window && window->window && window->window.isKeyWindow ? 1 : 0;
}

void zant_window_schedule_snapshot(ZantWindow *window, const char *path, int delay_ms) {
    if (!window || !path) return;
    NSString *target = [NSString stringWithUTF8String:path];
    NSTimeInterval interval = (NSTimeInterval)(delay_ms > 0 ? delay_ms : 0) / 1000.0;
    [NSTimer scheduledTimerWithTimeInterval:interval
                                     repeats:NO
                                       block:^(NSTimer *timer) {
        (void)timer;
        if (!window->view || !window->renderer) return;
        if (zant_renderer_is_gpu(window->renderer)) {
            zant_renderer_capture_next_png(window->renderer, target.UTF8String);
            zant_render_window(window);
            fprintf(stderr, "[zant] snapshot: %s\n", target.UTF8String);
        } else {
            [window->view display];
            zant_render_window(window);
            if (zant_renderer_write_png(window->renderer, target.UTF8String)) {
                fprintf(stderr, "[zant] snapshot: %s\n", target.UTF8String);
            } else {
                fprintf(stderr, "[zant] snapshot failed: %s\n", target.UTF8String);
            }
        }
        // ZANT_SNAPSHOT_EXIT=1: quit once the frame is on disk (automation)
        const char *exit_flag = getenv("ZANT_SNAPSHOT_EXIT");
        if (exit_flag && exit_flag[0] == '1') {
            dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(400 * NSEC_PER_MSEC)),
                           dispatch_get_main_queue(), ^{
                [NSApp terminate:nil];
            });
        }
    }];
}

int zant_clipboard_set_text(const char *text, size_t length) {
    if (!text) return 0;
    @autoreleasepool {
        NSString *string = [[NSString alloc] initWithBytes:text
                                                    length:length
                                                  encoding:NSUTF8StringEncoding];
        if (!string) return 0;
        NSPasteboard *pasteboard = [NSPasteboard generalPasteboard];
        [pasteboard clearContents];
        return [pasteboard setString:string forType:NSPasteboardTypeString] ? 1 : 0;
    }
}

size_t zant_clipboard_get_text(char *out, size_t capacity) {
    @autoreleasepool {
        NSPasteboard *pasteboard = [NSPasteboard generalPasteboard];
        NSString *string = [pasteboard stringForType:NSPasteboardTypeString];
        if (!string) {
            if (out && capacity > 0) out[0] = '\0';
            return 1;
        }
        NSData *data = [string dataUsingEncoding:NSUTF8StringEncoding];
        const size_t required = data.length + 1;
        if (out && capacity > 0) {
            const size_t copy = data.length < capacity - 1 ? data.length : capacity - 1;
            memcpy(out, data.bytes, copy);
            out[copy] = '\0';
        }
        return required;
    }
}

void zant_dialog_free(char *value) {
    free(value);
}

static NSArray<NSString *> *zant_split_extensions(const char *extensions) {
    if (!extensions || extensions[0] == '\0') return nil;
    NSString *joined = [NSString stringWithUTF8String:extensions];
    NSMutableArray<NSString *> *result = [NSMutableArray array];
    for (NSString *part in [joined componentsSeparatedByString:@","]) {
        NSString *trimmed = [part stringByTrimmingCharactersInSet:[NSCharacterSet whitespaceCharacterSet]];
        if (trimmed.length > 0) [result addObject:trimmed];
    }
    return result.count > 0 ? result : nil;
}

/* extensions: comma separated list ("md,txt"); NULL or "" allows any type. */
char *zant_open_file_dialog_ext(const char *extensions) {
    @autoreleasepool {
        NSOpenPanel *panel = [NSOpenPanel openPanel];
        panel.canChooseFiles = YES;
        panel.canChooseDirectories = NO;
        panel.allowsMultipleSelection = NO;
        NSArray<NSString *> *types = zant_split_extensions(extensions);
        if (types) panel.allowedFileTypes = types;
        if ([panel runModal] != NSModalResponseOK) return NULL;
        NSURL *url = panel.URL;
        if (!url) return NULL;
        return strdup(url.path.fileSystemRepresentation);
    }
}

/* extensions: comma separated list ("csv,md"); NULL or "" allows any type. */
char *zant_save_file_dialog_ext(const char *default_name, const char *extensions) {
    @autoreleasepool {
        NSSavePanel *panel = [NSSavePanel savePanel];
        NSArray<NSString *> *types = zant_split_extensions(extensions);
        if (types) panel.allowedFileTypes = types;
        panel.allowsOtherFileTypes = YES;
        if (default_name) {
            panel.nameFieldStringValue = [NSString stringWithUTF8String:default_name];
        }
        if ([panel runModal] != NSModalResponseOK) return NULL;
        NSURL *url = panel.URL;
        if (!url) return NULL;
        return strdup(url.path.fileSystemRepresentation);
    }
}

/* Returns 0 for the first button, 1 for the second, 2 for the third. */
int zant_confirm_dialog(const char *title, const char *message, const char *first,
                        const char *second, const char *third) {
    @autoreleasepool {
        NSAlert *alert = [[NSAlert alloc] init];
        if (title) alert.messageText = [NSString stringWithUTF8String:title];
        if (message) alert.informativeText = [NSString stringWithUTF8String:message];
        if (first) [alert addButtonWithTitle:[NSString stringWithUTF8String:first]];
        if (second) [alert addButtonWithTitle:[NSString stringWithUTF8String:second]];
        if (third) [alert addButtonWithTitle:[NSString stringWithUTF8String:third]];
        const NSModalResponse response = [alert runModal];
        if (response == NSAlertFirstButtonReturn) return 0;
        if (response == NSAlertSecondButtonReturn) return 1;
        return 2;
    }
}

int zant_open_url(const char *url) {
    @autoreleasepool {
        if (!url) return 0;
        NSString *string = [NSString stringWithUTF8String:url];
        NSURL *target = [NSURL URLWithString:string];
        if (!target || !target.scheme) {
            target = [NSURL URLWithString:[@"https://" stringByAppendingString:string]];
        }
        if (!target) return 0;
        return [[NSWorkspace sharedWorkspace] openURL:target] ? 1 : 0;
    }
}
