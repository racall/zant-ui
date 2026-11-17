//! Markdown tests: the parser-level cases, then the renderer / editor cases
//! (`render_document`, `streamdown`, `editor`, `link_safety_modal`, heading /
//! block keys).

use zant::markdown::ast::{Align, Block};
use zant::markdown::cache::{Cache, Mode};
use zant::markdown::editor::{self, Mode as EditorMode};
use zant::markdown::parser::{self, Options};
use zant::markdown::render::options::{Caret, Labels, Options as RenderOptions};
use zant::markdown::render::view::{self, Props as ViewProps};
use zant::markdown::render::{LinkModalProps, link_safety_modal};
use zant::markdown::theme;
use zant::ui;

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

pub const FULL_SAMPLE: &str = r#"---
title: Sample
---

# Heading with $$E=mc^2$$

Paragraph with **bold**, *italic*, ~~strike~~, ==mark==, `code`, [link](https://example.com), <https://zant.dev>, a footnote[^1] and $$\frac{a}{b}$$ inline.

> [!TIP]
> A callout with **markdown**.

> A quote
> > nested

1. first
2. second
   - [x] task
   - [ ] todo

| Left | Center | Right |
| :--- | :---: | ---: |
| a | b | c |

```ts
const x: number = 42; // answer
function f(a: string) { return `v${a}`; }
```

```mermaid
graph TD
  A[Start] --> B{Ok?}
  B -->|yes| C(Done)
  B -->|no| A
```

```mermaid
sequenceDiagram
  Alice->>Bob: Hello
  Bob-->>Alice: Hi
```

```mermaid
pie title Pets
  "Dogs" : 386
  "Cats" : 85
```

$$
\sum_{k=1}^{n} k = \frac{n(n+1)}{2}
$$

![alt text](https://example.com/a.png)

---

[^1]: The footnote.
"#;

#[test]
fn full_document_renders() {
    let doc = parse(FULL_SAMPLE);
    assert_eq!(doc.front_matter, "title: Sample");
    assert_eq!(doc.footnotes.len(), 1);
    ui::begin_frame();
    let node = view::document(
        &doc,
        RenderOptions {
            is_animating: true,
            caret: Caret::Block,
            ..Default::default()
        },
    );
    assert!(!node.element().children.is_empty());
    let dark = view::document(
        &doc,
        RenderOptions {
            theme: theme::dark(),
            labels: Labels::for_locale("zh-CN"),
            ..Default::default()
        },
    );
    assert!(!dark.element().children.is_empty());
    ui::end_frame();
}

#[test]
fn streamdown_view_memoises_blocks() {
    let mut cache = Cache::new();
    ui::begin_frame();
    let first = view::streamdown(
        &mut cache,
        "# A\n\npara **bo",
        ViewProps {
            mode: Mode::Streaming,
            render: RenderOptions {
                is_animating: true,
                caret: Caret::Block,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let first_hash = first.element().children[0]
        .element()
        .memo_hash
        .expect("a memoised block");
    ui::end_frame();
    assert_eq!(first.element().children.len(), 2);
    ui::begin_frame();
    let second = view::streamdown(
        &mut cache,
        "# A\n\npara **bold** done",
        ViewProps {
            mode: Mode::Streaming,
            render: RenderOptions {
                is_animating: true,
                caret: Caret::Block,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    // the unchanged heading block keeps its memo hash
    assert_eq!(
        first_hash,
        second.element().children[0]
            .element()
            .memo_hash
            .expect("a memoised block")
    );
    ui::end_frame();
}

#[test]
fn block_keys_are_stable_across_insertions_and_resolvable_by_offset() {
    let mut cache = Cache::new();
    let src1 = "# A\n\nOne.\n\n## B\n\nTwo.";
    ui::begin_frame();
    let first = view::streamdown(&mut cache, src1, ViewProps::default());
    let heading_key = first.element().children[2]
        .element()
        .key
        .clone()
        .expect("a keyed block");
    ui::end_frame();
    ui::begin_frame();
    let src2 = "# A\n\nInserted.\n\nOne.\n\n## B\n\nTwo.";
    let second = view::streamdown(&mut cache, src2, ViewProps::default());
    assert_eq!(
        Some(heading_key.as_str()),
        second.element().children[3].element().key.as_deref()
    );
    let offset = src2.find("## B").expect("the heading");
    assert_eq!(
        Some(heading_key.as_str()),
        view::block_key_at_offset(&cache, "md", offset + 2).as_deref()
    );
    ui::end_frame();
}

#[test]
fn editor_renders_in_every_mode() {
    let mut cache = Cache::new();
    ui::begin_frame();
    for mode in [EditorMode::Edit, EditorMode::Split, EditorMode::Preview] {
        let node = editor::editor(editor::Props {
            value: "# Hi\n\n- a",
            mode,
            cache: Some(&mut cache),
            labels: editor::Labels::for_locale("zh-CN"),
            ..Default::default()
        });
        assert_eq!(node.element().children.len(), 2, "mode {mode:?}");
    }
    ui::end_frame();
}

#[test]
fn link_safety_modal_is_absolute() {
    ui::begin_frame();
    let node = link_safety_modal(LinkModalProps {
        url: "https://example.com".into(),
        labels: Labels::for_locale("zh-CN"),
        ..Default::default()
    });
    ui::end_frame();
    assert!(node.element().layout.absolute);
}

#[test]
fn heading_keys_resolve_to_rendered_heading_nodes() {
    let mut cache = Cache::new();
    // the footnote keeps the whole document in one block
    let src = "# A\n\ntext[^1]\n\n## B\n\nmore\n\n[^1]: note";
    ui::begin_frame();
    let _ = view::streamdown(&mut cache, src, ViewProps::default());
    ui::end_frame();
    assert_eq!(cache.entries.len(), 1);
    let key = view::heading_key(&cache, "md", 1).expect("the second heading");
    let expected = format!("md-e{}-2", cache.entries[0].id);
    assert_eq!(key, expected);
    assert!(view::heading_key(&cache, "md", 2).is_none());
}
