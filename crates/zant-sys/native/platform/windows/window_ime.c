#include "window_internal.h"

#include <imm.h>
#include <math.h>
#include <stdlib.h>
#include <string.h>

/* ---------- IME (IMM32) ---------- */

void zant_ime_position(ZantWindow *w) {
    HIMC imc = ImmGetContext(w->hwnd);
    if (!imc) return;
    float x = zant_logical_width(w) * 0.5f, y = zant_logical_height(w) * 0.5f, cw = 1, ch = 18;
    if (w->host.ime_rect) {
        if (!w->host.ime_rect(w->host.userdata, &x, &y, &cw, &ch)) {
            x = zant_logical_width(w) * 0.5f;
            y = zant_logical_height(w) * 0.5f;
        }
    }
    const LONG px = (LONG)lroundf(x * w->scale);
    const LONG py = (LONG)lroundf(y * w->scale);
    const LONG pw = (LONG)lroundf(fmaxf(cw, 1) * w->scale);
    const LONG ph = (LONG)lroundf(fmaxf(ch, 1) * w->scale);
    COMPOSITIONFORM composition;
    memset(&composition, 0, sizeof(composition));
    composition.dwStyle = CFS_POINT;
    composition.ptCurrentPos.x = px;
    composition.ptCurrentPos.y = py;
    ImmSetCompositionWindow(imc, &composition);
    CANDIDATEFORM candidate;
    memset(&candidate, 0, sizeof(candidate));
    candidate.dwIndex = 0;
    candidate.dwStyle = CFS_EXCLUDE;
    candidate.ptCurrentPos.x = px;
    candidate.ptCurrentPos.y = py + ph;
    candidate.rcArea.left = px;
    candidate.rcArea.top = py;
    candidate.rcArea.right = px + pw;
    candidate.rcArea.bottom = py + ph;
    ImmSetCandidateWindow(imc, &candidate);
    ImmReleaseContext(w->hwnd, imc);
}

void zant_send_text(ZantWindow *w, int type, const wchar_t *text, int units, size_t sel_start,
                           size_t sel_end) {
    int len = 0;
    char *utf8 = zant_narrow(text ? text : L"", units, &len);
    if (!utf8) return;
    ZantEvent e = zant_event(w, type);
    e.text = utf8;
    e.text_len = (size_t)len;
    e.selection_start = sel_start;
    e.selection_end = sel_end;
    if (w->host.event) w->host.event(w->host.userdata, &e);
    free(utf8);
    zant_request(w);
}

static wchar_t *zant_ime_string(HIMC imc, DWORD kind, int *units) {
    const LONG bytes = ImmGetCompositionStringW(imc, kind, NULL, 0);
    *units = 0;
    if (bytes <= 0) return NULL;
    wchar_t *buf = (wchar_t *)malloc((size_t)bytes + sizeof(wchar_t));
    if (!buf) return NULL;
    ImmGetCompositionStringW(imc, kind, buf, (DWORD)bytes);
    *units = (int)(bytes / (LONG)sizeof(wchar_t));
    buf[*units] = 0;
    return buf;
}

void zant_ime_composition(ZantWindow *w, LPARAM flags) {
    HIMC imc = ImmGetContext(w->hwnd);
    if (!imc) return;
    if (flags & GCS_RESULTSTR) {
        int units = 0;
        wchar_t *result = zant_ime_string(imc, GCS_RESULTSTR, &units);
        if (result && units > 0) zant_send_text(w, ZANT_EVENT_TEXT_INPUT, result, units, 0, 0);
        free(result);
    }
    if (flags & GCS_COMPSTR) {
        int units = 0;
        wchar_t *marked = zant_ime_string(imc, GCS_COMPSTR, &units);
        LONG cursor = ImmGetCompositionStringW(imc, GCS_CURSORPOS, NULL, 0);
        if (cursor < 0 || cursor > units) cursor = units;
        zant_send_text(w, ZANT_EVENT_COMPOSITION, marked, units, (size_t)cursor, (size_t)cursor);
        free(marked);
    } else if (!(flags & GCS_RESULTSTR)) {
        /* composition cancelled */
        zant_send_text(w, ZANT_EVENT_COMPOSITION, L"", 0, 0, 0);
    }
    ImmReleaseContext(w->hwnd, imc);
    zant_ime_position(w);
}
