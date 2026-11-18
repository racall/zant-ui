#import "window_internal.h"

#include <stdlib.h>
#include <string.h>

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

int zant_clipboard_clear(void) {
    @autoreleasepool {
        [[NSPasteboard generalPasteboard] clearContents];
        return 1;
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

uint8_t *zant_clipboard_get_image_png(size_t *out_len) {
    @autoreleasepool {
        if (out_len) *out_len = 0;
        NSPasteboard *pasteboard = [NSPasteboard generalPasteboard];
        NSData *png = [pasteboard dataForType:NSPasteboardTypePNG];
        if (!png) {
            NSData *tiff = [pasteboard dataForType:NSPasteboardTypeTIFF];
            NSBitmapImageRep *rep = tiff ? [NSBitmapImageRep imageRepWithData:tiff] : nil;
            png = rep ? [rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}] : nil;
        }
        if (!png || png.length == 0) return NULL;
        uint8_t *bytes = malloc(png.length);
        if (!bytes) return NULL;
        memcpy(bytes, png.bytes, png.length);
        if (out_len) *out_len = png.length;
        return bytes;
    }
}

char *zant_clipboard_get_files(size_t *out_len) {
    @autoreleasepool {
        if (out_len) *out_len = 0;
        NSArray<NSURL *> *urls =
            [[NSPasteboard generalPasteboard] readObjectsForClasses:@[ NSURL.class ]
                                                            options:@{NSPasteboardURLReadingFileURLsOnlyKey : @YES}];
        NSMutableString *joined = [NSMutableString string];
        for (NSURL *url in urls) {
            NSString *path = url.path;
            if (path.length == 0) continue;
            if (joined.length) [joined appendString:@"\n"];
            [joined appendString:path];
        }
        const char *utf8 = joined.UTF8String;
        const size_t len = utf8 ? strlen(utf8) : 0;
        if (len == 0) return NULL;
        char *out = malloc(len + 1);
        if (!out) return NULL;
        memcpy(out, utf8, len + 1);
        if (out_len) *out_len = len;
        return out;
    }
}

int zant_clipboard_set_files(const char *paths, size_t len) {
    @autoreleasepool {
        if (!paths || len == 0) return 0;
        NSString *all = [[NSString alloc] initWithBytes:paths length:len encoding:NSUTF8StringEncoding];
        NSMutableArray<NSURL *> *urls = [NSMutableArray array];
        for (NSString *line in [all componentsSeparatedByString:@"\n"]) {
            if (line.length) [urls addObject:[NSURL fileURLWithPath:line]];
        }
        if (urls.count == 0) return 0;
        NSPasteboard *pasteboard = [NSPasteboard generalPasteboard];
        [pasteboard clearContents];
        return [pasteboard writeObjects:urls] ? 1 : 0;
    }
}

int zant_clipboard_set_image_png(const uint8_t *png, size_t len) {
    @autoreleasepool {
        if (!png || len == 0) return 0;
        NSData *data = [NSData dataWithBytes:png length:len];
        NSBitmapImageRep *rep = [NSBitmapImageRep imageRepWithData:data];
        NSPasteboard *pasteboard = [NSPasteboard generalPasteboard];
        [pasteboard clearContents];
        BOOL ok = [pasteboard setData:data forType:NSPasteboardTypePNG];
        /* many apps only read TIFF */
        NSData *tiff = rep ? [rep TIFFRepresentation] : nil;
        if (tiff) [pasteboard setData:tiff forType:NSPasteboardTypeTIFF];
        return ok ? 1 : 0;
    }
}

void zant_clipboard_free(void *bytes) {
    free(bytes);
}

uint64_t zant_clipboard_change_count(void) {
    return (uint64_t)[NSPasteboard generalPasteboard].changeCount;
}

int zant_clipboard_privacy(void) {
    @autoreleasepool {
        /* http://nspasteboard.org: the markers password managers and other
           apps put next to content clipboard managers should not keep */
        NSArray<NSPasteboardType> *types = [NSPasteboard generalPasteboard].types;
        for (NSString *marker in @[ @"org.nspasteboard.ConcealedType", @"org.nspasteboard.TransientType",
                                    @"org.nspasteboard.AutoGeneratedType" ]) {
            if ([types containsObject:marker]) return ZANT_CLIPBOARD_NO_HISTORY | ZANT_CLIPBOARD_NO_SYNC;
        }
        return 0;
    }
}

static NSTimer *g_clipboard_timer = nil;
static NSInteger g_clipboard_count = 0;

void zant_clipboard_watch(int enable) {
    [g_clipboard_timer invalidate];
    g_clipboard_timer = nil;
    if (!enable) return;
    /* macOS has no clipboard notification; the change count is a cheap read */
    g_clipboard_count = [NSPasteboard generalPasteboard].changeCount;
    g_clipboard_timer = [NSTimer timerWithTimeInterval:0.3
                                               repeats:YES
                                                 block:^(NSTimer *timer) {
        (void)timer;
        const NSInteger count = [NSPasteboard generalPasteboard].changeCount;
        if (count == g_clipboard_count) return;
        g_clipboard_count = count;
        zant_app_emit(ZANT_APP_EVENT_CLIPBOARD, 0);
    }];
    g_clipboard_timer.tolerance = 0.1;
    [[NSRunLoop mainRunLoop] addTimer:g_clipboard_timer forMode:NSRunLoopCommonModes];
}
