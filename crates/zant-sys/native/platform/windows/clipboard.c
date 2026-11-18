#include "window_internal.h"

#include <shellapi.h>
#include <shlobj.h>
#include <oleidl.h>
#include <stdlib.h>
#include <string.h>

/* ---------- clipboard ---------- */

int zant_clipboard_set_text(const char *text, size_t length) {
    if (!text) return 0;
    const int units = MultiByteToWideChar(CP_UTF8, 0, text, (int)length, NULL, 0);
    if (units < 0) return 0;
    wchar_t *wide = (wchar_t *)malloc(((size_t)units + 1) * sizeof(wchar_t));
    if (!wide) return 0;
    if (units > 0) MultiByteToWideChar(CP_UTF8, 0, text, (int)length, wide, units);
    /* the Windows clipboard convention is CRLF (zant_clipboard_get_text maps back) */
    size_t lone_lf = 0;
    for (int i = 0; i < units; i++) {
        if (wide[i] == L'\n' && (i == 0 || wide[i - 1] != L'\r')) lone_lf++;
    }
    HGLOBAL memory = GlobalAlloc(GMEM_MOVEABLE, ((size_t)units + lone_lf + 1) * sizeof(wchar_t));
    if (!memory) {
        free(wide);
        return 0;
    }
    wchar_t *dst = (wchar_t *)GlobalLock(memory);
    size_t n = 0;
    for (int i = 0; i < units; i++) {
        if (wide[i] == L'\n' && (i == 0 || wide[i - 1] != L'\r')) dst[n++] = L'\r';
        dst[n++] = wide[i];
    }
    dst[n] = 0;
    GlobalUnlock(memory);
    free(wide);
    HWND owner = g_main ? g_main->hwnd : NULL;
    for (int attempt = 0; attempt < 5; attempt++) {
        if (OpenClipboard(owner)) {
            EmptyClipboard();
            const bool ok = SetClipboardData(CF_UNICODETEXT, memory) != NULL;
            CloseClipboard();
            if (!ok) GlobalFree(memory);
            return ok ? 1 : 0;
        }
        Sleep(10);
    }
    GlobalFree(memory);
    return 0;
}

int zant_clipboard_clear(void) {
    HWND owner = g_main ? g_main->hwnd : NULL;
    for (int attempt = 0; attempt < 5; attempt++) {
        if (OpenClipboard(owner)) {
            const bool ok = EmptyClipboard() != 0;
            CloseClipboard();
            return ok ? 1 : 0;
        }
        Sleep(10);
    }
    return 0;
}

size_t zant_clipboard_get_text(char *out, size_t capacity) {
    HWND owner = g_main ? g_main->hwnd : NULL;
    size_t required = 1;
    bool opened = false;
    for (int attempt = 0; attempt < 5 && !opened; attempt++) {
        opened = OpenClipboard(owner) != 0;
        if (!opened) Sleep(10);
    }
    char *utf8 = NULL;
    int len = 0;
    if (opened) {
        HANDLE data = GetClipboardData(CF_UNICODETEXT);
        const wchar_t *wide = data ? (const wchar_t *)GlobalLock(data) : NULL;
        if (wide) {
            utf8 = zant_narrow(wide, -1, &len);
            GlobalUnlock(data);
        }
        CloseClipboard();
    }
    /* CRLF -> LF, like text copied on macOS */
    if (utf8) {
        int j = 0;
        for (int i = 0; i < len; i++) {
            if (utf8[i] == '\r' && i + 1 < len && utf8[i + 1] == '\n') continue;
            utf8[j++] = utf8[i];
        }
        len = j;
        utf8[len] = 0;
        required = (size_t)len + 1;
    }
    if (out && capacity > 0) {
        const size_t copy = utf8 ? ((size_t)len < capacity - 1 ? (size_t)len : capacity - 1) : 0;
        if (copy) memcpy(out, utf8, copy);
        out[copy] = 0;
    }
    free(utf8);
    return required;
}

/* The clipboard can be held briefly by the app that just wrote to it. */
static bool zant_open_clipboard(HWND owner) {
    for (int i = 0; i < 10; i++) {
        if (OpenClipboard(owner)) return true;
        Sleep(20);
    }
    return false;
}

/* A bitmap (BITMAPINFOHEADER / BITMAPV5HEADER + pixels) as PNG, through Skia. */
static uint8_t *zant_dib_to_png(const uint8_t *dib, size_t size, size_t *out_len) {
    if (size < sizeof(BITMAPINFOHEADER)) return NULL;
    const BITMAPINFOHEADER *bi = (const BITMAPINFOHEADER *)dib;
    const int width = bi->biWidth;
    const int height = bi->biHeight < 0 ? -bi->biHeight : bi->biHeight;
    const bool top_down = bi->biHeight < 0;
    const int bpp = bi->biBitCount;
    if (width <= 0 || height <= 0 || (bpp != 24 && bpp != 32) || width > 16384 || height > 16384) return NULL;
    if (bi->biCompression != BI_RGB && bi->biCompression != BI_BITFIELDS) return NULL;
    size_t offset = bi->biSize;
    if (bi->biCompression == BI_BITFIELDS && bi->biSize == sizeof(BITMAPINFOHEADER)) offset += 12; /* masks */
    offset += (size_t)bi->biClrUsed * 4;
    const size_t stride = (((size_t)width * (size_t)bpp + 31) / 32) * 4;
    if (offset + stride * (size_t)height > size) return NULL;
    uint8_t *pixels = (uint8_t *)malloc((size_t)width * (size_t)height * 4);
    if (!pixels) return NULL;
    /* 32-bit bitmaps often leave alpha at 0: then the image is opaque */
    bool any_alpha = false;
    if (bpp == 32) {
        for (int y = 0; y < height && !any_alpha; y++) {
            const uint8_t *row = dib + offset + stride * (size_t)y;
            for (int x = 0; x < width; x++) if (row[x * 4 + 3]) { any_alpha = true; break; }
        }
    }
    for (int y = 0; y < height; y++) {
        const uint8_t *src = dib + offset + stride * (size_t)(top_down ? y : height - 1 - y);
        uint8_t *dst = pixels + (size_t)y * (size_t)width * 4;
        for (int x = 0; x < width; x++) {
            const uint8_t b = src[x * (bpp / 8)], g = src[x * (bpp / 8) + 1], r = src[x * (bpp / 8) + 2];
            const uint8_t a = (bpp == 32 && any_alpha) ? src[x * 4 + 3] : 255;
            /* Skia N32 premultiplied: BGRA on Windows */
            dst[x * 4 + 0] = (uint8_t)(b * a / 255);
            dst[x * 4 + 1] = (uint8_t)(g * a / 255);
            dst[x * 4 + 2] = (uint8_t)(r * a / 255);
            dst[x * 4 + 3] = a;
        }
    }
    ZantImage *image = zant_image_from_pixels(pixels, width, height, width * 4);
    free(pixels);
    if (!image) return NULL;
    size_t png_len = 0;
    void *png = zant_image_encode_png(image, &png_len);
    zant_image_destroy(image);
    if (!png) return NULL;
    uint8_t *copy = (uint8_t *)malloc(png_len);
    if (copy) memcpy(copy, png, png_len);
    zant_bytes_free(png);
    if (copy && out_len) *out_len = png_len;
    return copy;
}

uint8_t *zant_clipboard_get_image_png(size_t *out_len) {
    if (out_len) *out_len = 0;
    const UINT png_format = RegisterClipboardFormatW(L"PNG");
    if (!IsClipboardFormatAvailable(png_format) && !IsClipboardFormatAvailable(CF_DIBV5) &&
        !IsClipboardFormatAvailable(CF_DIB))
        return NULL;
    if (!zant_open_clipboard(zant_app_window())) return NULL;
    uint8_t *result = NULL;
    HANDLE handle = GetClipboardData(png_format);
    if (handle) {
        const SIZE_T size = GlobalSize(handle);
        const void *data = GlobalLock(handle);
        if (data && size > 8) {
            result = (uint8_t *)malloc(size);
            if (result) {
                memcpy(result, data, size);
                if (out_len) *out_len = size;
            }
        }
        if (data) GlobalUnlock(handle);
    }
    /* a bitmap: copied out, then converted with the clipboard closed (a large
       one takes a while to encode; other apps must not wait for it) */
    uint8_t *dib = NULL;
    SIZE_T dib_size = 0;
    if (!result) {
        handle = GetClipboardData(CF_DIBV5);
        if (!handle) handle = GetClipboardData(CF_DIB);
        if (handle) {
            dib_size = GlobalSize(handle);
            const uint8_t *data = (const uint8_t *)GlobalLock(handle);
            if (data) {
                dib = (uint8_t *)malloc(dib_size);
                if (dib) memcpy(dib, data, dib_size);
                GlobalUnlock(handle);
            }
        }
    }
    CloseClipboard();
    if (dib) {
        result = zant_dib_to_png(dib, dib_size, out_len);
        free(dib);
    }
    return result;
}

/* ---- files on the clipboard (CF_HDROP, as Explorer copies them) ---- */

char *zant_clipboard_get_files(size_t *out_len) {
    if (out_len) *out_len = 0;
    if (!IsClipboardFormatAvailable(CF_HDROP)) return NULL;
    if (!zant_open_clipboard(zant_app_window())) return NULL;
    char *result = NULL;
    size_t used = 0, capacity = 0;
    HANDLE handle = GetClipboardData(CF_HDROP);
    HDROP drop = handle ? (HDROP)GlobalLock(handle) : NULL;
    if (drop) {
        const UINT count = DragQueryFileW(drop, 0xFFFFFFFF, NULL, 0);
        for (UINT i = 0; i < count; i++) {
            const UINT n = DragQueryFileW(drop, i, NULL, 0);
            wchar_t *wide = (wchar_t *)malloc(((size_t)n + 1) * sizeof(wchar_t));
            if (!wide) continue;
            DragQueryFileW(drop, i, wide, n + 1);
            int len = 0;
            char *utf8 = zant_narrow(wide, (int)n, &len);
            free(wide);
            if (!utf8) continue;
            if (used + (size_t)len + 2 > capacity) {
                capacity = (used + (size_t)len + 2) * 2;
                char *grown = (char *)realloc(result, capacity);
                if (!grown) {
                    free(utf8);
                    break;
                }
                result = grown;
            }
            if (used > 0) result[used++] = '\n';
            memcpy(result + used, utf8, (size_t)len);
            used += (size_t)len;
            result[used] = '\0';
            free(utf8);
        }
        GlobalUnlock(handle);
    }
    CloseClipboard();
    if (!result || used == 0) {
        free(result);
        return NULL;
    }
    if (out_len) *out_len = used;
    return result;
}

int zant_clipboard_set_files(const char *paths, size_t len) {
    if (!paths || len == 0) return 0;
    /* DROPFILES + each path as UTF-16 with its NUL + a final NUL */
    char *copy = (char *)malloc(len + 1);
    if (!copy) return 0;
    memcpy(copy, paths, len);
    copy[len] = '\0';
    /* one NUL-terminated path per line (empty lines are skipped) */
    for (size_t i = 0; i < len; i++) {
        if (copy[i] == '\n' || copy[i] == '\r') copy[i] = '\0';
    }
    size_t wide_units = 1;
    for (size_t at = 0; at < len; at += strlen(copy + at) + 1) {
        const int n = MultiByteToWideChar(CP_UTF8, 0, copy + at, -1, NULL, 0);
        if (n > 1) wide_units += (size_t)n;
    }
    if (wide_units == 1) {
        free(copy);
        return 0;
    }
    const size_t bytes = sizeof(DROPFILES) + wide_units * sizeof(wchar_t);
    HGLOBAL mem = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, bytes);
    if (!mem) {
        free(copy);
        return 0;
    }
    DROPFILES *drop = (DROPFILES *)GlobalLock(mem);
    drop->pFiles = sizeof(DROPFILES);
    drop->fWide = TRUE;
    wchar_t *out = (wchar_t *)((char *)drop + sizeof(DROPFILES));
    for (size_t at = 0; at < len;) {
        const char *line = copy + at;
        const size_t line_len = strlen(line);
        const int n = MultiByteToWideChar(CP_UTF8, 0, line, -1, NULL, 0);
        if (n > 1) {
            MultiByteToWideChar(CP_UTF8, 0, line, -1, out, n);
            out += n;
        }
        at += line_len + 1;
    }
    GlobalUnlock(mem);
    free(copy);
    /* paste copies (not moves) the files */
    HGLOBAL effect = GlobalAlloc(GMEM_MOVEABLE, sizeof(DWORD));
    if (effect) {
        *(DWORD *)GlobalLock(effect) = DROPEFFECT_COPY;
        GlobalUnlock(effect);
    }
    if (!zant_open_clipboard(zant_app_window())) {
        GlobalFree(mem);
        if (effect) GlobalFree(effect);
        return 0;
    }
    EmptyClipboard();
    const int ok = SetClipboardData(CF_HDROP, mem) != NULL;
    if (!ok) GlobalFree(mem);
    if (effect && !SetClipboardData(RegisterClipboardFormatW(CFSTR_PREFERREDDROPEFFECT), effect)) GlobalFree(effect);
    CloseClipboard();
    return ok;
}

int zant_clipboard_set_image_png(const uint8_t *png, size_t len) {
    if (!png || len == 0) return 0;
    ZantImage *image = zant_image_from_encoded(png, len);
    if (!image) return 0;
    const int width = zant_image_width(image), height = zant_image_height(image);
    const size_t pixel_bytes = (size_t)width * (size_t)height * 4;
    uint8_t *rgba = (uint8_t *)malloc(pixel_bytes);
    const bool read = rgba && zant_image_read_rgba(image, rgba, pixel_bytes);
    zant_image_destroy(image);
    /* "PNG" keeps transparency for apps that read it; CF_DIBV5 is what every
       app pastes (the system derives CF_DIB / CF_BITMAP from it) */
    HGLOBAL png_mem = GlobalAlloc(GMEM_MOVEABLE, len);
    HGLOBAL dib_mem = read ? GlobalAlloc(GMEM_MOVEABLE, sizeof(BITMAPV5HEADER) + pixel_bytes) : NULL;
    if (png_mem) {
        void *p = GlobalLock(png_mem);
        memcpy(p, png, len);
        GlobalUnlock(png_mem);
    }
    if (dib_mem) {
        uint8_t *p = (uint8_t *)GlobalLock(dib_mem);
        BITMAPV5HEADER *h = (BITMAPV5HEADER *)p;
        memset(h, 0, sizeof(*h));
        h->bV5Size = sizeof(*h);
        h->bV5Width = width;
        h->bV5Height = height; /* bottom-up, as most readers expect */
        h->bV5Planes = 1;
        h->bV5BitCount = 32;
        h->bV5Compression = BI_BITFIELDS;
        h->bV5RedMask = 0x00FF0000;
        h->bV5GreenMask = 0x0000FF00;
        h->bV5BlueMask = 0x000000FF;
        h->bV5AlphaMask = 0xFF000000;
        h->bV5CSType = LCS_sRGB;
        h->bV5Intent = LCS_GM_IMAGES;
        uint8_t *dst = p + sizeof(*h);
        for (int y = 0; y < height; y++) {
            const uint8_t *src = rgba + (size_t)(height - 1 - y) * (size_t)width * 4;
            uint8_t *row = dst + (size_t)y * (size_t)width * 4;
            for (int x = 0; x < width; x++) {
                row[x * 4 + 0] = src[x * 4 + 2];
                row[x * 4 + 1] = src[x * 4 + 1];
                row[x * 4 + 2] = src[x * 4 + 0];
                row[x * 4 + 3] = src[x * 4 + 3];
            }
        }
        GlobalUnlock(dib_mem);
    }
    free(rgba);
    int ok = 0;
    if ((png_mem || dib_mem) && zant_open_clipboard(zant_app_window())) {
        EmptyClipboard();
        if (png_mem && SetClipboardData(RegisterClipboardFormatW(L"PNG"), png_mem)) { png_mem = NULL; ok = 1; }
        if (dib_mem && SetClipboardData(CF_DIBV5, dib_mem)) { dib_mem = NULL; ok = 1; }
        CloseClipboard();
    }
    if (png_mem) GlobalFree(png_mem);
    if (dib_mem) GlobalFree(dib_mem);
    return ok;
}

void zant_clipboard_free(void *bytes) {
    free(bytes);
}

uint64_t zant_clipboard_change_count(void) {
    return (uint64_t)GetClipboardSequenceNumber();
}

/* A DWORD clipboard format set to 0 (CanIncludeInClipboardHistory and
   friends); the clipboard is open. */
static bool zant_clipboard_dword_zero(const wchar_t *name) {
    const UINT format = RegisterClipboardFormatW(name);
    if (!format || !IsClipboardFormatAvailable(format)) return false;
    HANDLE data = GetClipboardData(format);
    if (!data || GlobalSize(data) < sizeof(DWORD)) return false;
    const DWORD *value = (const DWORD *)GlobalLock(data);
    if (!value) return false;
    const bool zero = *value == 0;
    GlobalUnlock(data);
    return zero;
}

int zant_clipboard_privacy(void) {
    /* https://learn.microsoft.com/windows/win32/dataxchg/clipboard-formats
       (cloud clipboard and clipboard history formats) */
    const UINT exclude = RegisterClipboardFormatW(L"ExcludeClipboardContentFromMonitorProcessing");
    const UINT ignore = RegisterClipboardFormatW(L"Clipboard Viewer Ignore");
    if ((exclude && IsClipboardFormatAvailable(exclude)) || (ignore && IsClipboardFormatAvailable(ignore)))
        return ZANT_CLIPBOARD_NO_HISTORY | ZANT_CLIPBOARD_NO_SYNC;
    if (!zant_open_clipboard(NULL)) return 0;
    int flags = 0;
    if (zant_clipboard_dword_zero(L"CanIncludeInClipboardHistory")) flags |= ZANT_CLIPBOARD_NO_HISTORY | ZANT_CLIPBOARD_NO_SYNC;
    if (zant_clipboard_dword_zero(L"CanUploadToCloudClipboard")) flags |= ZANT_CLIPBOARD_NO_SYNC;
    CloseClipboard();
    return flags;
}

void zant_clipboard_watch(int enable) {
    static bool watching = false;
    HWND hwnd = zant_app_window();
    if (!hwnd || (enable != 0) == watching) return;
    watching = enable != 0;
    if (watching) AddClipboardFormatListener(hwnd);
    else RemoveClipboardFormatListener(hwnd);
}
