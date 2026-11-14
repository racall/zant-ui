//! Port of `src/markdown/parser/mod.zig` — WP-3.2.
//!
//! Markdown → `ast.Document`.

pub mod block;
pub mod inline;

use crate::markdown::ast;
pub use block::{LineRange, Options};

/// Parses `source` into a document that owns all its memory.
pub fn parse(source: &str, options: Options) -> ast::Document {
    let mut parser = block::Parser::new(options);
    parser.parse_document(source, None).into()
}

/// Top-level block line ranges, without parsing inline content.
pub fn top_level_ranges(source: &str, options: Options) -> Vec<LineRange> {
    let mut parser = block::Parser::new(options);
    parser.structure_only = true;
    let mut ranges: Vec<LineRange> = Vec::new();
    let _ = parser.parse_document(source, Some(&mut ranges));
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::ast::Block;

    fn paragraph_of(block: &Block) -> &[ast::Span] {
        match block {
            Block::Paragraph(spans) => spans,
            other => panic!("expected paragraph, got {other:?}"),
        }
    }

    #[test]
    fn nested_lists_with_tasks_and_loose_items() {
        let doc = parse("- a\n  - [x] b\n    1. c\n\n- d\n", Options::default());
        let Block::List(list) = &doc.blocks[0] else {
            panic!("expected list");
        };
        assert!(!list.tight);
        assert_eq!(list.items.len(), 2);
        let Block::List(nested) = &list.items[0].blocks[1] else {
            panic!("expected nested list");
        };
        assert!(nested.items[0].checked.expect("task state"));
        let Block::List(nested_ordered) = &nested.items[0].blocks[1] else {
            panic!("expected nested ordered list");
        };
        assert!(nested_ordered.ordered);
    }

    #[test]
    fn footnotes_callouts_front_matter() {
        let src = "---\ntitle: x\n---\n\n> [!WARNING]\n> careful\n\nText[^n].\n\n[^n]: The note.\n";
        let doc = parse(src, Options::default());
        assert_eq!(doc.front_matter, "title: x");
        let Block::Callout(callout) = &doc.blocks[0] else {
            panic!("expected callout");
        };
        assert_eq!(callout.kind, ast::CalloutKind::Warning);
        assert_eq!(doc.footnotes.len(), 1);
        assert_eq!(doc.footnotes[0].number, 1);
    }

    #[test]
    fn top_level_ranges() {
        let ranges = super::top_level_ranges(
            "# A\n\npara\nmore\n\n```\nx\n\ny\n```\n- a\n- b\n",
            Options::default(),
        );
        assert_eq!(ranges.len(), 4);
        assert_eq!(ranges[1].start, 2);
        assert_eq!(ranges[1].end, 4);
        assert_eq!(ranges[2].end, 10);
    }

    #[test]
    fn remark_math_syntax() {
        let doc = parse(
            "$20 and $10\n\nDisplay: $$E = mc^2$$\n\n$$\n\\frac{a}{b}\n$$\n\n$$x$$\n",
            Options::default(),
        );
        for s in paragraph_of(&doc.blocks[0]) {
            assert!(!s.style.math);
        }
        let mut inline_math = false;
        for s in paragraph_of(&doc.blocks[1]) {
            if s.style.math {
                inline_math = true;
            }
        }
        assert!(inline_math);
        let Block::Math(math) = &doc.blocks[2] else {
            panic!("expected math block");
        };
        assert_eq!(math.source, "\\frac{a}{b}");
        // single-line $$x$$ is text math inside a paragraph (meta may not contain $)
        assert!(matches!(doc.blocks[3], Block::Paragraph(_)));
    }

    fn math_spans(doc: &ast::Document) -> Vec<String> {
        let mut out = Vec::new();
        for s in paragraph_of(&doc.blocks[0]) {
            if s.style.math && out.len() < 8 {
                out.push(s.text.clone());
            }
        }
        out
    }

    #[test]
    fn single_dollar_inline_math_pandoc_rules() {
        let doc = parse(
            "其中 $U=[u_1\\ \\cdots\\ u_m]$，$V=[v_1\\ \\cdots\\ v_m]$。",
            Options::default(),
        );
        let out = math_spans(&doc);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], "U=[u_1\\ \\cdots\\ u_m]");
        assert_eq!(out[1], "V=[v_1\\ \\cdots\\ v_m]");
        for src in ["$20 and $10", "price $5-$10", "cost is $"] {
            let doc = parse(src, Options::default());
            assert_eq!(math_spans(&doc).len(), 0);
        }
        for src in ["a $x$ b", "\\$5 and $x$"] {
            let doc = parse(src, Options::default());
            let out = math_spans(&doc);
            assert_eq!(out.len(), 1);
            assert_eq!(out[0], "x");
        }
    }
}
