// Canvas state, clipping, shapes and paths.
#include "skia_bridge_internal.h"

#include "include/core/SkBlurTypes.h"
#include "include/core/SkMaskFilter.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkRRect.h"

#include <algorithm>

namespace zant_skia {

SkColor make_color(uint32_t argb) {
    return static_cast<SkColor>(argb);
}

SkRRect make_rrect(float x, float y, float w, float h, float radius) {
    const float r = std::max(0.0f, std::min(radius, std::min(w, h) * 0.5f));
    return SkRRect::MakeRectXY(SkRect::MakeXYWH(x, y, w, h), r, r);
}

}  // namespace zant_skia

using zant_skia::make_color;
using zant_skia::make_rrect;

namespace {

SkPaint fill_paint(uint32_t argb) {
    SkPaint paint;
    paint.setAntiAlias(true);
    paint.setColor(make_color(argb));
    paint.setStyle(SkPaint::kFill_Style);
    return paint;
}

SkPaint stroke_paint(uint32_t argb, float width) {
    SkPaint paint;
    paint.setAntiAlias(true);
    paint.setColor(make_color(argb));
    paint.setStyle(SkPaint::kStroke_Style);
    paint.setStrokeWidth(width);
    paint.setStrokeJoin(SkPaint::kRound_Join);
    paint.setStrokeCap(SkPaint::kRound_Cap);
    return paint;
}

}  // namespace

extern "C" {

void zant_canvas_save(ZantCanvas *canvas) {
    if (canvas && canvas->canvas) canvas->canvas->save();
}

void zant_canvas_restore(ZantCanvas *canvas) {
    if (canvas && canvas->canvas) canvas->canvas->restore();
}

void zant_canvas_translate(ZantCanvas *canvas, float x, float y) {
    if (canvas && canvas->canvas) canvas->canvas->translate(x, y);
}

void zant_canvas_scale(ZantCanvas *canvas, float sx, float sy) {
    if (canvas && canvas->canvas) canvas->canvas->scale(sx, sy);
}

void zant_canvas_save_layer_alpha(ZantCanvas *canvas, float x, float y, float width, float height, float alpha) {
    if (!canvas || !canvas->canvas) return;
    SkRect bounds = SkRect::MakeXYWH(x, y, width, height);
    canvas->canvas->saveLayerAlphaf(&bounds, alpha);
}

void zant_canvas_clip_rect(ZantCanvas *canvas, float x, float y, float width, float height) {
    if (canvas && canvas->canvas) {
        canvas->canvas->clipRect(SkRect::MakeXYWH(x, y, width, height), SkClipOp::kIntersect, true);
    }
}

void zant_canvas_clear(ZantCanvas *canvas, uint32_t color) {
    if (canvas && canvas->canvas) canvas->canvas->clear(make_color(color));
}

void zant_canvas_fill_rect(ZantCanvas *canvas, float x, float y, float width, float height,
                           uint32_t color) {
    if (!canvas || !canvas->canvas) return;
    canvas->canvas->drawRect(SkRect::MakeXYWH(x, y, width, height), fill_paint(color));
}

void zant_canvas_shadow_round_rect(ZantCanvas *canvas, float x, float y, float width, float height,
                                   float radius, uint32_t color, float blur) {
    if (!canvas || !canvas->canvas || blur <= 0.0f) return;
    SkPaint paint;
    paint.setAntiAlias(true);
    paint.setColor(color);
    paint.setMaskFilter(SkMaskFilter::MakeBlur(kNormal_SkBlurStyle, blur));
    canvas->canvas->drawRRect(make_rrect(x, y, width, height, radius), paint);
}

void zant_canvas_fill_round_rect(ZantCanvas *canvas, float x, float y, float width, float height,
                                 float radius, uint32_t color) {
    if (!canvas || !canvas->canvas) return;
    canvas->canvas->drawRRect(make_rrect(x, y, width, height, radius), fill_paint(color));
}

void zant_canvas_stroke_round_rect(ZantCanvas *canvas, float x, float y, float width, float height,
                                   float radius, uint32_t color, float stroke_width) {
    if (!canvas || !canvas->canvas) return;
    canvas->canvas->drawRRect(make_rrect(x, y, width, height, radius),
                              stroke_paint(color, std::max(0.5f, stroke_width)));
}

void zant_canvas_draw_line(ZantCanvas *canvas, float x0, float y0, float x1, float y1,
                           uint32_t color, float stroke_width) {
    if (!canvas || !canvas->canvas) return;
    canvas->canvas->drawLine(x0, y0, x1, y1, stroke_paint(color, std::max(0.5f, stroke_width)));
}

void zant_canvas_fill_path(ZantCanvas *canvas, const float *xy, int count, uint32_t color) {
    if (!canvas || !canvas->canvas || !xy || count < 3) return;
    SkPathBuilder builder;
    builder.moveTo(xy[0], xy[1]);
    for (int i = 1; i < count; ++i) builder.lineTo(xy[i * 2], xy[i * 2 + 1]);
    builder.close();
    SkPaint paint;
    paint.setAntiAlias(true);
    paint.setColor(make_color(color));
    canvas->canvas->drawPath(builder.detach(), paint);
}

void zant_canvas_stroke_path(ZantCanvas *canvas, const float *xy, int count, uint32_t color,
                             float stroke_width, int closed) {
    if (!canvas || !canvas->canvas || !xy || count < 2) return;
    SkPathBuilder builder;
    builder.moveTo(xy[0], xy[1]);
    for (int i = 1; i < count; ++i) builder.lineTo(xy[i * 2], xy[i * 2 + 1]);
    if (closed) builder.close();
    SkPaint paint;
    paint.setAntiAlias(true);
    paint.setStyle(SkPaint::kStroke_Style);
    paint.setStrokeWidth(stroke_width);
    paint.setStrokeJoin(SkPaint::kRound_Join);
    paint.setStrokeCap(SkPaint::kRound_Cap);
    paint.setColor(make_color(color));
    canvas->canvas->drawPath(builder.detach(), paint);
}

}  // extern "C"
