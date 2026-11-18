#ifndef ZANT_SKIA_BRIDGE_TEXT_INTERNAL_H
#define ZANT_SKIA_BRIDGE_TEXT_INTERNAL_H

#include "skia_bridge.h"

#include "modules/skparagraph/include/Paragraph.h"

#include <memory>
#include <string>
#include <string_view>

using skia::textlayout::Paragraph;

struct ZantTextLayout {
    std::string text;
    ZantTextStyle style{};
    std::unique_ptr<Paragraph> paragraph;
    float layout_width = 0.0f;
    /* rich text: always laid out at the given width */
    bool rich = false;

    void layout_at(float width) {
        if (!paragraph) return;
        const float target = width > 0.0f ? width : 1.0e9f;
        if (target == layout_width) return;
        paragraph->layout(target);
        layout_width = target;
    }
};

namespace zant_skia {

bool utf8_decode_one(std::string_view text, size_t at, uint32_t *out_cp, int *out_len);

}  // namespace zant_skia

#endif
