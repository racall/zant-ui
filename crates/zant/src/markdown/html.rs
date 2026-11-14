//! Port of `src/markdown/html.zig` — WP-3.2.
//!
//! HTML export of a parsed document (GFM-style markup). `standalone`
//! wraps it in a complete page with readable default styles and KaTeX
//! (auto-render) for math; otherwise only the body fragment is produced.

use crate::markdown::ast;

pub struct Options {
    /// Full HTML page (head, styles, KaTeX) instead of a fragment.
    pub standalone: bool,
    pub title: String,
    /// Dark page colours.
    pub dark: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            standalone: true,
            title: "Document".to_string(),
            dark: false,
        }
    }
}

struct Writer {
    out: String,
}

impl Writer {
    fn raw(&mut self, s: &str) {
        self.out.push_str(s);
    }

    fn print(&mut self, s: &str) {
        self.out.push_str(s);
    }

    /// Escapes text for element content and attribute values.
    fn text(&mut self, s: &str) {
        for c in s.chars() {
            match c {
                '&' => self.raw("&amp;"),
                '<' => self.raw("&lt;"),
                '>' => self.raw("&gt;"),
                '"' => self.raw("&quot;"),
                _ => self.out.push(c),
            }
        }
    }
}

/// Renders `doc` as HTML.
pub fn to_html(doc: &ast::Document, options: &Options) -> String {
    let mut w = Writer { out: String::new() };
    if options.standalone {
        head(&mut w, options);
    }
    blocks(&mut w, &doc.blocks);
    if !doc.footnotes.is_empty() {
        w.raw("<section class=\"footnotes\"><hr><ol>\n");
        for f in &doc.footnotes {
            w.raw("<li id=\"fn-");
            w.text(&f.label);
            w.raw("\">");
            blocks(&mut w, &f.blocks);
            w.raw("</li>\n");
        }
        w.raw("</ol></section>\n");
    }
    if options.standalone {
        w.raw("</article>\n</body>\n</html>\n");
    }
    w.out
}

fn head(w: &mut Writer, o: &Options) {
    let bg = if o.dark { "#1b1d1c" } else { "#fcfcfa" };
    let fg = if o.dark { "#e4e6e1" } else { "#292d2a" };
    let muted = if o.dark { "#9aa19b" } else { "#7d837e" };
    let line = if o.dark { "#333734" } else { "#e9ebe7" };
    let code_bg = if o.dark { "#202321" } else { "#f4f5f2" };
    w.raw("<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>");
    w.text(&o.title);
    w.raw("</title>\n");
    w.raw("<link rel=\"stylesheet\" href=\"https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.css\">\n");
    w.raw("<script defer src=\"https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.js\"></script>\n");
    w.raw("<script defer src=\"https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/contrib/auto-render.min.js\" onload=\"renderMathInElement(document.body,{delimiters:[{left:'\\\\[',right:'\\\\]',display:true},{left:'\\\\(',right:'\\\\)',display:false}]})\"></script>\n");
    w.print(&format!(
        "<style>
body{{margin:0;background:{bg};color:{fg};font:17px/1.8 -apple-system,\"PingFang SC\",\"Helvetica Neue\",sans-serif}}
article{{max-width:720px;margin:0 auto;padding:56px 24px 96px}}
h1,h2,h3,h4{{font-family:\"Songti SC\",Georgia,serif;font-weight:500;line-height:1.3;margin:1.6em 0 .6em}}
a{{color:inherit;text-underline-offset:3px}}
blockquote{{margin:1em 0;padding:0 1em;border-left:3px solid {muted};color:{muted}}}
code{{font:.9em ui-monospace,Menlo,monospace;background:{code_bg};padding:.1em .35em;border-radius:4px}}
pre{{background:{code_bg};border:1px solid {line_};border-radius:8px;padding:14px 16px;overflow:auto}}
pre code{{background:none;padding:0}}
table{{border-collapse:collapse;margin:1em 0}}th,td{{border:1px solid {line_};padding:6px 12px}}
hr{{border:0;border-top:1px solid {line_};margin:2em 0}}img{{max-width:100%}}
.callout{{border-left:3px solid {muted};padding:.2em 1em;margin:1em 0}}.callout-title{{font-weight:600}}
li.task{{list-style:none;margin-left:-1.4em}}
</style>
</head>
<body>
<article>
",
        line_ = line,
    ));
}

fn blocks(w: &mut Writer, list: &[ast::Block]) {
    for b in list {
        block(w, b);
    }
}

fn block(w: &mut Writer, b: &ast::Block) {
    match b {
        ast::Block::Heading(h) => {
            w.print(&format!("<h{}", h.level));
            if !h.id.is_empty() {
                w.raw(" id=\"");
                w.text(&h.id);
                w.raw("\"");
            }
            w.raw(">");
            spans(w, &h.spans);
            w.print(&format!("</h{}>\n", h.level));
        }
        ast::Block::Paragraph(s) => {
            w.raw("<p>");
            spans(w, s);
            w.raw("</p>\n");
        }
        ast::Block::Code(c) => {
            w.raw("<pre><code");
            if !c.lang.is_empty() {
                w.raw(" class=\"language-");
                w.text(&c.lang);
                w.raw("\"");
            }
            w.raw(">");
            w.text(&c.text);
            w.raw("</code></pre>\n");
        }
        ast::Block::Quote(inner) => {
            w.raw("<blockquote>\n");
            blocks(w, inner);
            w.raw("</blockquote>\n");
        }
        ast::Block::Callout(c) => {
            w.raw("<div class=\"callout\"><div class=\"callout-title\">");
            w.text(c.kind.label());
            w.raw("</div>\n");
            blocks(w, &c.blocks);
            w.raw("</div>\n");
        }
        ast::Block::List(l) => {
            if l.ordered {
                if l.start != 1 {
                    w.print(&format!("<ol start=\"{}\">\n", l.start));
                } else {
                    w.raw("<ol>\n");
                }
            } else {
                w.raw("<ul>\n");
            }
            for item in &l.items {
                if let Some(checked) = item.checked {
                    w.raw(if checked {
                        "<li class=\"task\"><input type=\"checkbox\" checked disabled> "
                    } else {
                        "<li class=\"task\"><input type=\"checkbox\" disabled> "
                    });
                } else {
                    w.raw("<li>");
                }
                // tight lists: inline the single paragraph
                if l.tight
                    && !item.blocks.is_empty()
                    && matches!(item.blocks[0], ast::Block::Paragraph(_))
                {
                    if let ast::Block::Paragraph(paragraph) = &item.blocks[0] {
                        spans(w, paragraph);
                    }
                    blocks(w, &item.blocks[1..]);
                } else {
                    blocks(w, &item.blocks);
                }
                w.raw("</li>\n");
            }
            w.raw(if l.ordered { "</ol>\n" } else { "</ul>\n" });
        }
        ast::Block::Table(t) => {
            w.raw("<table>\n<thead><tr>");
            for (i, cell) in t.headers.iter().enumerate() {
                cell_open(w, "th", &t.aligns, i);
                spans(w, cell);
                w.raw("</th>");
            }
            w.raw("</tr></thead>\n<tbody>\n");
            for row in &t.rows {
                w.raw("<tr>");
                for (i, cell) in row.iter().enumerate() {
                    cell_open(w, "td", &t.aligns, i);
                    spans(w, cell);
                    w.raw("</td>");
                }
                w.raw("</tr>\n");
            }
            w.raw("</tbody>\n</table>\n");
        }
        ast::Block::Math(m) => {
            w.raw("<p class=\"math\">\\[");
            w.text(&m.source);
            w.raw("\\]</p>\n");
        }
        ast::Block::Image(img) => {
            w.raw("<p><img src=\"");
            w.text(&img.url);
            w.raw("\" alt=\"");
            w.text(&img.alt);
            w.raw("\"></p>\n");
        }
        // raw HTML blocks are escaped: exported pages never run embedded markup
        ast::Block::Html(h) => {
            w.raw("<pre>");
            w.text(h);
            w.raw("</pre>\n");
        }
        ast::Block::Rule => w.raw("<hr>\n"),
    }
}

fn cell_open(w: &mut Writer, tag: &str, aligns: &[ast::Align], i: usize) {
    let a = if i < aligns.len() {
        aligns[i]
    } else {
        ast::Align::None
    };
    match a {
        ast::Align::None => w.print(&format!("<{tag}>")),
        ast::Align::Left => w.print(&format!("<{tag} style=\"text-align:left\">")),
        ast::Align::Center => w.print(&format!("<{tag} style=\"text-align:center\">")),
        ast::Align::Right => w.print(&format!("<{tag} style=\"text-align:right\">")),
    }
}

fn spans(w: &mut Writer, list: &[ast::Span]) {
    for s in list {
        span(w, s);
    }
}

fn span(w: &mut Writer, s: &ast::Span) {
    let st = s.style;
    if st.marker {
        return;
    }
    if st.math {
        w.raw("\\(");
        w.text(&s.text);
        w.raw("\\)");
        return;
    }
    if st.image {
        w.raw("<img src=\"");
        w.text(&s.url);
        w.raw("\" alt=\"");
        w.text(&s.text);
        w.raw("\">");
        return;
    }
    if st.footnote_ref {
        w.raw("<sup><a href=\"#fn-");
        w.text(&s.url);
        w.raw("\">");
        w.text(&s.text);
        w.raw("</a></sup>");
        return;
    }
    if st.link {
        w.raw("<a href=\"");
        w.text(&s.url);
        w.raw("\">");
    }
    if st.bold {
        w.raw("<strong>");
    }
    if st.italic {
        w.raw("<em>");
    }
    if st.strike {
        w.raw("<del>");
    }
    if st.underline {
        w.raw("<u>");
    }
    if st.highlight {
        w.raw("<mark>");
    }
    if st.superscript {
        w.raw("<sup>");
    }
    if st.subscript {
        w.raw("<sub>");
    }
    if st.code {
        w.raw("<code>");
    }
    // soft line breaks stay newlines; hard breaks become <br>
    for c in s.text.chars() {
        if c == '\n' {
            w.raw("\n");
        } else {
            w.text(&c.to_string());
        }
    }
    if st.code {
        w.raw("</code>");
    }
    if st.subscript {
        w.raw("</sub>");
    }
    if st.superscript {
        w.raw("</sup>");
    }
    if st.highlight {
        w.raw("</mark>");
    }
    if st.underline {
        w.raw("</u>");
    }
    if st.strike {
        w.raw("</del>");
    }
    if st.italic {
        w.raw("</em>");
    }
    if st.bold {
        w.raw("</strong>");
    }
    if st.link {
        w.raw("</a>");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::parser;

    #[test]
    fn html_export_covers_the_common_blocks_and_escapes_text() {
        let doc = parser::parse(
            "# Title a < b & c\n\nSome **bold** and $x^2$ & [a link](https://e.com).\n\n- [x] done\n- open\n\n```zig\na < b\n```\n\n$$\n\\frac{1}{2}\n$$\n",
            parser::Options::default(),
        );
        let html = to_html(
            &doc,
            &Options {
                standalone: false,
                ..Default::default()
            },
        );
        assert!(html.contains("Title a &lt; b &amp; c</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("\\(x^2\\)"));
        assert!(html.contains("&amp;"));
        assert!(html.contains("<a href=\"https://e.com\">a link</a>"));
        assert!(html.contains("type=\"checkbox\" checked"));
        assert!(html.contains("class=\"language-zig\">a &lt; b"));
        assert!(html.contains("\\[\\frac{1}{2}"));
        let page = to_html(
            &doc,
            &Options {
                title: "T".to_string(),
                ..Default::default()
            },
        );
        assert!(page.starts_with("<!doctype html>"));
        assert!(page.ends_with("</html>\n"));
    }
}
