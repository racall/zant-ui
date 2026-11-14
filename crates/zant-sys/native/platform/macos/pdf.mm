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

/* ---- Document API --------------------------------------------------- */

struct ZantPdfDocument {
    PDFDocument *doc;
    NSLock *lock;
};

static PDFPage *zant_pdf_page_at(ZantPdfDocument *d, int page) {
    if (!d || !d->doc || page < 0 || page >= (int)d->doc.pageCount) return nil;
    return [d->doc pageAtIndex:(NSUInteger)page];
}

/* Displayed size (crop box, rotation applied) in points. */
static CGSize zant_pdf_display_size(PDFPage *page) {
    const NSRect crop = [page boundsForBox:kPDFDisplayBoxCropBox];
    const int rot = ((int)page.rotation % 360 + 360) % 360;
    if (rot == 90 || rot == 270) return CGSizeMake(crop.size.height, crop.size.width);
    return crop.size;
}

/* Page space (PDF user space, y up) -> displayed page (top-left, y down). */
static CGRect zant_pdf_to_display(PDFPage *page, CGAffineTransform t, CGFloat display_h, NSRect r) {
    const CGRect box = CGRectApplyAffineTransform(NSRectToCGRect(r), t);
    return CGRectMake(box.origin.x, display_h - box.origin.y - box.size.height, box.size.width, box.size.height);
}

ZantPdfDocument *zant_pdf_open(const char *path, const char *password, int *locked) {
    @autoreleasepool {
        if (locked) *locked = 0;
        NSString *source = path ? [NSString stringWithUTF8String:path] : nil;
        if (source.length == 0) return NULL;
        PDFDocument *document = [[PDFDocument alloc] initWithURL:[NSURL fileURLWithPath:source]];
        if (!document) return NULL;
        if (document.isLocked) {
            NSString *pw = password ? [NSString stringWithUTF8String:password] : @"";
            if (![document unlockWithPassword:pw]) {
                if (locked) *locked = 1;
                return NULL;
            }
        }
        ZantPdfDocument *d = new ZantPdfDocument();
        d->doc = document;
        d->lock = [[NSLock alloc] init];
        return d;
    }
}

void zant_pdf_close(ZantPdfDocument *doc) {
    if (!doc) return;
    @autoreleasepool {
        [doc->lock lock];
        doc->doc = nil;
        [doc->lock unlock];
        doc->lock = nil;
    }
    delete doc;
}

int zant_pdf_count(ZantPdfDocument *doc) {
    if (!doc) return 0;
    [doc->lock lock];
    const int n = doc->doc ? (int)doc->doc.pageCount : 0;
    [doc->lock unlock];
    return n;
}

int zant_pdf_size(ZantPdfDocument *doc, int page, float *width, float *height) {
    if (!doc) return -1;
    @autoreleasepool {
        [doc->lock lock];
        PDFPage *p = zant_pdf_page_at(doc, page);
        CGSize size = p ? zant_pdf_display_size(p) : CGSizeZero;
        [doc->lock unlock];
        if (!p) return -1;
        if (width) *width = (float)size.width;
        if (height) *height = (float)size.height;
        return 0;
    }
}

int zant_pdf_render(ZantPdfDocument *doc, int page, int page_width, int page_height, int x, int y, int width,
                    int height, void *pixels, int stride) {
    if (!doc || !pixels || width <= 0 || height <= 0 || page_width <= 0 || page_height <= 0 || stride < width * 4)
        return -1;
    @autoreleasepool {
        CGColorSpaceRef space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
        CGContextRef ctx = CGBitmapContextCreate(pixels, (size_t)width, (size_t)height, 8, (size_t)stride, space,
                                                 kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big);
        CGColorSpaceRelease(space);
        if (!ctx) return -1;
        CGContextSetRGBFillColor(ctx, 1, 1, 1, 1);
        CGContextFillRect(ctx, CGRectMake(0, 0, width, height));
        CGContextSetInterpolationQuality(ctx, kCGInterpolationHigh);
        CGContextSetShouldSmoothFonts(ctx, false);
        int result = -1;
        [doc->lock lock];
        PDFPage *p = zant_pdf_page_at(doc, page);
        if (p) {
            const CGSize size = zant_pdf_display_size(p);
            if (size.width > 0 && size.height > 0) {
                /* CG is y up: the tile's top edge is page_height - y */
                CGContextTranslateCTM(ctx, -(CGFloat)x, -(CGFloat)(page_height - y - height));
                CGContextScaleCTM(ctx, (CGFloat)page_width / size.width, (CGFloat)page_height / size.height);
                /* drawWithBox applies the crop box origin and the rotation */
                [p drawWithBox:kPDFDisplayBoxCropBox toContext:ctx];
                result = 0;
            }
        }
        [doc->lock unlock];
        CGContextRelease(ctx);
        return result;
    }
}

int zant_pdf_glyphs(ZantPdfDocument *doc, int page, ZantPdfGlyph *out, int capacity) {
    if (!doc) return -1;
    @autoreleasepool {
        [doc->lock lock];
        PDFPage *p = zant_pdf_page_at(doc, page);
        if (!p) {
            [doc->lock unlock];
            return -1;
        }
        NSString *text = p.string ?: @"";
        const NSInteger count = (NSInteger)text.length;
        const CGAffineTransform t = [p transformForBox:kPDFDisplayBoxCropBox];
        const CGFloat display_h = zant_pdf_display_size(p).height;
        for (NSInteger i = 0; out && i < count && i < capacity; i++) {
            ZantPdfGlyph g;
            memset(&g, 0, sizeof(g));
            g.run = -1;
            const unichar u = [text characterAtIndex:(NSUInteger)i];
            uint32_t cp = u;
            if (CFStringIsSurrogateHighCharacter(u) && i + 1 < count) {
                const unichar low = [text characterAtIndex:(NSUInteger)(i + 1)];
                if (CFStringIsSurrogateLowCharacter(low)) cp = CFStringGetLongCharacterForSurrogatePair(u, low);
            } else if (CFStringIsSurrogateLowCharacter(u)) {
                cp = 0; /* second half: the first half carries the scalar */
            }
            g.codepoint = cp;
            /* characterBoundsAtIndex: uses PDFKit's internal glyph order,
               which drifts from `string` indices on many documents; a
               one-character selection is indexed like `string` (~1.5 µs). */
            NSRect r = NSZeroRect;
            @autoreleasepool {
                PDFSelection *sel = [p selectionForRange:NSMakeRange((NSUInteger)i, 1)];
                if (sel) r = [sel boundsForPage:p];
            }
            if (cp == 0 || cp == '\n' || cp == '\r' || r.size.width <= 0 || r.size.height <= 0) {
                g.flags = 2;
            } else {
                const CGRect box = zant_pdf_to_display(p, t, display_h, r);
                g.x = g.tight_x = (float)box.origin.x;
                g.y = g.tight_y = (float)box.origin.y;
                g.width = g.tight_width = (float)box.size.width;
                g.height = g.tight_height = (float)box.size.height;
                if (cp == ' ') g.flags = 1;
            }
            out[i] = g;
        }
        [doc->lock unlock];
        return (int)count;
    }
}

static void zant_pdf_copy_utf8(char *dst, size_t capacity, NSString *value) {
    if (capacity == 0) return;
    dst[0] = 0;
    if (!value) return;
    const char *utf8 = value.UTF8String;
    if (!utf8) return;
    size_t n = strlen(utf8);
    if (n + 1 > capacity) {
        n = capacity - 1;
        /* do not cut a UTF-8 sequence in half */
        while (n > 0 && ((unsigned char)utf8[n] & 0xC0) == 0x80) n--;
    }
    memcpy(dst, utf8, n);
    dst[n] = 0;
}

int zant_pdf_links(ZantPdfDocument *doc, int page, ZantPdfLink *out, int capacity) {
    if (!doc) return -1;
    @autoreleasepool {
        [doc->lock lock];
        PDFPage *p = zant_pdf_page_at(doc, page);
        if (!p) {
            [doc->lock unlock];
            return -1;
        }
        const CGAffineTransform t = [p transformForBox:kPDFDisplayBoxCropBox];
        const CGFloat display_h = zant_pdf_display_size(p).height;
        int n = 0;
        for (PDFAnnotation *a in p.annotations) {
            if (![a.type isEqualToString:@"Link"]) continue;
            NSURL *url = a.URL;
            PDFDestination *dest = a.destination;
            PDFAction *action = a.action;
            if (!url && [action isKindOfClass:[PDFActionURL class]]) url = ((PDFActionURL *)action).URL;
            if (!dest && [action isKindOfClass:[PDFActionGoTo class]]) dest = ((PDFActionGoTo *)action).destination;
            if (!url && !dest) continue;
            if (out && n < capacity) {
                ZantPdfLink *l = &out[n];
                memset(l, 0, sizeof(*l));
                const CGRect box = zant_pdf_to_display(p, t, display_h, a.bounds);
                l->x = (float)box.origin.x;
                l->y = (float)box.origin.y;
                l->width = (float)box.size.width;
                l->height = (float)box.size.height;
                l->dest_page = -1;
                l->dest_y = -1;
                if (dest && dest.page) {
                    l->dest_page = (int)[doc->doc indexForPage:dest.page];
                    const NSPoint pt = dest.point;
                    if (pt.y < 1e6 && pt.y > -1e6) {
                        PDFPage *target = dest.page;
                        const CGAffineTransform tt = [target transformForBox:kPDFDisplayBoxCropBox];
                        const CGPoint q = CGPointApplyAffineTransform(NSPointToCGPoint(pt), tt);
                        l->dest_y = (float)MAX(0, zant_pdf_display_size(target).height - q.y);
                    }
                }
                if (url) zant_pdf_copy_utf8(l->uri, sizeof(l->uri), url.absoluteString);
            }
            n++;
        }
        [doc->lock unlock];
        return n;
    }
}

static void zant_pdf_walk_outline(PDFDocument *document, PDFOutline *node, int depth, ZantPdfOutlineItem *out,
                                  int capacity, int *n) {
    for (NSUInteger i = 0; i < node.numberOfChildren; i++) {
        PDFOutline *child = [node childAtIndex:i];
        if (out && *n < capacity) {
            ZantPdfOutlineItem *item = &out[*n];
            memset(item, 0, sizeof(*item));
            item->depth = depth;
            PDFDestination *dest = child.destination;
            if (!dest && [child.action isKindOfClass:[PDFActionGoTo class]]) dest = ((PDFActionGoTo *)child.action).destination;
            item->page = (dest && dest.page) ? (int)[document indexForPage:dest.page] : -1;
            zant_pdf_copy_utf8(item->title, sizeof(item->title), child.label);
        }
        (*n)++;
        if (depth < 32) zant_pdf_walk_outline(document, child, depth + 1, out, capacity, n);
    }
}

int zant_pdf_outline(ZantPdfDocument *doc, ZantPdfOutlineItem *out, int capacity) {
    if (!doc) return -1;
    @autoreleasepool {
        [doc->lock lock];
        int n = 0;
        PDFOutline *root = doc->doc.outlineRoot;
        if (root) zant_pdf_walk_outline(doc->doc, root, 0, out, capacity, &n);
        [doc->lock unlock];
        return n;
    }
}

/* ---- Annotated export ------------------------------------------------ */

/* Displayed-page box (top-left origin) -> page space (PDF user space). */
static NSRect zant_pdf_from_display(PDFPage *page, float x, float y, float w, float h) {
    const CGAffineTransform t = [page transformForBox:kPDFDisplayBoxCropBox];
    const CGFloat display_h = zant_pdf_display_size(page).height;
    const CGRect box = CGRectMake(x, display_h - y - h, w, h);
    return NSRectFromCGRect(CGRectApplyAffineTransform(box, CGAffineTransformInvert(t)));
}

static NSPoint zant_pdf_point_from_display(PDFPage *page, float x, float y) {
    const CGAffineTransform t = [page transformForBox:kPDFDisplayBoxCropBox];
    const CGFloat display_h = zant_pdf_display_size(page).height;
    return NSPointFromCGPoint(CGPointApplyAffineTransform(CGPointMake(x, display_h - y), CGAffineTransformInvert(t)));
}

static NSColor *zant_pdf_color(uint32_t argb) {
    return [NSColor colorWithSRGBRed:((argb >> 16) & 0xff) / 255.0
                               green:((argb >> 8) & 0xff) / 255.0
                                blue:(argb & 0xff) / 255.0
                               alpha:((argb >> 24) & 0xff) / 255.0];
}

int zant_pdf_save_annotated(const char *source_path, const char *password, const char *out_path,
                            const ZantPdfAnnotation *annotations, int count) {
    @autoreleasepool {
        if (!source_path || !out_path) return -1;
        /* a separate copy: the displayed document stays unchanged */
        PDFDocument *doc = [[PDFDocument alloc] initWithURL:[NSURL fileURLWithPath:[NSString stringWithUTF8String:source_path]]];
        if (!doc) return -1;
        if (doc.isLocked && ![doc unlockWithPassword:password ? [NSString stringWithUTF8String:password] : @""]) return -1;
        for (int i = 0; i < count; i++) {
            const ZantPdfAnnotation *a = &annotations[i];
            if (a->page < 0 || a->page >= (int)doc.pageCount) continue;
            PDFPage *page = [doc pageAtIndex:(NSUInteger)a->page];
            PDFAnnotation *ann = nil;
            NSString *contents = a->contents ? [NSString stringWithUTF8String:a->contents] : nil;
            if (a->kind <= ZANT_PDF_ANNOT_SQUIGGLY) {
                if (a->rect_count <= 0) continue;
                NSRect bounds = NSZeroRect;
                NSRect boxes[a->rect_count];
                for (int r = 0; r < a->rect_count; r++) {
                    const float *v = &a->rects[r * 4];
                    boxes[r] = zant_pdf_from_display(page, v[0], v[1], v[2], v[3]);
                    bounds = r == 0 ? boxes[r] : NSUnionRect(bounds, boxes[r]);
                }
                PDFAnnotationSubtype type = a->kind == ZANT_PDF_ANNOT_HIGHLIGHT   ? PDFAnnotationSubtypeHighlight
                                            : a->kind == ZANT_PDF_ANNOT_UNDERLINE ? PDFAnnotationSubtypeUnderline
                                            : a->kind == ZANT_PDF_ANNOT_STRIKEOUT ? PDFAnnotationSubtypeStrikeOut
                                                                                  : (PDFAnnotationSubtype)@"/Squiggly"; /* no PDFKit constant */
                ann = [[PDFAnnotation alloc] initWithBounds:bounds forType:type withProperties:nil];
                /* quad points relative to the bounds: TL, TR, BL, BR per line */
                NSMutableArray<NSValue *> *quads = [NSMutableArray array];
                for (int r = 0; r < a->rect_count; r++) {
                    const NSRect b = boxes[r];
                    const CGFloat ox = bounds.origin.x, oy = bounds.origin.y;
                    [quads addObject:[NSValue valueWithPoint:NSMakePoint(NSMinX(b) - ox, NSMaxY(b) - oy)]];
                    [quads addObject:[NSValue valueWithPoint:NSMakePoint(NSMaxX(b) - ox, NSMaxY(b) - oy)]];
                    [quads addObject:[NSValue valueWithPoint:NSMakePoint(NSMinX(b) - ox, NSMinY(b) - oy)]];
                    [quads addObject:[NSValue valueWithPoint:NSMakePoint(NSMaxX(b) - ox, NSMinY(b) - oy)]];
                }
                ann.quadrilateralPoints = quads;
            } else if (a->kind == ZANT_PDF_ANNOT_INK) {
                if (a->stroke_count <= 0) continue;
                NSMutableArray<NSBezierPath *> *paths = [NSMutableArray array];
                NSRect bounds = NSZeroRect;
                bool first = true;
                int offset = 0;
                for (int s = 0; s < a->stroke_count; s++) {
                    const int n = a->stroke_lengths[s];
                    NSBezierPath *path = [NSBezierPath bezierPath];
                    for (int k = 0; k < n; k++) {
                        const NSPoint p = zant_pdf_point_from_display(page, a->points[(offset + k) * 2], a->points[(offset + k) * 2 + 1]);
                        if (k == 0) [path moveToPoint:p]; else [path lineToPoint:p];
                        const NSRect dot = NSMakeRect(p.x, p.y, 0, 0);
                        bounds = first ? dot : NSUnionRect(bounds, dot);
                        first = false;
                    }
                    offset += n;
                    [paths addObject:path];
                }
                const CGFloat pad = MAX(1.0, a->width);
                bounds = NSInsetRect(bounds, -pad, -pad);
                ann = [[PDFAnnotation alloc] initWithBounds:bounds forType:PDFAnnotationSubtypeInk withProperties:nil];
                for (NSBezierPath *path in paths) {
                    /* ink paths are relative to the annotation's origin */
                    NSAffineTransform *shift = [NSAffineTransform transform];
                    [shift translateXBy:-bounds.origin.x yBy:-bounds.origin.y];
                    [path transformUsingAffineTransform:shift];
                    [ann addBezierPath:path];
                }
                PDFBorder *border = [[PDFBorder alloc] init];
                border.lineWidth = a->width > 0 ? a->width : 2;
                ann.border = border;
            } else if (a->kind == ZANT_PDF_ANNOT_NOTE) {
                if (a->rect_count <= 0) continue;
                const float *v = a->rects;
                ann = [[PDFAnnotation alloc] initWithBounds:zant_pdf_from_display(page, v[0], v[1], v[2], v[3])
                                                    forType:PDFAnnotationSubtypeText
                                             withProperties:nil];
                ann.iconType = kPDFTextAnnotationIconNote;
            }
            if (!ann) continue;
            ann.color = zant_pdf_color(a->color);
            if (contents.length > 0) ann.contents = contents;
            ann.modificationDate = [NSDate date];
            [page addAnnotation:ann];
        }
        return [doc writeToFile:[NSString stringWithUTF8String:out_path]] ? 0 : -1;
    }
}
