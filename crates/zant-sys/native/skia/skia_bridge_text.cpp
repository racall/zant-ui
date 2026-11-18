// Font initialization, paragraph layout and direct glyph drawing.
#include "skia_bridge_internal.h"
#include "skia_bridge_text_internal.h"

#include "include/core/SkFont.h"
#include "include/core/SkFontMetrics.h"
#include "include/core/SkFontStyle.h"
#include "include/core/SkPaint.h"
#include "modules/skparagraph/include/FontCollection.h"
#include "modules/skparagraph/include/ParagraphBuilder.h"
#include "modules/skparagraph/include/ParagraphStyle.h"
#include "modules/skparagraph/include/TextStyle.h"
#include "modules/skunicode/include/SkUnicode_icu.h"

#include <algorithm>
#include <cmath>
#include <mutex>
#include <new>
#include <utility>
#include <vector>

using namespace skia::textlayout;

namespace {

sk_sp<SkFontMgr> g_font_mgr;
sk_sp<FontCollection> g_font_collection;
sk_sp<SkUnicode> g_unicode;

/* Paragraph building goes through this process-wide font collection / Unicode
   object and Skia's internal caches, which are not safe against concurrent
   shaping. The UI renders on one thread in an app, so this lock is
   uncontended there; the cargo test harness runs tests on several threads,
   and without it Windows aborts with
   STATUS_HEAP_CORRUPTION / access violations in the math and ui tests (macOS
   only survived on allocator luck). */
std::mutex g_text_mutex;

}  // namespace

namespace zant_skia {

void ensure_skia() {
    static std::once_flag once;
    std::call_once(once, [] {
        g_font_mgr = zant_platform::make_font_mgr();
        if (!g_font_mgr) {
            g_font_mgr = SkFontMgr::RefEmpty();
        }
        g_font_collection = sk_make_sp<FontCollection>();
        g_font_collection->setDefaultFontManager(g_font_mgr, zant_platform::default_families());
        g_font_collection->enableFontFallback();
        g_unicode = SkUnicodes::ICU::Make();
    });
}

}  // namespace zant_skia

using zant_skia::ensure_skia;
using zant_skia::make_color;
using zant_skia::utf8_decode_one;

namespace {

/* First installed family for a ZantTextStyle family id (cached). */
sk_sp<SkTypeface> family_typeface(int family, const SkFontStyle &style) {
    if (auto face = zant_platform::bundled_typeface(family)) return face;
    for (const char *const *name = zant_platform::family_names(family); name && *name; ++name) {
        if (auto face = g_font_mgr->matchFamilyStyle(*name, style)) return face;
    }
    return nullptr;
}

TextAlign text_align_from_int(int align) {
    switch (align) {
        case 1:
            return TextAlign::kCenter;
        case 2:
            return TextAlign::kRight;
        default:
            return TextAlign::kLeft;
    }
}

std::unique_ptr<Paragraph> build_paragraph(std::string_view text, const ZantTextStyle &style,
                                           float max_width) {
    ParagraphStyle paragraph_style;
    paragraph_style.setTextAlign(text_align_from_int(style.align));
    if (style.max_lines > 0) {
        paragraph_style.setMaxLines(static_cast<size_t>(style.max_lines));
        // text cut off by max_lines ends in "…"
        paragraph_style.setEllipsis(SkString("\u2026"));
    }

    TextStyle text_style;
    text_style.setColor(style.color);
    text_style.setFontSize(style.font_size > 0.0f ? style.font_size : 14.0f);
    const int weight = std::max(100, std::min(900, style.weight));
    const SkFontStyle sk_style(weight, SkFontStyle::kNormal_Width, SkFontStyle::kUpright_Slant);
    text_style.setFontStyle(sk_style);
    if (style.family >= 1 && style.family <= 4) {
        if (auto face = family_typeface(style.family, sk_style)) text_style.setTypeface(std::move(face));
    }
    if (style.line_height > 0.0f) {
        text_style.setHeight(style.line_height);
        text_style.setHeightOverride(true);
    }

    auto builder = ParagraphBuilder::make(paragraph_style, g_font_collection, g_unicode);
    if (!builder) {
        return nullptr;
    }
    builder->pushStyle(text_style);
    if (!text.empty()) {
        builder->addText(text.data(), text.size());
    }
    auto paragraph = builder->Build();
    if (paragraph) {
        // max_lines text is laid out at the box width so it ends in "…"
        const bool fixed_width = style.wrap || style.max_lines > 0;
        paragraph->layout(fixed_width ? std::max(max_width, 1.0f) : 1.0e9f);
        // a single unwrapped line: centre / right alignment within the huge
        // layout width would move the glyphs far off the box, so lay it out
        // at its own width (the container positions it)
        if (!fixed_width && style.align != 0) {
            paragraph->layout(std::ceil(paragraph->getMaxIntrinsicWidth()) + 1.0f);
        }
    }
    return paragraph;
}

sk_sp<SkTypeface> typeface_for(int family, int weight, bool italic) {
    ensure_skia();
    struct Entry { int family; int weight; bool italic; sk_sp<SkTypeface> face; };
    static std::vector<Entry> cache;
    static std::mutex cache_mutex;
    {
        const std::lock_guard<std::mutex> lock(cache_mutex);
        for (const auto &entry : cache) {
            if (entry.family == family && entry.weight == weight && entry.italic == italic) {
                return entry.face;
            }
        }
    }
    const SkFontStyle style(std::max(100, std::min(900, weight)), SkFontStyle::kNormal_Width,
                            italic ? SkFontStyle::kItalic_Slant : SkFontStyle::kUpright_Slant);
    sk_sp<SkTypeface> face = family_typeface((family >= 0 && family <= 4) ? family : 0, style);
    if (!face) face = family_typeface(0, style);
    if (!face) face = g_font_mgr->legacyMakeTypeface(nullptr, style);
    {
        const std::lock_guard<std::mutex> lock(cache_mutex);
        cache.push_back({family, weight, italic, face});
    }
    return face;
}

struct GlyphRun {
    sk_sp<SkTypeface> face;
    size_t start;
    size_t end;
};

// Splits UTF-8 text into runs that share one typeface, falling back per
// character to any installed font that has the glyph (CJK in \text{}, emoji).
std::vector<GlyphRun> fallback_runs(const char *utf8, size_t length, int family, int weight,
                                    bool italic) {
    std::vector<GlyphRun> runs;
    sk_sp<SkTypeface> primary = typeface_for(family, weight, italic);
    const SkFontStyle style(std::max(100, std::min(900, weight)), SkFontStyle::kNormal_Width,
                            italic ? SkFontStyle::kItalic_Slant : SkFontStyle::kUpright_Slant);
    std::string_view text(utf8, length);
    size_t at = 0;
    while (at < length) {
        uint32_t cp = 0;
        int len = 1;
        if (!utf8_decode_one(text, at, &cp, &len) || len <= 0) {
            cp = 0xFFFD;
            len = 1;
        }
        sk_sp<SkTypeface> face = primary;
        if (primary && primary->unicharToGlyph(static_cast<SkUnichar>(cp)) == 0 && cp > 0x20) {
            sk_sp<SkTypeface> fallback =
                    g_font_mgr->matchFamilyStyleCharacter(nullptr, style, nullptr, 0, static_cast<SkUnichar>(cp));
            if (fallback) face = fallback;
        }
        if (!runs.empty() && runs.back().face == face) {
            runs.back().end = at + len;
        } else {
            runs.push_back({face, at, at + static_cast<size_t>(len)});
        }
        at += static_cast<size_t>(len);
    }
    return runs;
}

}  // namespace

void zant_skia_ensure() {
    ensure_skia();
}

extern "C" {

void zant_skia_init(void) {
    ensure_skia();
}

ZantTextLayout *zant_rich_text_layout_create(const ZantRichSpan *spans, size_t count,
                                             float max_width, float line_height) {
    if (!spans || count == 0) return nullptr;
    const std::lock_guard<std::mutex> text_lock(g_text_mutex);
    std::string joined;
    for (size_t i = 0; i < count; ++i) {
        if (spans[i].placeholder_width > 0.0f) {
            joined.append("\xEF\xBF\xBC");  // U+FFFC, one UTF-16 unit like the placeholder
        } else if (spans[i].text && spans[i].length > 0) {
            joined.append(spans[i].text, spans[i].length);
        }
    }
    ParagraphStyle paragraph_style;
    paragraph_style.setTextAlign(TextAlign::kLeft);
    auto builder = ParagraphBuilder::make(paragraph_style, g_font_collection, g_unicode);
    if (!builder) return nullptr;
    for (size_t i = 0; i < count; ++i) {
        const ZantRichSpan &span = spans[i];
        TextStyle style;
        style.setColor(span.color);
        style.setFontSize(span.font_size > 0.0f ? span.font_size : 14.0f);
        const int weight = std::max(100, std::min(900, span.weight));
        const SkFontStyle sk_style(weight, SkFontStyle::kNormal_Width,
                                   span.italic ? SkFontStyle::kItalic_Slant : SkFontStyle::kUpright_Slant);
        style.setFontStyle(sk_style);
        if (span.family >= 1 && span.family <= 4) {
            if (auto face = family_typeface(span.family, sk_style)) style.setTypeface(std::move(face));
        }
        if (line_height > 0.0f) {
            style.setHeight(line_height);
            style.setHeightOverride(true);
        }
        if (span.background != 0) {
            SkPaint background;
            background.setColor(span.background);
            style.setBackgroundPaint(std::move(background));
        }
        if (span.decoration != 0) {
            int decoration = 0;
            if (span.decoration & 1) decoration |= TextDecoration::kUnderline;
            if (span.decoration & 2) decoration |= TextDecoration::kLineThrough;
            style.setDecoration(static_cast<TextDecoration>(decoration));
            style.setDecorationColor(span.color);
            style.setDecorationThicknessMultiplier(1.0f);
        }
        builder->pushStyle(style);
        if (span.placeholder_width > 0.0f) {
            const float height = std::max(1.0f, span.placeholder_ascent + span.placeholder_descent);
            builder->addPlaceholder(PlaceholderStyle(span.placeholder_width, height,
                                                     PlaceholderAlignment::kBaseline,
                                                     TextBaseline::kAlphabetic,
                                                     span.placeholder_ascent));
        } else if (span.text && span.length > 0) {
            builder->addText(span.text, span.length);
        }
        builder->pop();
    }
    auto paragraph = builder->Build();
    if (!paragraph) return nullptr;
    paragraph->layout(std::max(max_width, 1.0f));
    auto *result = new (std::nothrow) ZantTextLayout();
    if (!result) return nullptr;
    result->text = std::move(joined);
    result->paragraph = std::move(paragraph);
    result->layout_width = max_width;
    result->rich = true;
    return result;
}

ZantTextLayout *zant_text_layout_create(const char *utf8, size_t length,
                                        const ZantTextStyle *style, float max_width) {
    ensure_skia();
    if (!utf8 || !style || style->font_size <= 0.0f) return nullptr;
    auto *layout = new (std::nothrow) ZantTextLayout();
    if (!layout) return nullptr;
    layout->text.assign(utf8, length);
    layout->style = *style;
    {
        const std::lock_guard<std::mutex> lock(g_text_mutex);
        layout->paragraph = build_paragraph(layout->text, *style, max_width);
    }
    if (!layout->paragraph) {
        delete layout;
        return nullptr;
    }
    layout->layout_width = style->wrap ? std::max(max_width, 1.0f) : 1.0e9f;
    return layout;
}

void zant_text_layout_destroy(ZantTextLayout *layout) {
    delete layout;
}

void zant_text_layout_relayout(ZantTextLayout *layout, float max_width) {
    if (!layout || !layout->paragraph) return;
    const std::lock_guard<std::mutex> lock(g_text_mutex);
    Paragraph *p = layout->paragraph.get();
    if (layout->rich) {
        p->layout(std::max(max_width, 1.0f));
        layout->layout_width = max_width;
        return;
    }
    // same rules as build_paragraph
    const ZantTextStyle &style = layout->style;
    const bool fixed_width = style.wrap || style.max_lines > 0;
    p->layout(fixed_width ? std::max(max_width, 1.0f) : 1.0e9f);
    if (!fixed_width && style.align != 0) p->layout(std::ceil(p->getMaxIntrinsicWidth()) + 1.0f);
    layout->layout_width = style.wrap ? std::max(max_width, 1.0f) : 1.0e9f;
}

void zant_canvas_draw_text_layout(ZantCanvas *canvas, ZantTextLayout *layout, float x, float y) {
    if (!canvas || !canvas->canvas || !layout || !layout->paragraph) return;
    layout->paragraph->paint(canvas->canvas, x, y);
}

ZantInkMetrics zant_text_measure_ink(const char *utf8, size_t length, float size, int family,
                                     int weight, int italic) {
    ZantInkMetrics m{};
    if (!utf8 || length == 0 || size <= 0.0f) return m;
    float x = 0.0f;
    float top = 0.0f;
    float bottom = 0.0f;
    bool any_ink = false;
    for (const auto &run : fallback_runs(utf8, length, family, weight, italic != 0)) {
        SkFont font(run.face, size);
        font.setSubpixel(true);
        SkRect bounds;
        const float advance =
                font.measureText(utf8 + run.start, run.end - run.start, SkTextEncoding::kUTF8, &bounds);
        if (!bounds.isEmpty()) {
            top = any_ink ? std::min(top, bounds.top()) : bounds.top();
            bottom = any_ink ? std::max(bottom, bounds.bottom()) : bounds.bottom();
            any_ink = true;
        }
        SkFontMetrics fm;
        font.getMetrics(&fm);
        m.font_ascent = std::max(m.font_ascent, -fm.fAscent);
        m.font_descent = std::max(m.font_descent, fm.fDescent);
        x += advance;
    }
    m.advance = x;
    m.ink_top = any_ink ? top : 0.0f;
    m.ink_bottom = any_ink ? bottom : 0.0f;
    m.ascent = any_ink ? std::max(0.0f, -top) : 0.0f;
    m.descent = any_ink ? std::max(0.0f, bottom) : 0.0f;
    return m;
}

void zant_canvas_draw_simple_text(ZantCanvas *canvas, const char *utf8, size_t length, float x,
                                  float y, float size, int family, int weight, int italic,
                                  uint32_t color, float scale_y) {
    if (!canvas || !canvas->canvas || !utf8 || length == 0 || size <= 0.0f) return;
    SkPaint paint;
    paint.setAntiAlias(true);
    paint.setColor(make_color(color));
    SkCanvas *c = canvas->canvas;
    const bool stretch = scale_y > 0.0f && std::abs(scale_y - 1.0f) > 0.001f;
    if (stretch) {
        c->save();
        c->translate(0.0f, y);
        c->scale(1.0f, scale_y);
        c->translate(0.0f, -y);
    }
    float pen = x;
    for (const auto &run : fallback_runs(utf8, length, family, weight, italic != 0)) {
        SkFont font(run.face, size);
        font.setSubpixel(true);
        font.setEdging(SkFont::Edging::kSubpixelAntiAlias);
        c->drawSimpleText(utf8 + run.start, run.end - run.start, SkTextEncoding::kUTF8, pen, y, font, paint);
        pen += font.measureText(utf8 + run.start, run.end - run.start, SkTextEncoding::kUTF8);
    }
    if (stretch) c->restore();
}

}  // extern "C"
