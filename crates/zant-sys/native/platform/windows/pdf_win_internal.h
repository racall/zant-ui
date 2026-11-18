#ifndef ZANT_PDF_WIN_INTERNAL_H
#define ZANT_PDF_WIN_INTERNAL_H

#include "platform/pdf.h"

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>

#include <mutex>
#include <string>

namespace zant_pdf_win {

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

extern std::recursive_mutex g_lock;
extern Pdfium g_pdfium;

std::wstring widen(const char *utf8);
std::string narrow(const wchar_t *wide, int length);
bool pdfium();

}  // namespace zant_pdf_win

#endif
