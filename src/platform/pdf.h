#ifndef ZANT_PDF_H
#define ZANT_PDF_H
#include <stddef.h>
#ifdef __cplusplus
extern "C" {
#endif
int zant_pdf_page_count(const char *path);
int zant_pdf_render_page(const char *path, int page, int width, int height,
                         char *out_path, size_t capacity);
int zant_pdf_page_text(const char *path, int page, char *out_text, size_t capacity);
#ifdef __cplusplus
}
#endif
#endif
