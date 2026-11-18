#import "window_internal.h"
#import <QuartzCore/QuartzCore.h>

#include <math.h>
#include <string.h>

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

@implementation ZantView {
    /* the input method changed the text during the current keyDown */
    BOOL _imeResponded;
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
    /* panels and overlays work while another app stays active: hover must
       reach them without the app being active */
    const bool panel_like = self.macWindow && zant_style_is_panel(self.macWindow->style);
    NSTrackingAreaOptions options = NSTrackingMouseEnteredAndExited | NSTrackingMouseMoved |
                                    (panel_like ? NSTrackingActiveAlways : NSTrackingActiveInKeyWindow) |
                                    NSTrackingInVisibleRect;
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

- (void)viewDidChangeEffectiveAppearance {
    [super viewDidChangeEffectiveAppearance];
    if (self.macWindow) {
        zant_send_window_event(self.macWindow, ZANT_EVENT_APPEARANCE_CHANGE);
        zant_request_view_redraw(self);
    }
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
        _imeResponded = NO;
        [self interpretKeyEvents:@[ event ]];
        /* Esc during composition: input methods (Pinyin, Japanese) drop the
           marked text without calling unmarkText or setMarkedText:@"". Clear
           it here, or the next Esc arrives as an inserted U+001B. */
        if (!_imeResponded && _hasMarkedText && event.keyCode == 53) [self unmarkText];
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

/* The Edit menu's standard actions arrive here (the menu takes ⌘C / ⌘V /
   … before keyDown): forward them as the key presses the runtime handles. */
- (void)zant_sendMenuKey:(int)key modifiers:(int)modifiers {
    if (!self.macWindow || !self.macWindow->host.event) return;
    ZantEvent z;
    memset(&z, 0, sizeof(z));
    z.key = key;
    z.modifiers = modifiers;
    z.width = (int)lround(NSWidth(self.bounds));
    z.height = (int)lround(NSHeight(self.bounds));
    z.dpr = (float)self.window.backingScaleFactor;
    z.type = ZANT_EVENT_KEY_DOWN;
    const int handled = self.macWindow->host.event(self.macWindow->host.userdata, &z);
    z.type = ZANT_EVENT_KEY_UP;
    self.macWindow->host.event(self.macWindow->host.userdata, &z);
    if (handled) zant_request_view_redraw(self);
}
- (void)copy:(id)sender { [self zant_sendMenuKey:'c' modifiers:ZANT_MOD_META]; }
- (void)cut:(id)sender { [self zant_sendMenuKey:'x' modifiers:ZANT_MOD_META]; }
- (void)paste:(id)sender { [self zant_sendMenuKey:'v' modifiers:ZANT_MOD_META]; }
- (void)selectAll:(id)sender { [self zant_sendMenuKey:'a' modifiers:ZANT_MOD_META]; }
- (void)undo:(id)sender { [self zant_sendMenuKey:'z' modifiers:ZANT_MOD_META]; }
- (void)redo:(id)sender { [self zant_sendMenuKey:'z' modifiers:ZANT_MOD_META | ZANT_MOD_SHIFT]; }

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
    _imeResponded = YES;
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
    _imeResponded = YES;
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
    _imeResponded = YES;
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
