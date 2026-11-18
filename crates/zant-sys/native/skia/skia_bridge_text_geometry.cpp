// Text metrics, caret and hit testing, and range and placeholder rectangles.
#include "skia_bridge_text_internal.h"

#include <algorithm>

using namespace skia::textlayout;

namespace {

/* Number of UTF-8 bytes in one code point; invalid bytes consume one byte. */
int utf8_unit_length(unsigned char c) {
    if ((c & 0x80) == 0) return 1;
    if ((c & 0xE0) == 0xC0) return 2;
    if ((c & 0xF0) == 0xE0) return 3;
    if ((c & 0xF8) == 0xF0) return 4;
    return 1;
}

}  // namespace

namespace zant_skia {

bool utf8_decode_one(std::string_view text, size_t at, uint32_t *out_cp, int *out_len) {
    if (at >= text.size()) return false;
    const unsigned char c = static_cast<unsigned char>(text[at]);
    const int len = std::min<int>(utf8_unit_length(c), static_cast<int>(text.size() - at));
    if (len == 1) {
        *out_cp = c;
        *out_len = 1;
        return true;
    }
    uint32_t cp = c & (0xFF >> (len + 1));
    for (int i = 1; i < len; ++i) {
        const unsigned char cc = static_cast<unsigned char>(text[at + i]);
        if ((cc & 0xC0) != 0x80) {
            *out_cp = c;
            *out_len = 1;
            return true;
        }
        cp = (cp << 6) | (cc & 0x3F);
    }
    *out_cp = cp;
    *out_len = len;
    return true;
}

}  // namespace zant_skia

using zant_skia::utf8_decode_one;

namespace {

size_t utf8_to_utf16_offset(std::string_view text, size_t byte_offset) {
    const size_t limit = std::min(byte_offset, text.size());
    size_t at = 0;
    size_t units = 0;
    while (at < limit) {
        uint32_t cp = 0;
        int len = 1;
        utf8_decode_one(text, at, &cp, &len);
        if (at + static_cast<size_t>(len) > limit) {
            break;
        }
        units += (cp > 0xFFFF) ? 2 : 1;
        at += static_cast<size_t>(len);
    }
    return units;
}

size_t utf16_to_utf8_offset(std::string_view text, size_t unit_offset) {
    size_t at = 0;
    size_t units = 0;
    while (at < text.size() && units < unit_offset) {
        uint32_t cp = 0;
        int len = 1;
        utf8_decode_one(text, at, &cp, &len);
        const size_t add = (cp > 0xFFFF) ? 2 : 1;
        if (units + add > unit_offset) {
            break;
        }
        units += add;
        at += static_cast<size_t>(len);
    }
    return at;
}

}  // namespace

extern "C" {

ZantTextMetrics zant_text_layout_metrics(ZantTextLayout *layout) {
    ZantTextMetrics metrics{};
    if (!layout || !layout->paragraph) return metrics;
    metrics.width = layout->paragraph->getLongestLine();
    metrics.height = layout->paragraph->getHeight();
    metrics.max_intrinsic_width = layout->paragraph->getMaxIntrinsicWidth();
    metrics.longest_line = layout->paragraph->getLongestLine();
    metrics.first_baseline = layout->paragraph->getAlphabeticBaseline();
    metrics.line_count = static_cast<int>(layout->paragraph->lineNumber());
    return metrics;
}

bool zant_text_layout_caret_rect(ZantTextLayout *layout, size_t byte_offset, ZantRect *out_rect) {
    if (!layout || !layout->paragraph || !out_rect) return false;
    const size_t utf16_len = utf8_to_utf16_offset(layout->text, layout->text.size());
    const size_t offset = utf8_to_utf16_offset(layout->text, byte_offset);
    auto try_range = [&](size_t start, size_t end, bool take_right) -> bool {
        if (start > end || end > utf16_len) return false;
        auto rects = layout->paragraph->getRectsForRange(
                static_cast<unsigned>(start), static_cast<unsigned>(end), RectHeightStyle::kMax,
                RectWidthStyle::kTight);
        if (rects.empty()) return false;
        const SkRect &rect = rects.front().rect;
        out_rect->x = take_right ? rect.right() : rect.left();
        out_rect->y = rect.top();
        out_rect->width = 1.0f;
        out_rect->height = rect.height();
        return true;
    };

    if (layout->text.empty()) {
        const auto metrics = zant_text_layout_metrics(layout);
        out_rect->x = 0.0f;
        out_rect->y = 0.0f;
        out_rect->width = 1.0f;
        out_rect->height = metrics.height > 0.0f ? metrics.height : layout->style.font_size * 1.2f;
        return true;
    }
    if (offset >= utf16_len) {
        if (try_range(utf16_len - 1, utf16_len, true)) return true;
    } else {
        if (try_range(offset, offset + 1, false)) return true;
        if (try_range(offset, offset, false)) return true;
    }
    const auto metrics = zant_text_layout_metrics(layout);
    out_rect->x = 0.0f;
    out_rect->y = 0.0f;
    out_rect->width = 1.0f;
    out_rect->height = metrics.height > 0.0f ? metrics.height : layout->style.font_size * 1.2f;
    return true;
}

size_t zant_text_layout_index_at(ZantTextLayout *layout, float x, float y) {
    if (!layout || !layout->paragraph) return 0;
    auto position = layout->paragraph->getGlyphPositionAtCoordinate(x, y);
    if (position.position < 0) return 0;
    return utf16_to_utf8_offset(layout->text, static_cast<size_t>(position.position));
}

int zant_text_layout_range_rects(ZantTextLayout *layout, size_t start_byte, size_t end_byte,
                                 ZantRect *out_rects, int max_rects) {
    if (!layout || !layout->paragraph || !out_rects || max_rects <= 0) return 0;
    size_t start = utf8_to_utf16_offset(layout->text, start_byte);
    size_t end = utf8_to_utf16_offset(layout->text, end_byte);
    if (start > end) std::swap(start, end);
    if (start == end) return 0;
    auto rects = layout->paragraph->getRectsForRange(static_cast<unsigned>(start),
                                                     static_cast<unsigned>(end),
                                                     RectHeightStyle::kMax, RectWidthStyle::kTight);
    int count = 0;
    for (const auto &box : rects) {
        if (count >= max_rects) break;
        out_rects[count].x = box.rect.left();
        out_rects[count].y = box.rect.top();
        out_rects[count].width = box.rect.width();
        out_rects[count].height = box.rect.height();
        count++;
    }
    return count;
}

int zant_text_layout_placeholder_rects(ZantTextLayout *layout, ZantRect *out_rects, int max_rects) {
    if (!layout || !layout->paragraph || !out_rects || max_rects <= 0) return 0;
    auto rects = layout->paragraph->getRectsForPlaceholders();
    int count = 0;
    for (const auto &box : rects) {
        if (count >= max_rects) break;
        out_rects[count].x = box.rect.left();
        out_rects[count].y = box.rect.top();
        out_rects[count].width = box.rect.width();
        out_rects[count].height = box.rect.height();
        count++;
    }
    return count;
}

}  // extern "C"
