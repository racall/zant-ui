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
#include <cstdio>
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
};

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
    Doc doc = open_doc(path);
    if (!doc) return 0;
    const int count = g_pdfium.page_count(doc);
    return count > 0 ? count : 0;
}

int zant_pdf_render_page(const char *path, int page, int width, int height, char *out_path,
                         size_t capacity) {
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

}  // extern "C"
