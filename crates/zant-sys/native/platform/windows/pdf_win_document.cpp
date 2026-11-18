#include "pdf_win_internal.h"

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <cwchar>
#include <vector>

using namespace zant_pdf_win;

extern "C" {

/* ---- Document API --------------------------------------------------- */

struct ZantPdfDocument {
    std::vector<char> bytes; /* PDFium reads from it while the doc is open */
    void *doc = nullptr;
};

namespace {

/* Device-space oversampling for FPDF_PageToDevice (it returns ints). */
constexpr int kSub = 16;

struct PageScope {
    void *page = nullptr;
    void *text = nullptr;
    float w = 0, h = 0;
    PageScope(ZantPdfDocument *d, int index, bool want_text) {
        if (!d || !d->doc || index < 0 || index >= g_pdfium.page_count(d->doc)) return;
        page = g_pdfium.load_page(d->doc, index);
        if (!page) return;
        w = g_pdfium.page_width_f(page);
        h = g_pdfium.page_height_f(page);
        if (want_text) text = g_pdfium.text_load_page(page);
    }
    ~PageScope() {
        if (text) g_pdfium.text_close_page(text);
        if (page) g_pdfium.close_page(page);
    }
    /* page space (y up) -> displayed page (top-left, y down), in points */
    void map(double px, double py, float *x, float *y) const {
        int dx = 0, dy = 0;
        g_pdfium.page_to_device(page, 0, 0, std::max(1, (int)std::lround(w * kSub)),
                                std::max(1, (int)std::lround(h * kSub)), 0, px, py, &dx, &dy);
        *x = (float)dx / kSub;
        *y = (float)dy / kSub;
    }
    void mapRect(double l, double t, double r, double b, float *x, float *y, float *rw, float *rh) const {
        float x1, y1, x2, y2;
        map(l, t, &x1, &y1);
        map(r, b, &x2, &y2);
        *x = std::min(x1, x2);
        *y = std::min(y1, y2);
        *rw = std::fabs(x2 - x1);
        *rh = std::fabs(y2 - y1);
    }
};

void copy_utf8(char *dst, size_t capacity, const std::string &value) {
    if (capacity == 0) return;
    size_t n = std::min(value.size(), capacity - 1);
    while (n > 0 && n < value.size() && (static_cast<unsigned char>(value[n]) & 0xC0) == 0x80) n--;
    memcpy(dst, value.data(), n);
    dst[n] = 0;
}

/* target page / y of a destination (y in points from the top, -1 unknown) */
void dest_target(ZantPdfDocument *d, void *dest, int *page, float *y) {
    *page = -1;
    *y = -1;
    if (!dest) return;
    *page = g_pdfium.dest_get_page_index(d->doc, dest);
    int has_x = 0, has_y = 0, has_zoom = 0;
    float x = 0, py = 0, zoom = 0;
    if (*page >= 0 && g_pdfium.dest_get_location(dest, &has_x, &has_y, &has_zoom, &x, &py, &zoom) && has_y) {
        PageScope target(d, *page, false);
        if (target.page) {
            float tx = 0, ty = 0;
            target.map(x, py, &tx, &ty);
            *y = std::max(0.0f, ty);
        }
    }
}

}  // namespace

ZantPdfDocument *zant_pdf_open(const char *path, const char *password, int *locked) {
    std::lock_guard<std::recursive_mutex> guard(g_lock);
    if (locked) *locked = 0;
    if (!path || !path[0] || !pdfium() || !g_pdfium.doc_api) return nullptr;
    FILE *file = _wfopen(widen(path).c_str(), L"rb");
    if (!file) return nullptr;
    auto *d = new ZantPdfDocument();
    char chunk[64 * 1024];
    size_t n;
    while ((n = fread(chunk, 1, sizeof(chunk), file)) > 0) d->bytes.insert(d->bytes.end(), chunk, chunk + n);
    fclose(file);
    d->doc = g_pdfium.load_mem_document64(d->bytes.data(), d->bytes.size(), password);
    if (!d->doc) {
        constexpr unsigned long kErrPassword = 4;
        if (locked && g_pdfium.get_last_error() == kErrPassword) *locked = 1;
        delete d;
        return nullptr;
    }
    return d;
}

void zant_pdf_close(ZantPdfDocument *doc) {
    if (!doc) return;
    std::lock_guard<std::recursive_mutex> guard(g_lock);
    if (doc->doc) g_pdfium.close_document(doc->doc);
    delete doc;
}

int zant_pdf_count(ZantPdfDocument *doc) {
    std::lock_guard<std::recursive_mutex> guard(g_lock);
    if (!doc || !doc->doc) return 0;
    return std::max(0, g_pdfium.page_count(doc->doc));
}

int zant_pdf_size(ZantPdfDocument *doc, int page, float *width, float *height) {
    std::lock_guard<std::recursive_mutex> guard(g_lock);
    PageScope p(doc, page, false);
    if (!p.page) return -1;
    if (width) *width = p.w;
    if (height) *height = p.h;
    return 0;
}

int zant_pdf_render(ZantPdfDocument *doc, int page, int page_width, int page_height, int x, int y, int width,
                    int height, void *pixels, int stride) {
    if (!pixels || width <= 0 || height <= 0 || page_width <= 0 || page_height <= 0 || stride < width * 4)
        return -1;
    std::lock_guard<std::recursive_mutex> guard(g_lock);
    PageScope p(doc, page, false);
    if (!p.page) return -1;
    constexpr int kBGRA = 4;
    Bitmap bitmap = g_pdfium.bitmap_create_ex(width, height, kBGRA, pixels, stride);
    if (!bitmap) return -1;
    g_pdfium.bitmap_fill_rect(bitmap, 0, 0, width, height, 0xFFFFFFFFul);
    constexpr int kAnnot = 0x01;
    /* the scaled page starts at (-x, -y) of the tile */
    g_pdfium.render_page_bitmap(bitmap, p.page, -x, -y, page_width, page_height, 0, kAnnot);
    g_pdfium.bitmap_destroy(bitmap);
    return 0;
}

int zant_pdf_glyphs(ZantPdfDocument *doc, int page, ZantPdfGlyph *out, int capacity) {
    std::lock_guard<std::recursive_mutex> guard(g_lock);
    PageScope p(doc, page, true);
    if (!p.page || !p.text) return -1;
    const int count = std::max(0, g_pdfium.text_count_chars(p.text));
    void *prev_obj = nullptr;
    int run = -1;
    for (int i = 0; out && i < count && i < capacity; i++) {
        ZantPdfGlyph g;
        memset(&g, 0, sizeof(g));
        g.run = -1;
        g.codepoint = g_pdfium.text_get_unicode(p.text, i);
        float r[4] = {0, 0, 0, 0}; /* left, top, right, bottom */
        if (!g_pdfium.text_get_loose_char_box(p.text, i, r) || r[0] == r[2] || r[1] == r[3]) {
            g.flags = 2;
        } else {
            p.mapRect(r[0], r[1], r[2], r[3], &g.x, &g.y, &g.width, &g.height);
            double l = 0, rr = 0, b = 0, t = 0;
            if (g_pdfium.text_get_char_box(p.text, i, &l, &rr, &b, &t) && l != rr && b != t) {
                p.mapRect(l, t, rr, b, &g.tight_x, &g.tight_y, &g.tight_width, &g.tight_height);
            } else {
                g.tight_x = g.x;
                g.tight_y = g.y;
                g.tight_width = g.width;
                g.tight_height = g.height;
            }
            if (g.codepoint == ' ') g.flags = 1;
        }
        g.font_size = (float)g_pdfium.text_get_font_size(p.text, i);
        if (g_pdfium.text_get_text_object) {
            void *obj = g_pdfium.text_get_text_object(p.text, i);
            if (i == 0 || obj != prev_obj) run++;
            prev_obj = obj;
            g.run = run;
        }
        out[i] = g;
    }
    return count;
}

int zant_pdf_links(ZantPdfDocument *doc, int page, ZantPdfLink *out, int capacity) {
    std::lock_guard<std::recursive_mutex> guard(g_lock);
    PageScope p(doc, page, false);
    if (!p.page) return -1;
    int pos = 0, n = 0;
    void *link = nullptr;
    while (g_pdfium.link_enumerate(p.page, &pos, &link)) {
        if (!link) continue;
        void *dest = g_pdfium.link_get_dest(doc->doc, link);
        void *action = g_pdfium.link_get_action(link);
        std::string uri;
        if (action) {
            constexpr unsigned long kGoTo = 1, kUri = 3;
            const unsigned long type = g_pdfium.action_get_type(action);
            if (type == kUri) {
                const unsigned long size = g_pdfium.action_get_uri_path(doc->doc, action, nullptr, 0);
                if (size > 1) {
                    std::string buf(size, '\0');
                    g_pdfium.action_get_uri_path(doc->doc, action, buf.data(), size);
                    uri.assign(buf.c_str());
                }
            } else if (type == kGoTo && !dest) {
                dest = g_pdfium.action_get_dest(doc->doc, action);
            }
        }
        if (!dest && uri.empty()) continue;
        if (out && n < capacity) {
            ZantPdfLink *l = &out[n];
            memset(l, 0, sizeof(*l));
            float r[4] = {0, 0, 0, 0};
            if (g_pdfium.link_get_annot_rect(link, r)) {
                /* FS_RECTF: left, top, right, bottom (page space) */
                p.mapRect(r[0], r[1], r[2], r[3], &l->x, &l->y, &l->width, &l->height);
            }
            dest_target(doc, dest, &l->dest_page, &l->dest_y);
            copy_utf8(l->uri, sizeof(l->uri), uri);
        }
        n++;
    }
    return n;
}

static void walk_bookmarks(ZantPdfDocument *d, void *parent, int depth, ZantPdfOutlineItem *out, int capacity,
                           int *n) {
    void *bm = g_pdfium.bookmark_first_child(d->doc, parent);
    while (bm && *n < 10000) {
        if (out && *n < capacity) {
            ZantPdfOutlineItem *item = &out[*n];
            memset(item, 0, sizeof(*item));
            item->depth = depth;
            const unsigned long size = g_pdfium.bookmark_get_title(bm, nullptr, 0);
            if (size > 2) {
                std::vector<wchar_t> title(size / 2 + 1, 0);
                g_pdfium.bookmark_get_title(bm, title.data(), size);
                copy_utf8(item->title, sizeof(item->title), narrow(title.data(), (int)wcslen(title.data())));
            }
            void *dest = g_pdfium.bookmark_get_dest(d->doc, bm);
            if (!dest) {
                void *action = g_pdfium.bookmark_get_action(bm);
                if (action) dest = g_pdfium.action_get_dest(d->doc, action);
            }
            item->page = dest ? g_pdfium.dest_get_page_index(d->doc, dest) : -1;
        }
        (*n)++;
        if (depth < 32) walk_bookmarks(d, bm, depth + 1, out, capacity, n);
        bm = g_pdfium.bookmark_next_sibling(d->doc, bm);
    }
}

int zant_pdf_outline(ZantPdfDocument *doc, ZantPdfOutlineItem *out, int capacity) {
    std::lock_guard<std::recursive_mutex> guard(g_lock);
    if (!doc || !doc->doc) return -1;
    int n = 0;
    walk_bookmarks(doc, nullptr, 0, out, capacity, &n);
    return n;
}

/* ---- Annotated export ------------------------------------------------ */

}  // extern "C"

/* C++ helpers of the export: C++ linkage (utf16 returns a std::vector). */
namespace {

struct FileWrite {
    int version = 1;
    int (*write_block)(FileWrite *self, const void *data, unsigned long size) = nullptr;
    FILE *file = nullptr;
};

int write_block(FileWrite *self, const void *data, unsigned long size) {
    return fwrite(data, 1, size, self->file) == size ? 1 : 0;
}

std::vector<unsigned short> utf16(const char *utf8) {
    const std::wstring w = widen(utf8);
    std::vector<unsigned short> out(w.begin(), w.end());
    out.push_back(0);
    return out;
}

}  // namespace

extern "C" {

int zant_pdf_save_annotated(const char *source_path, const char *password, const char *out_path,
                            const ZantPdfAnnotation *annotations, int count) {
    std::lock_guard<std::recursive_mutex> guard(g_lock);
    if (!out_path || !pdfium() || !g_pdfium.doc_api || !g_pdfium.save_api) return -1;
    int locked = 0;
    /* a separate copy: the displayed document stays unchanged */
    ZantPdfDocument *doc = zant_pdf_open(source_path, password, &locked);
    if (!doc) return -1;
    constexpr int kHighlight = 9, kUnderline = 10, kSquiggly = 11, kStrikeOut = 12, kInk = 15, kText = 1;
    constexpr int kFlagPrint = 4;
    for (int i = 0; i < count; i++) {
        const ZantPdfAnnotation *a = &annotations[i];
        PageScope p(doc, a->page, false);
        if (!p.page) continue;
        auto to_page = [&](float x, float y, float *px, float *py) {
            double dx = 0, dy = 0;
            g_pdfium.device_to_page(p.page, 0, 0, std::max(1, (int)std::lround(p.w * kSub)),
                                    std::max(1, (int)std::lround(p.h * kSub)), 0, (int)std::lround(x * kSub),
                                    (int)std::lround(y * kSub), &dx, &dy);
            *px = (float)dx;
            *py = (float)dy;
        };
        int subtype = kHighlight;
        switch (a->kind) {
            case ZANT_PDF_ANNOT_UNDERLINE: subtype = kUnderline; break;
            case ZANT_PDF_ANNOT_STRIKEOUT: subtype = kStrikeOut; break;
            case ZANT_PDF_ANNOT_SQUIGGLY: subtype = kSquiggly; break;
            case ZANT_PDF_ANNOT_INK: subtype = kInk; break;
            case ZANT_PDF_ANNOT_NOTE: subtype = kText; break;
            default: break;
        }
        void *annot = g_pdfium.page_create_annot(p.page, subtype);
        if (!annot) continue;
        float l = 1e9f, t = -1e9f, r = -1e9f, b = 1e9f; /* page-space bounds (y up) */
        auto grow = [&](float x, float y) {
            l = std::min(l, x);
            r = std::max(r, x);
            b = std::min(b, y);
            t = std::max(t, y);
        };
        if (a->kind <= ZANT_PDF_ANNOT_SQUIGGLY || a->kind == ZANT_PDF_ANNOT_NOTE) {
            for (int k = 0; k < a->rect_count; k++) {
                const float *v = &a->rects[k * 4];
                float x1, y1, x2, y2, x3, y3, x4, y4;
                to_page(v[0], v[1], &x1, &y1);               /* top-left */
                to_page(v[0] + v[2], v[1], &x2, &y2);        /* top-right */
                to_page(v[0], v[1] + v[3], &x3, &y3);        /* bottom-left */
                to_page(v[0] + v[2], v[1] + v[3], &x4, &y4); /* bottom-right */
                grow(x1, y1);
                grow(x2, y2);
                grow(x3, y3);
                grow(x4, y4);
                if (a->kind != ZANT_PDF_ANNOT_NOTE) {
                    const float quad[8] = {x1, y1, x2, y2, x3, y3, x4, y4};
                    g_pdfium.annot_append_attachment_points(annot, quad);
                }
            }
        } else if (a->kind == ZANT_PDF_ANNOT_INK) {
            int offset = 0;
            for (int s = 0; s < a->stroke_count; s++) {
                const int n = a->stroke_lengths[s];
                std::vector<float> pts(static_cast<size_t>(n) * 2);
                for (int k = 0; k < n; k++) {
                    to_page(a->points[(offset + k) * 2], a->points[(offset + k) * 2 + 1], &pts[k * 2], &pts[k * 2 + 1]);
                    grow(pts[k * 2], pts[k * 2 + 1]);
                }
                if (n > 0) g_pdfium.annot_add_ink_stroke(annot, pts.data(), static_cast<size_t>(n));
                offset += n;
            }
            g_pdfium.annot_set_border(annot, 0, 0, a->width > 0 ? a->width : 2);
            const float pad = std::max(1.0f, a->width);
            l -= pad;
            r += pad;
            b -= pad;
            t += pad;
        }
        if (l <= r && b <= t) {
            const float rect[4] = {l, t, r, b}; /* FS_RECTF: left, top, right, bottom */
            g_pdfium.annot_set_rect(annot, rect);
        }
        const uint32_t c = a->color;
        g_pdfium.annot_set_color(annot, 0, (c >> 16) & 0xff, (c >> 8) & 0xff, c & 0xff, (c >> 24) & 0xff);
        g_pdfium.annot_set_flags(annot, kFlagPrint);
        if (a->contents && a->contents[0]) g_pdfium.annot_set_string_value(annot, "Contents", utf16(a->contents).data());
        SYSTEMTIME st;
        GetSystemTime(&st);
        char date[32];
        snprintf(date, sizeof(date), "D:%04d%02d%02d%02d%02d%02dZ", st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute,
                 st.wSecond);
        g_pdfium.annot_set_string_value(annot, "M", utf16(date).data());
        g_pdfium.page_close_annot(annot);
    }
    FILE *file = _wfopen(widen(out_path).c_str(), L"wb");
    int result = -1;
    if (file) {
        FileWrite writer;
        writer.write_block = write_block;
        writer.file = file;
        result = g_pdfium.save_as_copy(doc->doc, &writer, 0) ? 0 : -1;
        if (fclose(file) != 0) result = -1;
    }
    zant_pdf_close(doc);
    return result;
}

}  // extern "C"
