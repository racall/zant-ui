#ifndef ZANT_PDF_H
#define ZANT_PDF_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
int zant_pdf_page_count(const char *path);
int zant_pdf_render_page(const char *path, int page, int width, int height,
                         char *out_path, size_t capacity);
int zant_pdf_page_text(const char *path, int page, char *out_text, size_t capacity);

/* ---- Document API (zant.pdf.Document, ui.pdfViewer) -------------------
 * Coordinates are PDF points (1/72 in) of the displayed page: origin at the
 * top-left of the crop box, y down, the page's /Rotate applied. All calls
 * are thread-safe (serialized per backend), so pages can be rendered on a
 * worker thread while the UI thread reads text geometry. */
typedef struct ZantPdfDocument ZantPdfDocument;

/* NULL when the file cannot be opened; *locked = 1 when it needs a
 * (different) password. password may be NULL. */
ZantPdfDocument *zant_pdf_open(const char *path, const char *password, int *locked);
void zant_pdf_close(ZantPdfDocument *doc);
int zant_pdf_count(ZantPdfDocument *doc);
/* Displayed page size in points. 0 on success. */
int zant_pdf_size(ZantPdfDocument *doc, int page, float *width, float *height);
/* Renders the page, scaled to page_width x page_height pixels, on white
 * with annotations; only the region (x, y, width, height) of that scaled
 * page is drawn (tiles), into premultiplied 32-bit pixels in the native
 * byte order (RGBA on macOS, BGRA on Windows). 0 on success. */
int zant_pdf_render(ZantPdfDocument *doc, int page, int page_width, int page_height, int x, int y, int width,
                    int height, void *pixels, int stride);

typedef struct ZantPdfGlyph {
    /* loose box (line height): selection highlight, hit testing */
    float x, y, width, height;
    /* tight box (ink); equal to the loose box when unknown */
    float tight_x, tight_y, tight_width, tight_height;
    uint32_t codepoint; /* Unicode scalar value (0: none / second UTF-16 half) */
    float font_size;    /* 0 when unknown */
    int run;            /* glyphs of one text object share it; -1 when unknown */
    int flags;          /* 1 = space, 2 = empty box (generated line break, ...) */
} ZantPdfGlyph;

/* Text glyphs in reading order. Returns the page's glyph count; fills up to
 * capacity entries (out may be NULL to ask for the count). -1 on error. */
int zant_pdf_glyphs(ZantPdfDocument *doc, int page, ZantPdfGlyph *out, int capacity);

typedef struct ZantPdfLink {
    float x, y, width, height;
    int dest_page;  /* internal link target, -1 for none */
    float dest_y;   /* target y in points from the top, -1 unknown */
    char uri[1024]; /* external link (UTF-8), "" for none */
} ZantPdfLink;

/* Link areas of a page; returns the count, fills up to capacity. */
int zant_pdf_links(ZantPdfDocument *doc, int page, ZantPdfLink *out, int capacity);

typedef struct ZantPdfOutlineItem {
    int depth; /* 0 = top level */
    int page;  /* -1 when the entry has no page */
    char title[256];
} ZantPdfOutlineItem;

/* Bookmarks, flattened depth-first; returns the count, fills up to capacity. */
int zant_pdf_outline(ZantPdfDocument *doc, ZantPdfOutlineItem *out, int capacity);

enum {
    ZANT_PDF_ANNOT_HIGHLIGHT = 0,
    ZANT_PDF_ANNOT_UNDERLINE = 1,
    ZANT_PDF_ANNOT_STRIKEOUT = 2,
    ZANT_PDF_ANNOT_SQUIGGLY = 3,
    ZANT_PDF_ANNOT_INK = 4,
    ZANT_PDF_ANNOT_NOTE = 5, /* sticky note (Text annotation) */
};

/* An annotation to write, in displayed-page points (like the glyphs). */
typedef struct ZantPdfAnnotation {
    int page;
    int kind;       /* ZANT_PDF_ANNOT_* */
    uint32_t color; /* 0xAARRGGBB; alpha is the opacity */
    float width;    /* ink stroke width in points */
    /* text markup: one (x, y, width, height) box per line; note: the icon
       box. 4 floats per rect. */
    const float *rects;
    int rect_count;
    /* ink: points (x, y pairs) of all strokes, split by stroke_lengths */
    const float *points;
    const int *stroke_lengths;
    int stroke_count;
    const char *contents; /* comment / note text (UTF-8), may be NULL */
} ZantPdfAnnotation;

/* Writes a copy of the PDF at source_path to out_path with the annotations
 * added as standard PDF annotations (Highlight, Underline, StrikeOut,
 * Squiggly, Ink, Text) that other viewers show and edit. 0 on success. */
int zant_pdf_save_annotated(const char *source_path, const char *password, const char *out_path,
                            const ZantPdfAnnotation *annotations, int count);
#ifdef __cplusplus
}
#endif
#endif
