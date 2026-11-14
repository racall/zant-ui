//! Parser-level cases from `tests/markdown_test.zig` (WP-3.2).
//!
//! The remaining cases in that file need the renderer / `ui` layer
//! (`renderDocument`, `streamdown`, `editor`, `linkSafetyModal`, heading /
//! block keys) and are ported by WP-3.3 and the UI work packages.

use zant::markdown::ast::{Align, Block};
use zant::markdown::parser::{self, Options};

fn parse(source: &str) -> zant::markdown::ast::Document {
    parser::parse(source, Options::default())
}

fn paragraph_of(block: &Block) -> &[zant::markdown::ast::Span] {
    match block {
        Block::Paragraph(spans) => spans,
        other => panic!("expected paragraph, got {other:?}"),
    }
}

fn list_of(block: &Block) -> &zant::markdown::ast::List {
    match block {
        Block::List(list) => list,
        other => panic!("expected list, got {other:?}"),
    }
}

#[test]
fn markdown_blocks_and_inline_styles() {
    let source = "# Title\n\nHello **bold** and `code` and $$x^2$$.\n\n- one\n- two\n\n```zig\nconst x = 1;\n```\n";
    let doc = parse(source);
    assert_eq!(doc.blocks.len(), 4);
    let Block::Heading(heading) = &doc.blocks[0] else {
        panic!("expected heading");
    };
    assert_eq!(heading.level, 1);
    let mut has_bold = false;
    let mut has_code = false;
    let mut has_math = false;
    for span in paragraph_of(&doc.blocks[1]) {
        if span.style.bold {
            has_bold = true;
        }
        if span.style.code {
            has_code = true;
        }
        if span.style.math {
            has_math = true;
        }
    }
    assert!(has_bold && has_code && has_math);
    assert_eq!(list_of(&doc.blocks[2]).items.len(), 2);
    let Block::Code(code) = &doc.blocks[3] else {
        panic!("expected code block");
    };
    assert_eq!(code.text, "const x = 1;");
    assert_eq!(code.lang, "zig");
}

#[test]
fn markdown_table_and_task_list() {
    let source = "| Name | Value |\n| --- | ---: |\n| A | 1 |\n\n- [x] done\n- [ ] todo\n";
    let doc = parse(source);
    assert_eq!(doc.blocks.len(), 2);
    let Block::Table(table) = &doc.blocks[0] else {
        panic!("expected table");
    };
    assert_eq!(table.aligns[1], Align::Right);
    assert_eq!(table.raw_rows[0][0], "A");
    let list = list_of(&doc.blocks[1]);
    assert!(list.items[0].checked.expect("first item checked"));
    assert!(!list.items[1].checked.expect("second item unchecked"));
}

#[test]
fn setext_heading_autolinks_indented_code() {
    let doc = parse("Title\n=====\n\n<https://example.com>\n\n    let x = 1;\n    let y = 2;\n");
    assert_eq!(doc.blocks.len(), 3);
    let Block::Heading(heading) = &doc.blocks[0] else {
        panic!("expected heading");
    };
    assert_eq!(heading.level, 1);
    assert!(paragraph_of(&doc.blocks[1])[0].style.link);
    let Block::Code(code) = &doc.blocks[2] else {
        panic!("expected code block");
    };
    assert_eq!(code.text, "let x = 1;\nlet y = 2;");
}

#[test]
fn nested_blockquote_and_nested_lists() {
    let doc = parse("> outer\n>> inner\n\n- a\n  - b\n    - c\n");
    let Block::Quote(outer) = &doc.blocks[0] else {
        panic!("expected block quote");
    };
    assert_eq!(outer.len(), 2);
    let Block::Quote(inner) = &outer[1] else {
        panic!("expected nested block quote");
    };
    assert_eq!(inner.len(), 1);
    let level1 = list_of(&doc.blocks[1]);
    let level2 = list_of(&level1.items[0].blocks[1]);
    let level3 = list_of(&level2.items[0].blocks[1]);
    assert_eq!(level3.items.len(), 1);
}

#[test]
fn entities_escapes_breaks() {
    let doc = parse("a &amp; b &lt;tag&gt; \\*not italic\\* `a < b`\none\ntwo  \nthree");
    let mut joined = String::new();
    for span in paragraph_of(&doc.blocks[0]) {
        assert!(!span.style.italic);
        joined.push_str(&span.text);
    }
    assert!(joined.contains("a & b <tag>"));
    assert!(joined.contains("one two"));
    assert!(joined.contains("two\nthree"));
}

#[test]
fn thematic_break_and_html_block() {
    let doc = parse("a\n\n* * *\n\n<div>hello</div>\n");
    assert_eq!(doc.blocks[1], Block::Rule);
    let Block::Html(html) = &doc.blocks[2] else {
        panic!("expected html block");
    };
    assert_eq!(html, "<div>hello</div>");
}
