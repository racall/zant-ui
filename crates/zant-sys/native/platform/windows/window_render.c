#include "window_internal.h"

#include <dwmapi.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* animation frames of a hidden window: nothing is on screen, so slowly */
#define ZANT_HIDDEN_FRAME_MS 250

/* ---------- rendering ---------- */

/* Monotonic microseconds (QueryPerformanceCounter). */
long long zant_now_us(void) {
    static LARGE_INTEGER frequency;
    LARGE_INTEGER now;
    if (!frequency.QuadPart) QueryPerformanceFrequency(&frequency);
    QueryPerformanceCounter(&now);
    return (long long)(now.QuadPart / frequency.QuadPart * 1000000 +
                       now.QuadPart % frequency.QuadPart * 1000000 / frequency.QuadPart);
}

/* All trace hooks run on the window thread. */
static bool zant_frame_trace_enabled(void) {
    static int enabled = -1;
    if (enabled < 0) {
        const char *value = getenv("ZANT_FRAME_TRACE");
        enabled = value && strcmp(value, "1") == 0;
    }
    return enabled != 0;
}

void zant_frame_trace_enter(ZantWindow *w) {
    if (!zant_frame_trace_enabled()) return;
    memset(&w->trace, 0, sizeof(w->trace));
    w->trace.active = true;
    w->trace.paint_source = ZANT_FRAME_PAINT;
    w->trace.start_us = zant_now_us();
}

void zant_frame_trace_exit(ZantWindow *w) {
    ZantFrameTrace *t = &w->trace;
    if (!t->active) return;
    t->active = false;
    fprintf(stderr, "[frame-trace] ms=%lld size_msgs=%lld move_msgs=%lld wakes=%lld "
            "requests=%lld/%lld timer=%lld/%lld frames=size:%lld paint:%lld timer:%lld "
            "hidden:%lld other:%lld new_size=%lld same_size=%lld begin_us=%lld/%lld render_us=%lld/%lld\n",
            (zant_now_us() - t->start_us) / 1000, t->size_msgs, t->move_msgs, t->wakes,
            t->requests_in_draw, t->requests_outside_draw, t->timer_fired, t->timer_skipped,
            t->frames[ZANT_FRAME_SIZE], t->frames[ZANT_FRAME_PAINT], t->frames[ZANT_FRAME_MODAL_TIMER],
            t->frames[ZANT_FRAME_HIDDEN], t->frames[ZANT_FRAME_OTHER], t->new_size, t->same_size,
            t->begin.samples ? t->begin.total_us / t->begin.samples : 0, t->begin.max_us,
            t->render.samples ? t->render.total_us / t->render.samples : 0, t->render.max_us);
    fflush(stderr);
}

static void zant_record_timing(ZantFrameTiming *timing, long long us) {
    ++timing->samples;
    timing->total_us += us;
    if (us > timing->max_us) timing->max_us = us;
}

void zant_update_frame(ZantWindow *w, ZantFrameSource source) {
    InvalidateRect(w->hwnd, NULL, FALSE);
    if (w->trace.active) {
        const ZantFrameSource previous = w->trace.paint_source;
        w->trace.paint_source = source;
        UpdateWindow(w->hwnd);
        w->trace.paint_source = previous;
    } else {
        UpdateWindow(w->hwnd);
    }
}

void zant_modal_tick(ZantWindow *w) {
    /* As long as WM_SIZE advances the animation, a timer frame would take
       the next resize frame's vsync. Keep animating when the size settles. */
    const long long now = zant_now_us();
    if (w->animating && now - w->last_resize_us >= 50000 && now - w->last_frame_us >= 8000) {
        if (w->trace.active) ++w->trace.timer_fired;
        zant_update_frame(w, ZANT_FRAME_MODAL_TIMER);
    } else if (w->trace.active) {
        ++w->trace.timer_skipped;
    }
}

/* Hidden or minimized: Windows sends it no WM_PAINT. */
bool zant_backgrounded(ZantWindow *w) {
    return !IsWindowVisible(w->hwnd) || IsIconic(w->hwnd);
}

void zant_request(ZantWindow *w) {
    if (!w) return;
    if (w->trace.active) {
        if (w->in_draw) ++w->trace.requests_in_draw;
        else ++w->trace.requests_outside_draw;
    }
    if (!w->hwnd || w->closing) return;
    if (!zant_backgrounded(w)) {
        InvalidateRect(w->hwnd, NULL, FALSE);
    } else if (!w->hidden_frame_pending) {
        w->hidden_frame_pending = true;
        if (!PostMessageW(w->hwnd, ZANT_WM_HIDDEN_FRAME, 0, 0)) w->hidden_frame_pending = false;
    }
}

static void zant_present_raster(ZantWindow *w, HDC dc) {
    int width = 0, height = 0, stride = 0;
    void *pixels = zant_renderer_pixels(w->renderer, &width, &height, &stride);
    if (!pixels || width <= 0 || height <= 0 || stride <= 0) return;
    BITMAPINFO bmi;
    memset(&bmi, 0, sizeof(bmi));
    bmi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    bmi.bmiHeader.biWidth = stride / 4;
    bmi.bmiHeader.biHeight = -height; /* top-down */
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB; /* BGRA: Skia's native order on Windows */
    SetDIBitsToDevice(dc, 0, 0, (DWORD)width, (DWORD)height, 0, 0, 0, (UINT)height, pixels, &bmi,
                      DIB_RGB_COLORS);
}

static void zant_schedule_next(ZantWindow *w) {
    if (!w->host.next_frame_delay || w->closing) return;
    const int delay = w->host.next_frame_delay(w->host.userdata);
    w->animating = delay > 0 && delay <= 16;
    KillTimer(w->hwnd, ZANT_TIMER_REDRAW);
    if (w->animating && zant_backgrounded(w)) {
        SetTimer(w->hwnd, ZANT_TIMER_REDRAW, ZANT_HIDDEN_FRAME_MS, NULL);
    } else if (w->animating && w->in_size_move) {
        /* live resize / move: WM_SIZE draws each new size and ZANT_TIMER_MODAL
           the animation frames between them. Requesting the next frame here
           would draw one at the old size after every resize frame, each
           waiting for a vsync: half the new sizes drawn, two vsyncs late. */
    } else if (w->animating) {
        zant_request(w);
    } else if (delay > 0) {
        SetTimer(w->hwnd, ZANT_TIMER_REDRAW, (UINT)delay, NULL);
    } else if (delay == 0) {
        zant_request(w);
    }
}

static void zant_record_draw(ZantWindow *w, ZantFrameSource source) {
    if (!zant_frame_trace_enabled()) return;
    if (w->trace.active) {
        if (source == ZANT_FRAME_PAINT) source = w->trace.paint_source;
        ++w->trace.frames[source];
        if (w->width_px == w->last_draw_width_px && w->height_px == w->last_draw_height_px) {
            ++w->trace.same_size;
        } else {
            ++w->trace.new_size;
        }
    }
    /* The first frame of a move compares against the last frame before entry. */
    w->last_draw_width_px = w->width_px;
    w->last_draw_height_px = w->height_px;
}

void zant_render(ZantWindow *w, HDC dc, ZantFrameSource source) {
    if (!w || w->closing || w->in_draw || !w->host.draw || !w->renderer) return;
    const long long render_start = w->trace.active ? zant_now_us() : 0;
    const bool background = zant_backgrounded(w);
    ZantRenderer *renderer = w->renderer;
    if (background) {
        /* one pixel: the frame builds and lays out at the window's size, and
           its paint is clipped away (a hidden full-screen overlay costs
           nothing to keep) */
        if (!w->background_renderer) w->background_renderer = zant_renderer_create_raster(1, 1);
        if (!w->background_renderer) {
            if (render_start) zant_record_timing(&w->trace.render, zant_now_us() - render_start);
            return;
        }
        renderer = w->background_renderer;
    }
    w->in_draw = true;
    /* a minimized window has no client area: it keeps its restored size */
    if (!IsIconic(w->hwnd)) zant_update_client_size(w);
    if (!background) zant_renderer_resize(renderer, w->width_px, w->height_px);
    const long long begin_start = w->trace.active ? zant_now_us() : 0;
    ZantCanvas *canvas = zant_renderer_begin(renderer, w->scale);
    if (begin_start) zant_record_timing(&w->trace.begin, zant_now_us() - begin_start);
    if (canvas) {
        zant_record_draw(w, source);
        w->host.draw(w->host.userdata, canvas, zant_logical_width(w), zant_logical_height(w), w->scale);
    }
    zant_renderer_end(renderer);
    const bool gpu = zant_renderer_is_gpu(w->renderer);
    if (!gpu && !background) {
        HDC target = dc ? dc : GetDC(w->hwnd);
        zant_present_raster(w, target);
        if (!dc) ReleaseDC(w->hwnd, target);
    }
    w->in_draw = false;
    w->last_frame_us = zant_now_us();
    zant_schedule_next(w);
    /* raster animation frames wait for the next composition (vsync) */
    if (w->animating && !gpu && !w->in_size_move && !background) DwmFlush();
    /* A host callback may have run a nested message loop. */
    if (w->trace.active && render_start >= w->trace.start_us) {
        zant_record_timing(&w->trace.render, zant_now_us() - render_start);
    }
}

/* ---------- snapshot ---------- */

void zant_take_snapshot(ZantWindow *w) {
    if (!w->renderer || !w->snapshot_path[0]) return;
    if (zant_renderer_is_gpu(w->renderer)) {
        zant_renderer_capture_next_png(w->renderer, w->snapshot_path);
        zant_render(w, NULL, ZANT_FRAME_OTHER);
        fprintf(stderr, "[zant] snapshot: %s\n", w->snapshot_path);
    } else {
        zant_render(w, NULL, ZANT_FRAME_OTHER);
        if (zant_renderer_write_png(w->renderer, w->snapshot_path)) {
            fprintf(stderr, "[zant] snapshot: %s\n", w->snapshot_path);
        } else {
            fprintf(stderr, "[zant] snapshot failed: %s\n", w->snapshot_path);
        }
    }
    fflush(stderr);
    const char *exit_flag = getenv("ZANT_SNAPSHOT_EXIT");
    if (exit_flag && exit_flag[0] == '1') SetTimer(w->hwnd, ZANT_TIMER_EXIT, 400, NULL);
}

void zant_window_request_redraw(ZantWindow *window, int delay_ms) {
    if (!window || window->closing) return;
    if (delay_ms <= 0) {
        zant_request(window);
    } else if (!window->animating) {
        SetTimer(window->hwnd, ZANT_TIMER_REDRAW, (UINT)delay_ms, NULL);
    }
}

const char *zant_window_renderer_name(ZantWindow *window) {
    return window ? window->backend_name : "none";
}

void zant_window_schedule_snapshot(ZantWindow *window, const char *path, int delay_ms) {
    if (!window || !path) return;
    snprintf(window->snapshot_path, sizeof(window->snapshot_path), "%s", path);
    SetTimer(window->hwnd, ZANT_TIMER_SNAPSHOT, (UINT)(delay_ms > 0 ? delay_ms : 1), NULL);
}
