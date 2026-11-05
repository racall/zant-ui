#import <AppKit/AppKit.h>
#import <CoreGraphics/CoreGraphics.h>
#import <Metal/Metal.h>
#import <QuartzCore/QuartzCore.h>
#import <dispatch/dispatch.h>

#include "src/platform/macos/window.h"
#include "src/platform/platform.h"
#include "src/renderer/skia/skia_bridge.h"

#include <math.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct ZantMacWindow ZantMacWindow;

@interface ZantView : NSView <NSTextInputClient>
@property(nonatomic, assign) ZantMacWindow *macWindow;
@end

struct ZantMacWindow {
    ZantHost host;
    ZantRenderer *renderer;
    __unsafe_unretained NSWindow *window;
    __unsafe_unretained ZantView *view;
    int width_px;
    int height_px;
    bool closing;
    bool in_draw;
    char backend_name[64];
};

static NSWindow *g_window = nil;
static ZantView *g_view = nil;
static id g_app_delegate = nil;
static NSTimer *g_redraw_timer = nil;
static unsigned long g_redraw_generation = 0;
static bool g_gpu_render_pending = false;

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
        case 51: return ZANT_KEY_BACKSPACE;
        case 117: return ZANT_KEY_DELETE;
        case 53: return ZANT_KEY_ESCAPE;
        case 48: return ZANT_KEY_TAB;
        case 36:
        case 76: return ZANT_KEY_ENTER;
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

static void zant_send_window_event(ZantMacWindow *window, int type) {
    if (!window || !window->host.event) return;
    ZantEvent event;
    memset(&event, 0, sizeof(event));
    event.type = type;
    event.width = (int)lround(NSWidth(window->view.bounds));
    event.height = (int)lround(NSHeight(window->view.bounds));
    event.dpr = (float)window->view.window.backingScaleFactor;
    window->host.event(window->host.userdata, &event);
}

static void zant_render_window(ZantMacWindow *window);

static void zant_schedule_redraw(ZantMacWindow *window, int delay_ms) {
    if (!window || window->closing || !window->view) return;
    [g_redraw_timer invalidate];
    g_redraw_timer = nil;
    g_redraw_generation++;
    if (delay_ms <= 0) {
        if (window->renderer && zant_renderer_is_gpu(window->renderer)) {
            if (!g_gpu_render_pending) {
                g_gpu_render_pending = true;
                dispatch_async(dispatch_get_main_queue(), ^{
                    g_gpu_render_pending = false;
                    if (!window->closing) zant_render_window(window);
                });
            }
        } else {
            [window->view setNeedsDisplay:YES];
        }
        return;
    }
    const unsigned long generation = g_redraw_generation;
    NSTimeInterval interval = (NSTimeInterval)delay_ms / 1000.0;
    if (interval < 0.001) interval = 0.001;
    g_redraw_timer = [NSTimer scheduledTimerWithTimeInterval:interval
                                                     repeats:NO
                                                       block:^(NSTimer *timer) {
        (void)timer;
        if (generation != g_redraw_generation) return;
        g_redraw_timer = nil;
        if (!window->closing && window->view) {
            if (window->renderer && zant_renderer_is_gpu(window->renderer)) {
                zant_render_window(window);
            } else {
                [window->view setNeedsDisplay:YES];
            }
        }
    }];
}

static CGImageRef zant_create_cgimage(ZantMacWindow *window, size_t *out_width,
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

static void zant_render_window(ZantMacWindow *window) {
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
        if (delay >= 0) {
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
    self.macWindow->host.event(self.macWindow->host.userdata, &z);
    zant_request_view_redraw(self);
}

- (void)mouseEntered:(NSEvent *)event {
    [self sendPointer:event type:ZANT_EVENT_POINTER_ENTER button:ZANT_POINTER_BUTTON_LEFT];
}

- (void)mouseExited:(NSEvent *)event {
    [self sendPointer:event type:ZANT_EVENT_POINTER_LEAVE button:ZANT_POINTER_BUTTON_LEFT];
    if (self.macWindow) zant_macos_set_cursor(self.macWindow, ZANT_CURSOR_ARROW);
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
    z.scroll_x = (float)event.scrollingDeltaX;
    z.scroll_y = (float)event.scrollingDeltaY;
    z.modifiers = zant_event_modifiers(event.modifierFlags);
    z.width = (int)lround(NSWidth(self.bounds));
    z.height = (int)lround(NSHeight(self.bounds));
    z.dpr = (float)self.window.backingScaleFactor;
    self.macWindow->host.event(self.macWindow->host.userdata, &z);
    zant_request_view_redraw(self);
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

@interface ZantAppDelegate : NSObject <NSApplicationDelegate, NSWindowDelegate>
@end

@implementation ZantAppDelegate

- (BOOL)applicationShouldTerminateAfterLastWindowClosed:(NSApplication *)sender {
    (void)sender;
    return YES;
}

- (void)windowWillClose:(NSNotification *)notification {
    (void)notification;
    ZantMacWindow *window = g_view ? g_view.macWindow : NULL;
    if (window) {
        window->closing = true;
        zant_send_window_event(window, ZANT_EVENT_WINDOW_CLOSE);
    }
    [NSApp terminate:nil];
}

- (void)windowDidResignKey:(NSNotification *)notification {
    (void)notification;
    ZantMacWindow *window = g_view ? g_view.macWindow : NULL;
    if (window) {
        zant_send_window_event(window, ZANT_EVENT_WINDOW_BLUR);
        zant_request_view_redraw(g_view);
    }
}

- (void)windowDidBecomeKey:(NSNotification *)notification {
    (void)notification;
    ZantMacWindow *window = g_view ? g_view.macWindow : NULL;
    if (window) {
        zant_send_window_event(window, ZANT_EVENT_WINDOW_FOCUS);
    }
}

- (void)windowDidResize:(NSNotification *)notification {
    (void)notification;
    ZantMacWindow *window = g_view ? g_view.macWindow : NULL;
    if (window) {
        zant_send_window_event(window, ZANT_EVENT_WINDOW_RESIZE);
        zant_request_view_redraw(g_view);
    }
}

@end

static void zant_install_menu(void) {
    NSMenu *main_menu = [[NSMenu alloc] init];
    NSMenuItem *app_item = [[NSMenuItem alloc] init];
    [main_menu addItem:app_item];
    NSMenu *app_menu = [[NSMenu alloc] init];
    NSMenuItem *quit_item = [[NSMenuItem alloc] initWithTitle:@"Quit Zant UI"
                                                        action:@selector(terminate:)
                                                 keyEquivalent:@"q"];
    [app_menu addItem:quit_item];
    app_item.submenu = app_menu;
    [NSApp setMainMenu:main_menu];
}

ZantMacWindow *zant_macos_window_create(const char *title, const ZantWindowOptions *options,
                                       const ZantHost *host) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
        zant_install_menu();

        ZantMacWindow *window = calloc(1, sizeof(ZantMacWindow));
        if (!window) return NULL;
        window->host = *host;
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
        }
        g_window = [[NSWindow alloc] initWithContentRect:frame
                                               styleMask:mask
                                                 backing:NSBackingStoreBuffered
                                                   defer:NO];
        if (!g_window) {
            free(window);
            return NULL;
        }
        g_window.title = [NSString stringWithUTF8String:title ? title : "Zant UI"];
        g_window.minSize = NSMakeSize(options->min_width, options->min_height);
        g_window.releasedWhenClosed = NO;
        g_window.hasShadow = YES;
        const uint32_t bg = options->background;
        g_window.backgroundColor = [NSColor colorWithSRGBRed:((bg >> 16) & 0xff) / 255.0
                                                       green:((bg >> 8) & 0xff) / 255.0
                                                        blue:(bg & 0xff) / 255.0
                                                       alpha:((bg >> 24) & 0xff) / 255.0];
        g_window.opaque = ((bg >> 24) & 0xff) == 0xff;
        if (options->movable_by_background) g_window.movableByWindowBackground = YES;
        if (options->always_on_top) [g_window setLevel:NSFloatingWindowLevel];
        if (style == ZANT_WINDOW_HIDDEN_TITLEBAR) {
            g_window.titlebarAppearsTransparent = YES;
            g_window.titleVisibility = NSWindowTitleHidden;
        } else if (style == ZANT_WINDOW_BORDERLESS) {
            g_window.titleVisibility = NSWindowTitleHidden;
            if (options->movable_by_background) g_window.movableByWindowBackground = YES;
        } else {
            g_window.titlebarAppearsTransparent = NO;
        }

        g_view = [[ZantView alloc] initWithFrame:frame];
        g_view.macWindow = window;
        g_window.contentView = g_view;
        if (options->center) [g_window center];

        g_app_delegate = [[ZantAppDelegate alloc] init];
        g_window.delegate = g_app_delegate;
        NSApp.delegate = g_app_delegate;

        window->window = g_window;
        window->view = g_view;

        const CGFloat scale = g_window.backingScaleFactor;
        window->width_px = (int)lround(width * scale);
        window->height_px = (int)lround(height * scale);

        const char *requested = getenv("ZANT_RENDERER");
        const bool force_raster = requested && strcmp(requested, "raster") == 0;
        const bool want_metal = !force_raster;
        if (want_metal) {
            g_view.wantsLayer = YES;
            CAMetalLayer *metal_layer = (CAMetalLayer *)g_view.layer;
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
                g_window.contentView = fallback_view;
                g_view = fallback_view;
                window->view = fallback_view;
            }
        }
        if (!window->renderer) {
            window->renderer = zant_renderer_create_raster(window->width_px, window->height_px);
        }
        if (!window->renderer) {
            g_window = nil;
            g_view = nil;
            free(window);
            return NULL;
        }
        snprintf(window->backend_name, sizeof(window->backend_name), "%s",
                 zant_renderer_backend_name(window->renderer));

        [g_window makeKeyAndOrderFront:nil];
        [NSApp activateIgnoringOtherApps:YES];
        [g_window makeFirstResponder:g_view];
        zant_request_view_redraw(g_view);
        return window;
    }
}

void zant_macos_run(ZantMacWindow *window) {
    (void)window;
    @autoreleasepool {
        [NSApp run];
    }
}

void zant_macos_destroy(ZantMacWindow *window) {
    if (!window) return;
    [g_redraw_timer invalidate];
    g_redraw_timer = nil;
    if (window->renderer) {
        zant_renderer_destroy(window->renderer);
        window->renderer = NULL;
    }
    if (g_window) {
        g_window.delegate = nil;
        [g_window close];
    }
    g_window = nil;
    g_view = nil;
    g_app_delegate = nil;
    NSApp.delegate = nil;
    free(window);
}

void zant_macos_request_redraw(ZantMacWindow *window, int delay_ms) {
    zant_schedule_redraw(window, delay_ms);
}

void zant_macos_set_cursor(ZantMacWindow *window, int cursor) {
    (void)window;
    switch (cursor) {
        case ZANT_CURSOR_POINTING_HAND:
            [[NSCursor pointingHandCursor] set];
            break;
        case ZANT_CURSOR_IBEAM:
            [[NSCursor IBeamCursor] set];
            break;
        default:
            [[NSCursor arrowCursor] set];
            break;
    }
}

const char *zant_macos_renderer_name(ZantMacWindow *window) {
    if (!window) return "none";
    return window->backend_name;
}

void zant_macos_schedule_snapshot(ZantMacWindow *window, const char *path, int delay_ms) {
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
        } else {
            [window->view display];
            zant_render_window(window);
            if (zant_renderer_write_png(window->renderer, target.UTF8String)) {
                fprintf(stderr, "[zant] snapshot: %s\n", target.UTF8String);
            } else {
                fprintf(stderr, "[zant] snapshot failed: %s\n", target.UTF8String);
            }
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
