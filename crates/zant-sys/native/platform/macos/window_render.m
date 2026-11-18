#import "window_internal.h"
#import <CoreGraphics/CoreGraphics.h>
#import <QuartzCore/QuartzCore.h>
#import <dispatch/dispatch.h>

#include <math.h>
#include <stdio.h>
#include <stdlib.h>

static void zant_link_set_active(ZantWindow *window, bool active) {
    if (active && !window->display_link && window->view &&
        [window->view respondsToSelector:@selector(displayLinkWithTarget:selector:)]) {
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunguarded-availability-new"
        CADisplayLink *link = [window->view displayLinkWithTarget:window->view
                                                         selector:@selector(zantDisplayLinkFired:)];
        [link addToRunLoop:[NSRunLoop mainRunLoop] forMode:NSRunLoopCommonModes];
        link.paused = YES;
        window->display_link = link;
#pragma clang diagnostic pop
    }
    if (!window->display_link) {
        window->link_active = false;
        return;
    }
    if (window->link_active == active) return;
    window->link_active = active;
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunguarded-availability-new"
    [(CADisplayLink *)window->display_link setPaused:!active];
#pragma clang diagnostic pop
}

void zant_link_invalidate(ZantWindow *window) {
    if (window->display_link) {
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunguarded-availability-new"
        [(CADisplayLink *)window->display_link invalidate];
#pragma clang diagnostic pop
    }
    window->display_link = nil;
    window->link_active = false;
}

/* Animation frames of a hidden window: nothing is on screen, so slowly. */
#define ZANT_HIDDEN_FRAME_MS 250

static void zant_schedule_redraw(ZantWindow *window, int delay_ms) {
    if (!window || window->closing || !window->view) return;
    const bool visible = window->window.isVisible;
    /* the running display link renders on the next vsync anyway */
    if (delay_ms <= 0 && window->link_active && visible) return;
    /* a hidden view gets no display link callbacks */
    if (!visible) zant_link_set_active(window, false);
    [window->redraw_timer invalidate];
    window->redraw_timer = nil;
    window->redraw_generation++;
    if (delay_ms <= 0 && visible) {
        if (window->renderer && zant_renderer_is_gpu(window->renderer)) {
            if (!window->gpu_render_pending) {
                window->gpu_render_pending = true;
                dispatch_async(dispatch_get_main_queue(), ^{
                    window->gpu_render_pending = false;
                    if (!window->closing) zant_render_window(window);
                });
            }
        } else {
            [window->view setNeedsDisplay:YES];
        }
        return;
    }
    /* AppKit never asks a hidden view to draw, yet the frame still has to run
       (first build, effects, tray) while the app sits in the tray: the
       one-shot timer below renders it (zant_window_destroy cancels it) */
    if (delay_ms <= 0) delay_ms = 1;
    const unsigned long generation = window->redraw_generation;
    NSTimeInterval interval = (NSTimeInterval)delay_ms / 1000.0;
    if (interval < 0.001) interval = 0.001;
    window->redraw_timer = [NSTimer scheduledTimerWithTimeInterval:interval
                                                     repeats:NO
                                                       block:^(NSTimer *timer) {
        (void)timer;
        if (generation != window->redraw_generation) return;
        window->redraw_timer = nil;
        if (!window->closing && window->view) {
            if ((window->renderer && zant_renderer_is_gpu(window->renderer)) || !window->window.isVisible) {
                zant_render_window(window);
            } else {
                [window->view setNeedsDisplay:YES];
            }
        }
    }];
}

static CGImageRef zant_create_cgimage(ZantWindow *window, size_t *out_width,
                                     size_t *out_height) {
    int width = 0;
    int height = 0;
    int stride = 0;
    void *pixels = zant_renderer_pixels(window->renderer, &width, &height, &stride);
    if (!pixels || width <= 0 || height <= 0 || stride <= 0) return NULL;
    const size_t byte_count = (size_t)stride * (size_t)height;
    CGDataProviderRef provider = CGDataProviderCreateWithData(NULL, pixels, byte_count, NULL);
    if (!provider) return NULL;
    CGColorSpaceRef color_space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    CGBitmapInfo bitmap_info = kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big;
    CGImageRef image = CGImageCreate((size_t)width, (size_t)height, 8, 32, (size_t)stride,
                                     color_space, bitmap_info, provider, NULL, false,
                                     kCGRenderingIntentDefault);
    CGColorSpaceRelease(color_space);
    CGDataProviderRelease(provider);
    if (out_width) *out_width = (size_t)width;
    if (out_height) *out_height = (size_t)height;
    return image;
}

void zant_render_window(ZantWindow *window) {
    if (!window || !window->host.draw || window->in_draw) return;
    window->in_draw = true;

    const CGFloat scale = window->view.window.backingScaleFactor;
    const CGFloat logical_width = NSWidth(window->view.bounds);
    const CGFloat logical_height = NSHeight(window->view.bounds);
    int pixel_width = (int)lround(logical_width * scale);
    int pixel_height = (int)lround(logical_height * scale);
    if (pixel_width < 1) pixel_width = 1;
    if (pixel_height < 1) pixel_height = 1;

    if (!window->renderer) {
        window->renderer = zant_renderer_create_raster(pixel_width, pixel_height);
        if (window->renderer) {
            snprintf(window->backend_name, sizeof(window->backend_name), "%s",
                     zant_renderer_backend_name(window->renderer));
        }
    } else if (pixel_width != window->width_px || pixel_height != window->height_px) {
        zant_renderer_resize(window->renderer, pixel_width, pixel_height);
    }
    window->width_px = pixel_width;
    window->height_px = pixel_height;

    if (window->renderer) {
        ZantCanvas *canvas = zant_renderer_begin(window->renderer, (float)scale);
        if (canvas) {
            window->host.draw(window->host.userdata, canvas, (float)logical_width,
                              (float)logical_height, (float)scale);
        }
        zant_renderer_end(window->renderer);

        if (!zant_renderer_is_gpu(window->renderer)) {
            size_t image_width = 0;
            size_t image_height = 0;
            /* a hidden window's frame builds and paints but shows nothing
               (a raster window is only presented from drawRect:) */
            CGImageRef image = window->window.isVisible && NSGraphicsContext.currentContext
                                   ? zant_create_cgimage(window, &image_width, &image_height)
                                   : NULL;
            if (image) {
                NSImage *ns_image = [[NSImage alloc] initWithCGImage:image
                                                                size:NSMakeSize(logical_width,
                                                                                logical_height)];
                [ns_image drawInRect:window->view.bounds
                            fromRect:NSZeroRect
                           operation:NSCompositingOperationCopy
                            fraction:1.0
                      respectFlipped:YES
                               hints:nil];
                CGImageRelease(image);
            }
        }
    }

    window->in_draw = false;
    if (window->host.next_frame_delay) {
        const int delay = window->host.next_frame_delay(window->host.userdata);
        /* animations run on the display link; slower work (caret blink,
           tooltip delay) on a one-shot timer */
        const bool animating = delay > 0 && delay <= 16 && !window->closing;
        const bool visible = window->window.isVisible;
        zant_link_set_active(window, animating && visible);
        if (window->link_active) {
            [window->redraw_timer invalidate];
            window->redraw_timer = nil;
            window->redraw_generation++;
        } else if (animating && !visible) {
            zant_schedule_redraw(window, ZANT_HIDDEN_FRAME_MS);
        } else if (delay >= 0) {
            zant_schedule_redraw(window, delay);
        }
    }
}

void zant_request_view_redraw(ZantView *view) {
    if (!view || !view.macWindow) return;
    zant_schedule_redraw(view.macWindow, 0);
}

void zant_window_request_redraw(ZantWindow *window, int delay_ms) {
    zant_schedule_redraw(window, delay_ms);
}

void zant_window_post_wake(ZantWindow *window) {
    if (!window) return;
    /* coalesce: one main-queue block per burst of wakes */
    if (atomic_exchange(&window->wake_pending, true)) return;
    dispatch_async(dispatch_get_main_queue(), ^{
        atomic_store(&window->wake_pending, false);
        if (window->closing) return;
        if (window->host.wake) window->host.wake(window->host.userdata);
        zant_schedule_redraw(window, 0);
    });
}

const char *zant_window_renderer_name(ZantWindow *window) {
    if (!window) return "none";
    return window->backend_name;
}

void zant_window_schedule_snapshot(ZantWindow *window, const char *path, int delay_ms) {
    if (!window || !path) return;
    NSString *target = [NSString stringWithUTF8String:path];
    NSTimeInterval interval = (NSTimeInterval)(delay_ms > 0 ? delay_ms : 0) / 1000.0;
    [NSTimer scheduledTimerWithTimeInterval:interval
                                     repeats:NO
                                       block:^(NSTimer *timer) {
        (void)timer;
        if (!window->view || !window->renderer) return;
        if (zant_renderer_is_gpu(window->renderer)) {
            zant_renderer_capture_next_png(window->renderer, target.UTF8String);
            zant_render_window(window);
            fprintf(stderr, "[zant] snapshot: %s\n", target.UTF8String);
        } else {
            [window->view display];
            zant_render_window(window);
            if (zant_renderer_write_png(window->renderer, target.UTF8String)) {
                fprintf(stderr, "[zant] snapshot: %s\n", target.UTF8String);
            } else {
                fprintf(stderr, "[zant] snapshot failed: %s\n", target.UTF8String);
            }
        }
        // ZANT_SNAPSHOT_EXIT=1: quit once the frame is on disk (automation)
        const char *exit_flag = getenv("ZANT_SNAPSHOT_EXIT");
        if (exit_flag && exit_flag[0] == '1') {
            dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(400 * NSEC_PER_MSEC)),
                           dispatch_get_main_queue(), ^{
                [NSApp terminate:nil];
            });
        }
    }];
}
