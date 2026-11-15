/* Screens, other apps' windows, screen capture and text recognition (macOS).
 *
 * Desktop units are points in CoreGraphics global coordinates: origin at the
 * top left of the primary screen, y down (the coordinates CGWindowList and
 * ScreenCaptureKit use). Cocoa frames (origin bottom left) are flipped
 * against the primary screen's height.
 */

#import <AppKit/AppKit.h>
#import <CoreGraphics/CoreGraphics.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <Vision/Vision.h>

#include <stdlib.h>
#include <string.h>

#include "../platform.h"

/* ---- screens ---- */

static CGFloat zant_primary_height(void) {
    NSArray<NSScreen *> *screens = [NSScreen screens];
    return screens.count ? screens[0].frame.size.height : 0;
}

static uint32_t zant_screen_number(NSScreen *screen) {
    NSNumber *number = screen.deviceDescription[@"NSScreenNumber"];
    return number ? number.unsignedIntValue : 0;
}

static NSRect zant_flip(NSRect frame, CGFloat primary_height) {
    return NSMakeRect(frame.origin.x, primary_height - NSMaxY(frame), frame.size.width, frame.size.height);
}

static int zant_screen_list_main(ZantScreenInfo *out, int capacity) {
    NSArray<NSScreen *> *screens = [NSScreen screens];
    const CGFloat primary_height = zant_primary_height();
    int count = 0;
    for (NSScreen *screen in screens) {
        if (out && count < capacity) {
            const NSRect frame = zant_flip(screen.frame, primary_height);
            const NSRect work = zant_flip(screen.visibleFrame, primary_height);
            ZantScreenInfo *info = &out[count];
            info->id = zant_screen_number(screen);
            info->x = (float)frame.origin.x;
            info->y = (float)frame.origin.y;
            info->width = (float)frame.size.width;
            info->height = (float)frame.size.height;
            info->work_x = (float)work.origin.x;
            info->work_y = (float)work.origin.y;
            info->work_width = (float)work.size.width;
            info->work_height = (float)work.size.height;
            info->scale = (float)screen.backingScaleFactor;
            info->units_per_point = 1.0f;
            info->primary = count == 0;
        }
        count++;
    }
    return count;
}

int zant_screen_list(ZantScreenInfo *out, int capacity) {
    if ([NSThread isMainThread]) return zant_screen_list_main(out, capacity);
    __block int count = 0;
    dispatch_sync(dispatch_get_main_queue(), ^{
        count = zant_screen_list_main(out, capacity);
    });
    return count;
}

/* ---- windows ---- */

int zant_window_list(ZantWindowInfo *out, int capacity) {
    @autoreleasepool {
        CFArrayRef list = CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID);
        if (!list) return 0;
        NSArray *windows = CFBridgingRelease(list);
        int count = 0;
        for (NSDictionary *window in windows) {
            NSNumber *layer = window[(id)kCGWindowLayer];
            if (!layer || layer.intValue != 0) continue;
            NSNumber *alpha = window[(id)kCGWindowAlpha];
            if (alpha && alpha.doubleValue <= 0.0) continue;
            CGRect bounds;
            NSDictionary *dict = window[(id)kCGWindowBounds];
            if (!dict || !CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)dict, &bounds)) continue;
            if (!isfinite(bounds.origin.x) || bounds.size.width < 40 || bounds.size.height < 40) continue;
            if (out && count < capacity) {
                ZantWindowInfo *info = &out[count];
                info->x = (float)bounds.origin.x;
                info->y = (float)bounds.origin.y;
                info->width = (float)bounds.size.width;
                info->height = (float)bounds.size.height;
                info->pid = [window[(id)kCGWindowOwnerPID] unsignedIntValue];
                NSString *owner = window[(id)kCGWindowOwnerName];
                info->app[0] = 0;
                if (owner) strlcpy(info->app, owner.UTF8String ?: "", sizeof(info->app));
            }
            count++;
        }
        return count;
    }
}

/* ---- capture ---- */

int zant_screen_capture_access(void) {
    return CGPreflightScreenCaptureAccess() ? 1 : 0;
}

int zant_screen_capture_request(void) {
    if (CGPreflightScreenCaptureAccess()) return 1;
    return CGRequestScreenCaptureAccess() ? 1 : 0;
}

void zant_capture_free(uint8_t *pixels) {
    free(pixels);
}

/* Premultiplied RGBA (zant_image_from_pixels order on macOS). */
static uint8_t *zant_cgimage_pixels(CGImageRef image, int *width, int *height, int *stride) {
    const size_t w = CGImageGetWidth(image);
    const size_t h = CGImageGetHeight(image);
    if (!w || !h) return NULL;
    const size_t row = w * 4;
    uint8_t *pixels = malloc(row * h);
    if (!pixels) return NULL;
    CGColorSpaceRef space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    CGContextRef context = CGBitmapContextCreate(pixels, w, h, 8, row, space,
                                                 kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big);
    CGColorSpaceRelease(space);
    if (!context) {
        free(pixels);
        return NULL;
    }
    CGContextSetBlendMode(context, kCGBlendModeCopy);
    CGContextDrawImage(context, CGRectMake(0, 0, w, h), image);
    CGContextRelease(context);
    *width = (int)w;
    *height = (int)h;
    *stride = (int)row;
    return pixels;
}

int zant_screen_capture_all(ZantCapture *out, int capacity) {
    if (!out || capacity <= 0 || [NSThread isMainThread]) return 0;
    if (!CGPreflightScreenCaptureAccess()) return 0;
    if (@available(macOS 14.0, *)) {
        ZantScreenInfo screens[16];
        const int screen_count = MIN(zant_screen_list(screens, 16), MIN(capacity, 16));
        if (screen_count <= 0) return 0;

        __block SCShareableContent *content = nil;
        dispatch_semaphore_t got = dispatch_semaphore_create(0);
        [SCShareableContent getShareableContentExcludingDesktopWindows:NO
                                                   onScreenWindowsOnly:YES
                                                     completionHandler:^(SCShareableContent *result, NSError *error) {
            (void)error;
            content = result;
            dispatch_semaphore_signal(got);
        }];
        if (dispatch_semaphore_wait(got, dispatch_time(DISPATCH_TIME_NOW, 3 * NSEC_PER_SEC)) != 0 || !content) return 0;

        /* every display at once: the slowest one sets the latency */
        dispatch_group_t group = dispatch_group_create();
        for (int i = 0; i < screen_count; i++) {
            out[i].screen_id = screens[i].id;
            out[i].pixels = NULL;
            out[i].width = out[i].height = out[i].stride = 0;
            SCDisplay *display = nil;
            for (SCDisplay *candidate in content.displays) {
                if (candidate.displayID == screens[i].id) display = candidate;
            }
            if (!display) continue;
            SCContentFilter *filter = [[SCContentFilter alloc] initWithDisplay:display excludingWindows:@[]];
            SCStreamConfiguration *config = [[SCStreamConfiguration alloc] init];
            config.width = (size_t)lround(screens[i].width * screens[i].scale);
            config.height = (size_t)lround(screens[i].height * screens[i].scale);
            config.showsCursor = NO;
            config.capturesAudio = NO;
            config.pixelFormat = 'BGRA';
            if (@available(macOS 14.2, *)) config.captureResolution = SCCaptureResolutionBest;
            ZantCapture *slot = &out[i];
            dispatch_group_enter(group);
            [SCScreenshotManager captureImageWithFilter:filter
                                          configuration:config
                                      completionHandler:^(CGImageRef image, NSError *error) {
                (void)error;
                if (image) slot->pixels = zant_cgimage_pixels(image, &slot->width, &slot->height, &slot->stride);
                dispatch_group_leave(group);
            }];
        }
        if (dispatch_group_wait(group, DISPATCH_TIME_FOREVER) != 0) {
            /* a late completion would write into `out` after we return */
            return 0;
        }
        int ok = 0;
        for (int i = 0; i < screen_count; i++) {
            if (out[i].pixels) ok++;
        }
        if (ok == 0) return 0;
        return screen_count;
    }
    return 0;
}

int zant_local_utc_offset(void) {
    return (int)[[NSTimeZone localTimeZone] secondsFromGMT];
}

/* ---- text recognition ---- */

int zant_ocr_available(void) {
    return 1;
}

typedef struct {
    char *text;
    size_t text_len, text_cap;
    ZantOcrLine *lines;
    size_t line_count, line_cap;
    ZantOcrToken *tokens;
    size_t token_count, token_cap;
} ZantOcrBuilder;

static bool zant_grow(void **items, size_t *cap, size_t need, size_t size) {
    if (need <= *cap) return true;
    size_t next = *cap ? *cap * 2 : 64;
    while (next < need) next *= 2;
    void *grown = realloc(*items, next * size);
    if (!grown) return false;
    *items = grown;
    *cap = next;
    return true;
}

/* Word-like pieces (Yivra TextSegmenter): split at whitespace, punctuation
   and between CJK and Latin runs; each CJK character is its own piece so a
   name inside a sentence can be covered alone. */
typedef enum { ZANT_SEG_SPACE, ZANT_SEG_PUNCT, ZANT_SEG_CJK, ZANT_SEG_WORD } ZantSegClass;

static ZantSegClass zant_seg_class(unichar c) {
    if (c == ' ' || c == '\t' || c == 0x3000 || c == 0xA0) return ZANT_SEG_SPACE;
    if ((c >= 0x3040 && c <= 0x30FF) || (c >= 0x3400 && c <= 0x9FFF) || (c >= 0xAC00 && c <= 0xD7AF) ||
        (c >= 0xF900 && c <= 0xFAFF))
        return ZANT_SEG_CJK;
    if ((c >= 0x3000 && c <= 0x303F) || (c >= 0xFF00 && c <= 0xFF0F) || (c >= 0xFF1A && c <= 0xFF20) ||
        c == ',' || c == ';' || c == '(' || c == ')' || c == '[' || c == ']' || c == '"' || c == '\'' ||
        c == 0x201C || c == 0x201D || c == 0x2018 || c == 0x2019)
        return ZANT_SEG_PUNCT;
    return ZANT_SEG_WORD;
}

static size_t zant_utf8_offset(NSString *string, NSUInteger utf16) {
    return [[string substringToIndex:utf16] lengthOfBytesUsingEncoding:NSUTF8StringEncoding];
}

ZantOcrResult *zant_ocr_recognize(const uint8_t *pixels, int width, int height, int stride) {
    if (!pixels || width <= 0 || height <= 0 || stride < width * 4) return NULL;
    @autoreleasepool {
        CGColorSpaceRef space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
        CGDataProviderRef provider = CGDataProviderCreateWithData(NULL, pixels, (size_t)stride * height, NULL);
        CGImageRef image = CGImageCreate(width, height, 8, 32, stride, space,
                                         kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big, provider, NULL,
                                         false, kCGRenderingIntentDefault);
        CGDataProviderRelease(provider);
        CGColorSpaceRelease(space);
        if (!image) return NULL;

        VNRecognizeTextRequest *request = [[VNRecognizeTextRequest alloc] init];
        request.recognitionLevel = VNRequestTextRecognitionLevelAccurate;
        /* correction turns e-mail addresses and keys into dictionary words */
        request.usesLanguageCorrection = NO;
        /* fixed languages: automatic detection costs seconds per call and
           mixes up CJK; Chinese models also read Latin text */
        request.recognitionLanguages = @[ @"zh-Hans", @"zh-Hant", @"en-US" ];
        VNImageRequestHandler *handler = [[VNImageRequestHandler alloc] initWithCGImage:image options:@{}];
        NSError *error = nil;
        const BOOL ok = [handler performRequests:@[ request ] error:&error];
        CGImageRelease(image);
        if (!ok) return NULL;

        ZantOcrBuilder b = {0};
        const CGFloat w = width, h = height;
        for (VNRecognizedTextObservation *observation in request.results) {
            VNRecognizedText *candidate = [[observation topCandidates:1] firstObject];
            if (!candidate) continue;
            NSString *line = candidate.string;
            if (line.length == 0) continue;
            if (b.line_count && !zant_grow((void **)&b.text, &b.text_cap, b.text_len + 1, 1)) break;
            if (b.line_count) b.text[b.text_len++] = '\n';
            const size_t line_start = b.text_len;
            const char *utf8 = line.UTF8String;
            const size_t utf8_len = strlen(utf8);
            if (!zant_grow((void **)&b.text, &b.text_cap, b.text_len + utf8_len + 1, 1)) break;
            memcpy(b.text + b.text_len, utf8, utf8_len);
            b.text_len += utf8_len;
            b.text[b.text_len] = 0;

            if (!zant_grow((void **)&b.lines, &b.line_cap, b.line_count + 1, sizeof(ZantOcrLine))) break;
            const CGRect box = observation.boundingBox; /* normalized, bottom-left origin */
            ZantOcrLine *out_line = &b.lines[b.line_count++];
            out_line->start = (uint32_t)line_start;
            out_line->len = (uint32_t)utf8_len;
            out_line->x = (float)(box.origin.x * w);
            out_line->y = (float)((1.0 - box.origin.y - box.size.height) * h);
            out_line->width = (float)(box.size.width * w);
            out_line->height = (float)(box.size.height * h);
            out_line->first_token = (uint32_t)b.token_count;
            out_line->token_count = 0;

            const NSUInteger length = line.length;
            NSUInteger i = 0;
            while (i < length) {
                const ZantSegClass cls = zant_seg_class([line characterAtIndex:i]);
                NSUInteger j = i + 1;
                if (cls == ZANT_SEG_WORD) {
                    while (j < length && zant_seg_class([line characterAtIndex:j]) == ZANT_SEG_WORD) j++;
                } else {
                    j = NSMaxRange([line rangeOfComposedCharacterSequenceAtIndex:i]);
                }
                if (cls == ZANT_SEG_WORD || cls == ZANT_SEG_CJK) {
                    VNRectangleObservation *piece = [candidate boundingBoxForRange:NSMakeRange(i, j - i) error:nil];
                    if (piece && zant_grow((void **)&b.tokens, &b.token_cap, b.token_count + 1, sizeof(ZantOcrToken))) {
                        const CGRect r = piece.boundingBox;
                        ZantOcrToken *token = &b.tokens[b.token_count++];
                        const size_t start = zant_utf8_offset(line, i);
                        token->start = (uint32_t)(line_start + start);
                        token->len = (uint32_t)(zant_utf8_offset(line, j) - start);
                        token->x = (float)(r.origin.x * w);
                        token->y = (float)((1.0 - r.origin.y - r.size.height) * h);
                        token->width = (float)(r.size.width * w);
                        token->height = (float)(r.size.height * h);
                        out_line->token_count++;
                    }
                }
                i = j;
            }
        }
        ZantOcrResult *result = calloc(1, sizeof(ZantOcrResult));
        if (!result) {
            free(b.text);
            free(b.lines);
            free(b.tokens);
            return NULL;
        }
        if (!b.text) b.text = calloc(1, 1);
        result->text = b.text;
        result->text_len = b.text_len;
        result->lines = b.lines;
        result->line_count = b.line_count;
        result->tokens = b.tokens;
        result->token_count = b.token_count;
        return result;
    }
}

void zant_ocr_free(ZantOcrResult *result) {
    if (!result) return;
    free(result->text);
    free(result->lines);
    free(result->tokens);
    free(result);
}
