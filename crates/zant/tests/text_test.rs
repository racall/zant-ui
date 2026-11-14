//! CJK / mixed shaping, wrapping, caret
//! indexing and the relayout-vs-fresh-layout comparison.

use zant::renderer::text::{self, TextLayout};

#[test]
fn cjk_and_mixed_text_shaping_and_wrapping() {
    let layout = TextLayout::init(
        "Zant UI 中文排版与 English 混排，自动换行并参与 flex 布局。",
        text::Style {
            font_size: 16.0,
            color: 0xe0000000,
            wrap: true,
            line_height: 1.5,
            ..Default::default()
        },
        180.0,
    )
    .expect("text layout");
    let metrics = layout.metrics();
    assert!(metrics.line_count >= 2);
    assert!(metrics.height > 20.0);
    assert!(metrics.longest_line <= 180.5);
    assert!(layout.caret_rect(0).is_some());
    assert!(layout.caret_rect(3).is_some());
}

#[test]
fn utf8_cursor_offset_never_splits_chinese_input() {
    let s = "中文 Zant";
    let layout = TextLayout::init(
        s,
        text::Style {
            font_size: 14.0,
            wrap: false,
            ..Default::default()
        },
        1.0e6,
    )
    .expect("text layout");
    let caret = layout.caret_rect(3).expect("caret");
    assert!(caret.width > 0.0);
    let index = layout.index_at(caret.x + 1.0, caret.y + 2.0);
    assert!(index <= s.len());
}

#[test]
fn text_relayout_at_another_width_matches_a_fresh_layout() {
    let samples = [
        text::Style {
            font_size: 15.0,
            wrap: true,
            ..Default::default()
        },
        text::Style {
            font_size: 13.0,
            wrap: false,
            text_align: text::Align::Center,
            ..Default::default()
        },
        text::Style {
            font_size: 14.0,
            wrap: true,
            max_lines: 2,
            ..Default::default()
        },
    ];
    let content = "Relayout keeps the shaped text: 中文混排 English words wrap differently at each width, 123456.";
    for style in samples {
        let mut a = TextLayout::init(content, style, 300.0).expect("text layout");
        a.relayout(140.0);
        let fresh = TextLayout::init(content, style, 140.0).expect("text layout");
        assert!(
            (fresh.metrics().height - a.metrics().height).abs() <= 0.01,
            "height {} != {}",
            fresh.metrics().height,
            a.metrics().height
        );
        assert!(
            (fresh.metrics().longest_line - a.metrics().longest_line).abs() <= 0.01,
            "longest_line {} != {}",
            fresh.metrics().longest_line,
            a.metrics().longest_line
        );
        assert_eq!(fresh.metrics().line_count, a.metrics().line_count);
        assert_eq!(
            fresh.index_at(100.0, fresh.metrics().height - 5.0),
            a.index_at(100.0, a.metrics().height - 5.0)
        );
    }
}
