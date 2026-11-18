//! Unit tests.

use super::*;

#[test]
fn wrap_toggles() {
    let r1 = apply(
        "hello world",
        Selection {
            anchor: 6,
            cursor: 11,
        },
        Command::Bold,
    );
    assert_eq!(r1.text, "hello **world**");
    assert_eq!(r1.selection.start(), 8);
    let r2 = apply(&r1.text, r1.selection, Command::Bold);
    assert_eq!(r2.text, "hello world");
}

#[test]
fn wrap_handles_multi_byte_text() {
    // "好" selected: the marker-sized window before it starts inside "你"
    let text = "你好世界";
    let bold = apply(
        text,
        Selection {
            anchor: 3,
            cursor: 6,
        },
        Command::Bold,
    );
    assert_eq!(bold.text, "你**好**世界");
    let plain = apply(&bold.text, bold.selection, Command::Bold);
    assert_eq!(plain.text, text);
    // a caret between two characters inserts an empty pair
    let italic = apply(text, Selection::collapsed(6), Command::Italic);
    assert_eq!(italic.text, "你好**世界");
    let code = apply(text, Selection::collapsed(3), Command::InlineCode);
    assert_eq!(code.text, "你``好世界");
}

#[test]
fn list_continuation() {
    let r = continue_list("1. one", Selection::collapsed(6)).expect("list item");
    assert_eq!(r.text, "1. one\n2. ");
    assert_eq!(r.selection.cursor, 10);
    let t = continue_list("- [x] done", Selection::collapsed(10)).expect("task item");
    assert_eq!(t.text, "- [x] done\n- [ ] ");
    let end = continue_list("- a\n- ", Selection::collapsed(6)).expect("empty item");
    assert_eq!(end.text, "- a\n");
    assert!(continue_list("plain", Selection::collapsed(5)).is_none());
}

#[test]
fn line_prefixes_and_headings() {
    let q = apply(
        "a\nb",
        Selection {
            anchor: 0,
            cursor: 3,
        },
        Command::Quote,
    );
    assert_eq!(q.text, "> a\n> b");
    let o = apply(
        "- a\n- b",
        Selection {
            anchor: 0,
            cursor: 7,
        },
        Command::OrderedList,
    );
    assert_eq!(o.text, "1. a\n2. b");
    let h1 = apply("Title", Selection::collapsed(2), Command::Heading);
    assert_eq!(h1.text, "# Title");
    let h2 = apply(&h1.text, h1.selection, Command::Heading);
    assert_eq!(h2.text, "## Title");
    let ind = indent_list("- a", Selection::collapsed(3), false).expect("list line");
    assert_eq!(ind.text, "  - a");
}
