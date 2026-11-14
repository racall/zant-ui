//! Port of `src/markdown/remend/mod.zig` (WP-3.1).
//!
//! Rust port of Streamdown's `remend`: heals incomplete Markdown at the end of
//! a streamed text so partial syntax renders as its finished form
//! (`**bo` → `**bo**`, `[link](htt` → `[link](streamdown:incomplete-link)`).
//!
//! Handlers run in Streamdown's priority order; each is a separate module
//! mirroring `packages/remend/src`.

pub mod comparison_operator;
pub mod context;
pub mod emphasis;
pub mod html_tag;
pub mod inline_code;
pub mod katex;
pub mod link_image;
pub mod patterns;
pub mod scan;
pub mod setext_heading;
pub mod single_tilde;
pub mod strikethrough;

pub use context::Context;
pub use link_image::{INCOMPLETE_IMAGE, INCOMPLETE_LINK, LinkMode};

/// Custom handler (Streamdown's `RemendHandler`). Runs after the built-ins
/// with the same priority unless `priority` says otherwise.
#[derive(Clone, Copy)]
pub struct Handler {
    pub name: &'static str,
    pub priority: u8,
    pub handle: fn(&mut Context, &str) -> String,
}

/// Every option defaults to enabled, except `inline_katex` (opt-in).
#[derive(Clone)]
pub struct Options {
    pub bold: bool,
    pub bold_italic: bool,
    pub comparison_operators: bool,
    pub html_tags: bool,
    pub images: bool,
    pub inline_code: bool,
    pub inline_katex: bool,
    pub italic: bool,
    pub katex: bool,
    pub link_mode: LinkMode,
    pub links: bool,
    pub setext_headings: bool,
    pub single_tilde: bool,
    pub strikethrough: bool,
    pub handlers: Vec<Handler>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            bold: true,
            bold_italic: true,
            comparison_operators: true,
            html_tags: true,
            images: true,
            inline_code: true,
            inline_katex: false,
            italic: true,
            katex: true,
            link_mode: LinkMode::Protocol,
            links: true,
            setext_headings: true,
            single_tilde: true,
            strikethrough: true,
            handlers: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step {
    SingleTilde,
    ComparisonOperators,
    HtmlTags,
    SetextHeadings,
    Links,
    BoldItalic,
    Bold,
    ItalicDoubleUnderscore,
    ItalicSingleAsterisk,
    ItalicSingleUnderscore,
    InlineCode,
    Strikethrough,
    Katex,
    InlineKatex,
}

/// Built-ins in priority order (`std.enums.values(Step)` order of the Zig port).
const STEPS: [Step; 14] = [
    Step::SingleTilde,
    Step::ComparisonOperators,
    Step::HtmlTags,
    Step::SetextHeadings,
    Step::Links,
    Step::BoldItalic,
    Step::Bold,
    Step::ItalicDoubleUnderscore,
    Step::ItalicSingleAsterisk,
    Step::ItalicSingleUnderscore,
    Step::InlineCode,
    Step::Strikethrough,
    Step::Katex,
    Step::InlineKatex,
];

impl Step {
    fn priority(self) -> u8 {
        match self {
            Step::SingleTilde => 0,
            Step::ComparisonOperators => 5,
            Step::HtmlTags => 10,
            Step::SetextHeadings => 15,
            Step::Links => 20,
            Step::BoldItalic => 30,
            Step::Bold => 35,
            Step::ItalicDoubleUnderscore => 40,
            Step::ItalicSingleAsterisk => 41,
            Step::ItalicSingleUnderscore => 42,
            Step::InlineCode => 50,
            Step::Strikethrough => 60,
            Step::Katex => 70,
            Step::InlineKatex => 75,
        }
    }

    fn enabled(self, o: &Options) -> bool {
        match self {
            Step::SingleTilde => o.single_tilde,
            Step::ComparisonOperators => o.comparison_operators,
            Step::HtmlTags => o.html_tags,
            Step::SetextHeadings => o.setext_headings,
            Step::Links => o.links || o.images,
            Step::BoldItalic => o.bold_italic,
            Step::Bold => o.bold,
            Step::ItalicDoubleUnderscore
            | Step::ItalicSingleAsterisk
            | Step::ItalicSingleUnderscore => o.italic,
            Step::InlineCode => o.inline_code,
            Step::Strikethrough => o.strikethrough,
            Step::Katex => o.katex,
            Step::InlineKatex => o.inline_katex,
        }
    }

    fn run(self, ctx: &mut Context, text: &str, o: &Options) -> String {
        match self {
            Step::SingleTilde => single_tilde::handle_single_tilde_escape(ctx, text),
            Step::ComparisonOperators => {
                comparison_operator::handle_comparison_operators(ctx, text)
            }
            Step::HtmlTags => html_tag::handle_incomplete_html_tag(ctx, text),
            Step::SetextHeadings => setext_heading::handle_incomplete_setext_heading(ctx, text),
            Step::Links => link_image::handle_incomplete_links_and_images(ctx, text, o.link_mode),
            Step::BoldItalic => emphasis::handle_incomplete_bold_italic(ctx, text),
            Step::Bold => emphasis::handle_incomplete_bold(ctx, text),
            Step::ItalicDoubleUnderscore => {
                emphasis::handle_incomplete_double_underscore_italic(ctx, text)
            }
            Step::ItalicSingleAsterisk => {
                emphasis::handle_incomplete_single_asterisk_italic(ctx, text)
            }
            Step::ItalicSingleUnderscore => {
                emphasis::handle_incomplete_single_underscore_italic(ctx, text)
            }
            Step::InlineCode => inline_code::handle_incomplete_inline_code(ctx, text),
            Step::Strikethrough => strikethrough::handle_incomplete_strikethrough(ctx, text),
            Step::Katex => katex::handle_incomplete_block_katex(ctx, text),
            Step::InlineKatex => katex::handle_incomplete_inline_katex(ctx, text),
        }
    }
}

fn strip_single_trailing_space(text: &str) -> &str {
    if text.ends_with(' ') && !text.ends_with("  ") {
        return &text[..text.len() - 1];
    }
    text
}

fn ends_with_incomplete_target(text: &str) -> bool {
    text.ends_with(&format!("]({INCOMPLETE_LINK})"))
        || text.ends_with(&format!("]({INCOMPLETE_IMAGE})"))
}

/// Heals `text` and returns the (possibly unchanged) result.
pub fn remend(text: &str, options: &Options) -> String {
    if text.is_empty() {
        return text.to_owned();
    }
    let mut ctx = Context::new();
    let mut result = strip_single_trailing_space(text).to_owned();

    let builtin_count = STEPS.len();
    let mut custom_index = 0usize;
    let mut step_index = 0usize;
    // Merge built-ins (already priority-sorted) with custom handlers (stable).
    while step_index < builtin_count || custom_index < options.handlers.len() {
        let step: Option<Step> = if step_index < builtin_count {
            Some(STEPS[step_index])
        } else {
            None
        };
        let custom: Option<&Handler> = if custom_index < options.handlers.len() {
            Some(&options.handlers[custom_index])
        } else {
            None
        };
        let take_custom = match (custom, step) {
            (Some(custom), Some(step)) => custom.priority < step.priority(),
            (Some(_), None) => true,
            (None, _) => false,
        };
        if take_custom {
            let handler = custom.expect("custom handler taken");
            result = (handler.handle)(&mut ctx, &result);
            custom_index += 1;
            continue;
        }
        step_index += 1;
        let s = step.expect("step present when no custom handler is taken");
        if !s.enabled(options) {
            continue;
        }
        result = s.run(&mut ctx, &result, options);
        if s == Step::Links
            && options.link_mode == LinkMode::Protocol
            && ends_with_incomplete_target(&result)
        {
            return result;
        }
    }
    strip_single_trailing_space(&result).to_owned()
}

/// Convenience wrapper corresponding to Zig's `remendAlloc`; the allocator
/// parameter disappears because `remend` already returns an owned `String`.
pub fn remend_alloc(text: &str, options: &Options) -> String {
    remend(text, options)
}

#[test]
fn basic_completions() {
    let cases = [
        ("Text with **bold", "Text with **bold**"),
        ("**xxx*", "**xxx**"),
        ("Use `code", "Use `code`"),
        ("~~gone", "~~gone~~"),
        (
            "see [docs](https://exa",
            "see [docs](streamdown:incomplete-link)",
        ),
        ("*it", "*it*"),
        ("$$x^2", "$$x^2$$"),
        ("snake_case_name", "snake_case_name"),
        ("Range 20~25", "Range 20\\~25"),
    ];
    for (input, expected) in cases {
        let out = remend(input, &Options::default());
        assert_eq!(expected, out);
    }
}
