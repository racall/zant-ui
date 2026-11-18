//! Inline Markdown parser unit tests.

use super::{Context, RefMap, parse};

fn test_context<'a>(refs: &'a RefMap, order: &'a mut Vec<String>) -> Context<'a> {
    Context {
        refs,
        footnote_order: order,
        single_dollar_math: true,
        latex_delimiters: false,
    }
}

#[test]
fn emphasis_and_code() {
    let refs = RefMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut ctx = test_context(&refs, &mut order);
    let spans = parse(&mut ctx, "a **b *c*** `d` snake_case_word ~~e~~");
    let mut bold_italic = false;
    let mut code = false;
    let mut strike = false;
    for s in &spans {
        if s.style.bold && s.style.italic && s.text == "c" {
            bold_italic = true;
        }
        if s.style.code {
            code = true;
        }
        if s.style.strike {
            strike = true;
        }
        assert!(!(s.style.italic && s.text == "case"));
    }
    assert!(bold_italic && code && strike);
}

#[test]
fn cjk_emphasis_closes_before_ideographic_text() {
    let refs = RefMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut ctx = test_context(&refs, &mut order);
    let spans = parse(&mut ctx, "**中文。**后文");
    assert!(spans[0].style.bold);
    assert_eq!(spans[0].text, "中文。");
}

#[test]
fn links_and_autolinks() {
    let refs = RefMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut ctx = test_context(&refs, &mut order);
    let spans = parse(
        &mut ctx,
        "[a](http://x.com \"t\") and https://y.com/p, mail me@ex.com",
    );
    let urls = spans.iter().filter(|s| s.style.link).count();
    assert_eq!(urls, 3);
    assert_eq!(spans[0].url, "http://x.com");
}
