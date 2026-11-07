// Windows PDF backend for ui.pdfPreview (src/platform/pdf.h), built into
// zant_skia.dll. Uses PDFium (pdfium.dll next to the executable or the
// DLL, loaded on first use); without it the preview shows its
// "cannot preview" state. Rendered pages are cached as PNG files in the
// temp directory, like PDFKit on macOS.
#include "src/platform/pdf.h"

#include "include/core/SkData.h"
#include "include/core/SkImageInfo.h"
#include "include/core/SkPixmap.h"
#include "include/core/SkStream.h"
#include "include/encode/SkPngEncoder.h"

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <mutex>
#include <cstring>
#include <string>
#include <vector>

bool zant_write_file_utf8(const char *path, const void *data, size_t size);

namespace {

using Doc = void *;
using Page = void *;
using Bitmap = void *;
using TextPage = void *;

struct Pdfium {
    bool tried = false;
    bool ok = false;
    void (*init_library)() = nullptr;
    Doc (*load_mem_document64)(const void *, size_t, const char *) = nullptr;
    void (*close_document)(Doc) = nullptr;
    int (*page_count)(Doc) = nullptr;
    Page (*load_page)(Doc, int) = nullptr;
    void (*close_page)(Page) = nullptr;
    double (*page_width)(Page) = nullptr;
    double (*page_height)(Page) = nullptr;
    Bitmap (*bitmap_create)(int, int, int) = nullptr;
    int (*bitmap_fill_rect)(Bitmap, int, int, int, int, unsigned long) = nullptr;
    void (*render_page_bitmap)(Bitmap, Page, int, int, int, int, int, int) = nullptr;
    void *(*bitmap_buffer)(Bitmap) = nullptr;
    int (*bitmap_stride)(Bitmap) = nullptr;
    void (*bitmap_destroy)(Bitmap) = nullptr;
    TextPage (*text_load_page)(Page) = nullptr;
    void (*text_close_page)(TextPage) = nullptr;
    int (*text_count_chars)(TextPage) = nullptr;
    int (*text_get_text)(TextPage, int, int, unsigned short *) = nullptr;
    // document API (ui.pdfViewer); the optional ones may be missing in old builds
    unsigned long (*get_last_error)() = nullptr;
    Bitmap (*bitmap_create_ex)(int, int, int, void *, int) = nullptr;
    float (*page_width_f)(Page) = nullptr;
    float (*page_height_f)(Page) = nullptr;
    int (*text_get_loose_char_box)(TextPage, int, float *) = nullptr;
    int (*text_get_char_box)(TextPage, int, double *, double *, double *, double *) = nullptr;
    unsigned int (*text_get_unicode)(TextPage, int) = nullptr;
    double (*text_get_font_size)(TextPage, int) = nullptr;
    void *(*text_get_text_object)(TextPage, int) = nullptr;  // optional
    int (*page_to_device)(Page, int, int, int, int, int, double, double, int *, int *) = nullptr;
    int (*link_enumerate)(Page, int *, void **) = nullptr;
    int (*link_get_annot_rect)(void *, float *) = nullptr;
    void *(*link_get_dest)(Doc, void *) = nullptr;
    void *(*link_get_action)(void *) = nullptr;
    unsigned long (*action_get_type)(void *) = nullptr;
    void *(*action_get_dest)(Doc, void *) = nullptr;
    unsigned long (*action_get_uri_path)(Doc, void *, void *, unsigned long) = nullptr;
    int (*dest_get_page_index)(Doc, void *) = nullptr;
    int (*dest_get_location)(void *, int *, int *, int *, float *, float *, float *) = nullptr;
    void *(*bookmark_first_child)(Doc, void *) = nullptr;
    void *(*bookmark_next_sibling)(Doc, void *) = nullptr;
    unsigned long (*bookmark_get_title)(void *, void *, unsigned long) = nullptr;
    void *(*bookmark_get_dest)(Doc, void *) = nullptr;
    void *(*bookmark_get_action)(void *) = nullptr;
    bool doc_api = false;
    // annotated export
    int (*device_to_page)(Page, int, int, int, int, int, int, int, double *, double *) = nullptr;
    void *(*page_create_annot)(Page, int) = nullptr;
    void (*page_close_annot)(void *) = nullptr;
    int (*annot_set_color)(void *, int, unsigned int, unsigned int, unsigned int, unsigned int) = nullptr;
    int (*annot_append_attachment_points)(void *, const float *) = nullptr;
    int (*annot_set_rect)(void *, const float *) = nullptr;
    int (*annot_add_ink_stroke)(void *, const float *, size_t) = nullptr;
    int (*annot_set_border)(void *, float, float, float) = nullptr;
    int (*annot_set_string_value)(void *, const char *, const unsigned short *) = nullptr;
    int (*annot_set_flags)(void *, int) = nullptr;
    int (*save_as_copy)(Doc, void *, unsigned long) = nullptr;
    bool save_api = false;
};

/* PDFium is not thread-safe: every call goes through this lock. */
std::recursive_mutex g_lock;

Pdfium g_pdfium;

std::wstring widen(const char *utf8) {
    if (!utf8) return {};
    const int n = MultiByteToWideChar(CP_UTF8, 0, utf8, -1, nullptr, 0);
    if (n <= 0) return {};
    std::wstring out(static_cast<size_t>(n), L'\0');
    MultiByteToWideChar(CP_UTF8, 0, utf8, -1, out.data(), n);
    out.resize(static_cast<size_t>(n - 1));
    return out;
}

std::string narrow(const wchar_t *wide, int length) {
    const int n = WideCharToMultiByte(CP_UTF8, 0, wide, length, nullptr, 0, nullptr, nullptr);
    if (n <= 0) return {};
    std::string out(static_cast<size_t>(n), '\0');
    WideCharToMultiByte(CP_UTF8, 0, wide, length, out.data(), n, nullptr, nullptr);
    return out;
}

std::wstring module_dir(HMODULE module) {
    wchar_t buf[MAX_PATH * 4];
    const DWORD n = GetModuleFileNameW(module, buf, static_cast<DWORD>(std::size(buf)));
    std::wstring path(buf, n);
    const size_t slash = path.find_last_of(L"\\/");
    return slash == std::wstring::npos ? std::wstring() : path.substr(0, slash);
}

template <typename T>
bool bind(HMODULE lib, T &slot, const char *name) {
    slot = reinterpret_cast<T>(reinterpret_cast<void *>(GetProcAddress(lib, name)));
    return slot != nullptr;
}

bool pdfium() {
    Pdfium &p = g_pdfium;
    if (p.tried) return p.ok;
    p.tried = true;
    HMODULE self = nullptr;
    GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                       reinterpret_cast<LPCWSTR>(&pdfium), &self);
    HMODULE lib = nullptr;
    for (const std::wstring &dir : {module_dir(nullptr), module_dir(self)}) {
        if (dir.empty()) continue;
        lib = LoadLibraryExW((dir + L"\\pdfium.dll").c_str(), nullptr, LOAD_WITH_ALTERED_SEARCH_PATH);
        if (lib) break;
    }
    if (!lib) lib = LoadLibraryW(L"pdfium.dll");
    if (!lib) {
        fprintf(stderr, "[zant] pdfium.dll not found; PDF preview is unavailable\n");
        return false;
    }
    p.ok = bind(lib, p.init_library, "FPDF_InitLibrary") &&
           bind(lib, p.load_mem_document64, "FPDF_LoadMemDocument64") &&
           bind(lib, p.close_document, "FPDF_CloseDocument") && bind(lib, p.page_count, "FPDF_GetPageCount") &&
           bind(lib, p.load_page, "FPDF_LoadPage") && bind(lib, p.close_page, "FPDF_ClosePage") &&
           bind(lib, p.page_width, "FPDF_GetPageWidth") && bind(lib, p.page_height, "FPDF_GetPageHeight") &&
           bind(lib, p.bitmap_create, "FPDFBitmap_Create") &&
           bind(lib, p.bitmap_fill_rect, "FPDFBitmap_FillRect") &&
           bind(lib, p.render_page_bitmap, "FPDF_RenderPageBitmap") &&
           bind(lib, p.bitmap_buffer, "FPDFBitmap_GetBuffer") &&
           bind(lib, p.bitmap_stride, "FPDFBitmap_GetStride") &&
           bind(lib, p.bitmap_destroy, "FPDFBitmap_Destroy") &&
           bind(lib, p.text_load_page, "FPDFText_LoadPage") &&
           bind(lib, p.text_close_page, "FPDFText_ClosePage") &&
           bind(lib, p.text_count_chars, "FPDFText_CountChars") &&
           bind(lib, p.text_get_text, "FPDFText_GetText");
    if (!p.ok) {
        fprintf(stderr, "[zant] pdfium.dll is missing expected functions\n");
        return false;
    }
    bind(lib, p.text_get_text_object, "FPDFText_GetTextObject");
    p.doc_api = bind(lib, p.get_last_error, "FPDF_GetLastError") &&
                bind(lib, p.bitmap_create_ex, "FPDFBitmap_CreateEx") &&
                bind(lib, p.page_width_f, "FPDF_GetPageWidthF") &&
                bind(lib, p.page_height_f, "FPDF_GetPageHeightF") &&
                bind(lib, p.text_get_loose_char_box, "FPDFText_GetLooseCharBox") &&
                bind(lib, p.text_get_char_box, "FPDFText_GetCharBox") &&
                bind(lib, p.text_get_unicode, "FPDFText_GetUnicode") &&
                bind(lib, p.text_get_font_size, "FPDFText_GetFontSize") &&
                bind(lib, p.page_to_device, "FPDF_PageToDevice") &&
                bind(lib, p.link_enumerate, "FPDFLink_Enumerate") &&
                bind(lib, p.link_get_annot_rect, "FPDFLink_GetAnnotRect") &&
                bind(lib, p.link_get_dest, "FPDFLink_GetDest") &&
                bind(lib, p.link_get_action, "FPDFLink_GetAction") &&
                bind(lib, p.action_get_type, "FPDFAction_GetType") &&
                bind(lib, p.action_get_dest, "FPDFAction_GetDest") &&
                bind(lib, p.action_get_uri_path, "FPDFAction_GetURIPath") &&
                bind(lib, p.dest_get_page_index, "FPDFDest_GetDestPageIndex") &&
                bind(lib, p.dest_get_location, "FPDFDest_GetLocationInPage") &&
                bind(lib, p.bookmark_first_child, "FPDFBookmark_GetFirstChild") &&
                bind(lib, p.bookmark_next_sibling, "FPDFBookmark_GetNextSibling") &&
                bind(lib, p.bookmark_get_title, "FPDFBookmark_GetTitle") &&
                bind(lib, p.bookmark_get_dest, "FPDFBookmark_GetDest") &&
                bind(lib, p.bookmark_get_action, "FPDFBookmark_GetAction");
    if (!p.doc_api) fprintf(stderr, "[zant] pdfium.dll is too old for the PDF viewer\n");
    p.save_api = bind(lib, p.device_to_page, "FPDF_DeviceToPage") &&
                 bind(lib, p.page_create_annot, "FPDFPage_CreateAnnot") &&
                 bind(lib, p.page_close_annot, "FPDFPage_CloseAnnot") &&
                 bind(lib, p.annot_set_color, "FPDFAnnot_SetColor") &&
                 bind(lib, p.annot_append_attachment_points, "FPDFAnnot_AppendAttachmentPoints") &&
                 bind(lib, p.annot_set_rect, "FPDFAnnot_SetRect") &&
                 bind(lib, p.annot_add_ink_stroke, "FPDFAnnot_AddInkStroke") &&
                 bind(lib, p.annot_set_border, "FPDFAnnot_SetBorder") &&
                 bind(lib, p.annot_set_string_value, "FPDFAnnot_SetStringValue") &&
                 bind(lib, p.annot_set_flags, "FPDFAnnot_SetFlags") &&
                 bind(lib, p.save_as_copy, "FPDF_SaveAsCopy");
    p.init_library();
    return true;
}

/* The last opened document stays open: ui.pdfPreview asks for the page
   count and page image on every rebuild. */
struct OpenDoc {
    std::string path;
    FILETIME write_time{};
    uint64_t size = 0;
    std::vector<char> bytes; /* PDFium reads from it while the doc is open */
    Doc doc = nullptr;
};

OpenDoc g_doc;

bool file_stamp(const std::wstring &path, FILETIME *time, uint64_t *size) {
    WIN32_FILE_ATTRIBUTE_DATA data;
    if (!GetFileAttributesExW(path.c_str(), GetFileExInfoStandard, &data)) return false;
    *time = data.ftLastWriteTime;
    *size = (static_cast<uint64_t>(data.nFileSizeHigh) << 32) | data.nFileSizeLow;
    return true;
}

Doc open_doc(const char *path) {
    if (!path || !path[0] || !pdfium()) return nullptr;
    const std::wstring wide = widen(path);
    FILETIME time{};
    uint64_t size = 0;
    if (!file_stamp(wide, &time, &size)) return nullptr;
    if (g_doc.doc && g_doc.path == path && g_doc.size == size &&
        CompareFileTime(&g_doc.write_time, &time) == 0) {
        return g_doc.doc;
    }
    if (g_doc.doc) g_pdfium.close_document(g_doc.doc);
    g_doc = OpenDoc{};
    FILE *file = _wfopen(wide.c_str(), L"rb");
    if (!file) return nullptr;
    std::vector<char> bytes(static_cast<size_t>(size));
    const size_t read = size ? fread(bytes.data(), 1, bytes.size(), file) : 0;
    fclose(file);
    if (read != bytes.size()) return nullptr;
    g_doc.bytes = std::move(bytes);
    g_doc.doc = g_pdfium.load_mem_document64(g_doc.bytes.data(), g_doc.bytes.size(), nullptr);
    if (!g_doc.doc) {
        g_doc = OpenDoc{};
        return nullptr;
    }
    g_doc.path = path;
    g_doc.write_time = time;
    g_doc.size = size;
    return g_doc.doc;
}

int clamp_page(Doc doc, int page) {
    const int count = g_pdfium.page_count(doc);
    if (count <= 0) return -1;
    return std::max(0, std::min(page, count - 1));
}

bool copy_out(const std::string &value, char *out, size_t capacity) {
    if (!out || capacity == 0 || value.size() + 1 > capacity) return false;
    memcpy(out, value.c_str(), value.size() + 1);
    return true;
}

}  // namespace

extern "C" {

int zant_pdf_page_count(const char *path) {
    std::lock_guard<std::recursive_mutex> guard(g_lock);
    Doc doc = open_doc(path);
    if (!doc) return 0;
    const int count = g_pdfium.page_count(doc);
    return count > 0 ? count : 0;
}

int zant_pdf_render_page(const char *path, int page, int width, int height, char *out_path,
                         size_t capacity) {
    std::lock_guard<std::recursive_mutex> guard(g_lock);
    if (width <= 0 || height <= 0) return -1;
    Doc doc = open_doc(path);
    if (!doc) return -1;
    page = clamp_page(doc, page);
    if (page < 0) return -1;

    wchar_t temp[MAX_PATH + 1];
    const DWORD temp_len = GetTempPathW(MAX_PATH + 1, temp);
    if (temp_len == 0) return -1;
    const size_t hash = std::hash<std::string>{}(g_doc.path) ^ (static_cast<size_t>(page) * 1315423911u) ^
                        (static_cast<size_t>(width) << 8) ^ static_cast<size_t>(height) ^
                        static_cast<size_t>(g_doc.write_time.dwLowDateTime);
    wchar_t name[64];
    swprintf(name, std::size(name), L"zant-pdf-%llx.png", static_cast<unsigned long long>(hash));
    const std::wstring output = std::wstring(temp, temp_len) + name;
    const std::string output_utf8 = narrow(output.c_str(), static_cast<int>(output.size()));
    if (GetFileAttributesW(output.c_str()) != INVALID_FILE_ATTRIBUTES) {
        return copy_out(output_utf8, out_path, capacity) ? 0 : -1;
    }

    Page pdf_page = g_pdfium.load_page(doc, page);
    if (!pdf_page) return -1;
    /* the page fits the box, centred, on white (like PDFKit's thumbnail) */
    const double pw = g_pdfium.page_width(pdf_page);
    const double ph = g_pdfium.page_height(pdf_page);
    const double scale = (pw > 0 && ph > 0) ? std::min(width / pw, height / ph) : 1.0;
    const int draw_w = std::max(1, static_cast<int>(pw * scale + 0.5));
    const int draw_h = std::max(1, static_cast<int>(ph * scale + 0.5));
    Bitmap bitmap = g_pdfium.bitmap_create(width, height, 1);
    if (!bitmap) {
        g_pdfium.close_page(pdf_page);
        return -1;
    }
    g_pdfium.bitmap_fill_rect(bitmap, 0, 0, width, height, 0xFFFFFFFFul);
    constexpr int kAnnot = 0x01, kLcdText = 0x02;
    g_pdfium.render_page_bitmap(bitmap, pdf_page, (width - draw_w) / 2, (height - draw_h) / 2, draw_w, draw_h,
                                0, kAnnot | kLcdText);
    const SkImageInfo info = SkImageInfo::Make(width, height, kBGRA_8888_SkColorType, kPremul_SkAlphaType);
    SkPixmap pixmap(info, g_pdfium.bitmap_buffer(bitmap), static_cast<size_t>(g_pdfium.bitmap_stride(bitmap)));
    SkDynamicMemoryWStream stream;
    const bool encoded = SkPngEncoder::Encode(&stream, pixmap, SkPngEncoder::Options());
    g_pdfium.bitmap_destroy(bitmap);
    g_pdfium.close_page(pdf_page);
    if (!encoded) return -1;
    sk_sp<SkData> png = stream.detachAsData();
    if (!zant_write_file_utf8(output_utf8.c_str(), png->data(), png->size())) return -1;
    return copy_out(output_utf8, out_path, capacity) ? 0 : -1;
}

int zant_pdf_page_text(const char *path, int page, char *out_text, size_t capacity) {
    std::lock_guard<std::recursive_mutex> guard(g_lock);
    if (!out_text || capacity == 0) return -1;
    Doc doc = open_doc(path);
    if (!doc) return -1;
    page = clamp_page(doc, page);
    if (page < 0) return -1;
    Page pdf_page = g_pdfium.load_page(doc, page);
    if (!pdf_page) return -1;
    TextPage text = g_pdfium.text_load_page(pdf_page);
    std::string utf8;
    if (text) {
        const int count = g_pdfium.text_count_chars(text);
        if (count > 0) {
            std::vector<unsigned short> units(static_cast<size_t>(count) + 1);
            const int written = g_pdfium.text_get_text(text, 0, count, units.data());
            /* written includes the terminating NUL */
            if (written > 1) utf8 = narrow(reinterpret_cast<const wchar_t *>(units.data()), written - 1);
            /* PDFium ends lines with CRLF; the framework's text is LF (as on macOS) */
            utf8.erase(std::remove(utf8.begin(), utf8.end(), '\r'), utf8.end());
        }
        g_pdfium.text_close_page(text);
    }
    g_pdfium.close_page(pdf_page);
    if (!text) return -1;
    size_t length = std::min(utf8.size(), capacity - 1);
    /* do not cut a UTF-8 sequence in half */
    while (length > 0 && length < utf8.size() && (static_cast<unsigned char>(utf8[length]) & 0xC0) == 0x80) {
        length--;
    }
    memcpy(out_text, utf8.data(), length);
    out_text[length] = 0;
    return 0;
}

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
