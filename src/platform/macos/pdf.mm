#import <Foundation/Foundation.h>
#import <AppKit/AppKit.h>
#import <PDFKit/PDFKit.h>
#include "../pdf.h"

static NSString *cached_pdf_page(const char *path, int page, int width, int height) {
    NSString *source = [NSString stringWithUTF8String:path];
    if (source.length == 0 || width <= 0 || height <= 0) return nil;
    PDFDocument *document = [[PDFDocument alloc] initWithURL:[NSURL fileURLWithPath:source]];
    if (!document || document.pageCount == 0) return nil;
    if (page < 0) page = 0;
    if (page >= (int)document.pageCount) page = (int)document.pageCount - 1;
    PDFPage *pdf_page = [document pageAtIndex:(NSUInteger)page];
    if (!pdf_page) return nil;

    NSUInteger hash = source.hash ^ ((NSUInteger)page * 1315423911u) ^ ((NSUInteger)width << 8) ^ (NSUInteger)height;
    NSString *cache_dir = NSTemporaryDirectory();
    NSString *filename = [NSString stringWithFormat:@"zant-pdf-%lx.png", (unsigned long)hash];
    NSString *output = [cache_dir stringByAppendingPathComponent:filename];
    if ([[NSFileManager defaultManager] fileExistsAtPath:output]) return output;

    NSImage *thumbnail = [pdf_page thumbnailOfSize:NSMakeSize(width, height) forBox:kPDFDisplayBoxMediaBox];
    if (!thumbnail) return nil;
    NSImage *canvas = [[NSImage alloc] initWithSize:NSMakeSize(width, height)];
    [canvas lockFocus];
    [[NSColor whiteColor] setFill];
    NSRectFill(NSMakeRect(0, 0, width, height));
    [thumbnail drawInRect:NSMakeRect(0, 0, width, height) fromRect:NSZeroRect
                 operation:NSCompositingOperationSourceOver fraction:1.0];
    [canvas unlockFocus];
    NSBitmapImageRep *rep = [NSBitmapImageRep imageRepWithData:[canvas TIFFRepresentation]];
    NSData *png = [rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
    if (!png || ![png writeToFile:output atomically:YES]) return nil;
    return output;
}

int zant_pdf_page_count(const char *path) {
    @autoreleasepool {
        NSString *source = [NSString stringWithUTF8String:path];
        if (source.length == 0) return 0;
        PDFDocument *document = [[PDFDocument alloc] initWithURL:[NSURL fileURLWithPath:source]];
        return document ? (int)document.pageCount : 0;
    }
}

int zant_pdf_page_text(const char *path, int page, char *out_text, size_t capacity) {
    @autoreleasepool {
        NSString *source = [NSString stringWithUTF8String:path];
        if (source.length == 0 || capacity == 0) return -1;
        PDFDocument *document = [[PDFDocument alloc] initWithURL:[NSURL fileURLWithPath:source]];
        if (!document || document.pageCount == 0) return -1;
        if (page < 0) page = 0;
        if (page >= (int)document.pageCount) page = (int)document.pageCount - 1;
        PDFPage *pdf_page = [document pageAtIndex:(NSUInteger)page];
        NSString *text = pdf_page.string;
        if (!text) return -1;
        const char *utf8 = text.UTF8String;
        size_t length = strlen(utf8);
        if (length + 1 > capacity) length = capacity - 1;
        memcpy(out_text, utf8, length);
        out_text[length] = 0;
        return 0;
    }
}

int zant_pdf_render_page(const char *path, int page, int width, int height,
                         char *out_path, size_t capacity) {
    @autoreleasepool {
        NSString *output = cached_pdf_page(path, page, width, height);
        if (!output || capacity == 0) return -1;
        const char *utf8 = output.fileSystemRepresentation;
        size_t length = strlen(utf8);
        if (length + 1 > capacity) return -1;
        memcpy(out_path, utf8, length + 1);
        return 0;
    }
}
