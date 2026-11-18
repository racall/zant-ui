//! `zant-markdown`: the Markdown (Streamdown) gallery.
//!
//! The repository documentation sample rendered with `markdown::streamdown`,
//! `ZANT_DARK=1` for dark. The block cache is owned by the app.
#![forbid(unsafe_code)]

use zant::core::geometry::Edges;
use zant::flex;
use zant::markdown::render::options::Options as RenderOptions;
use zant::markdown::render::view::Props as ViewProps;
use zant::markdown::{self, cache::Cache};
use zant::platform::platform::WindowOptions;
use zant::theme::tokens::{self, Theme};
use zant::ui::{self, BoxOptions, RunOptions};

/// The sample document (no trailing newline).
pub const SAMPLE: &str = r#"# Streamdown for Zant

Native Markdown with **bold**, *italic*, ~~strike~~, ==highlight==, `inline code`, [links](https://github.com/vercel/streamdown), autolinks like https://zant.dev, footnotes[^1] and inline math $$e^{i\pi} + 1 = 0$$ that sits on the baseline.

> [!TIP]
> Callouts, quotes and **nested** content are supported.

## Lists

1. Ordered item
2. Second item
   - [x] finished task
   - [ ] open task
     - deeper bullet

## Math

$$
\int_{-\infty}^{\infty} e^{-x^2}\,dx = \sqrt{\pi} \qquad A = \begin{pmatrix} 1 & 2 \\ 3 & 4 \end{pmatrix}
$$

## Table

| Feature | Status | Notes |
| :--- | :---: | ---: |
| remend | ✅ | 522 upstream cases |
| parse-blocks | ✅ | incremental |

## Code

```rust
use std::env;
fn main() {
    println!("hello {}", env::args().count()); // greet
}
```

## Diagram

```mermaid
graph LR
  A[Source] --> B(remend)
  B --> C{parse-blocks}
  C -->|changed| D[Parse]
  C -->|stable| E[Reuse]
  D --> F([Render])
  E --> F
```

---

[^1]: Footnotes render at the end of the document."#;

/// The block cache and the dark flag.
pub struct MarkdownApp {
    cache: Cache,
    dark: bool,
}

impl MarkdownApp {
    pub fn new(dark: bool) -> MarkdownApp {
        MarkdownApp {
            cache: Cache::new(),
            dark,
        }
    }

    /// The window tree: the root node never scrolls itself, the content column
    /// does, wide enough for a 720 px document.
    fn build(&mut self) -> ui::Node {
        let theme = if self.dark {
            markdown::theme::dark()
        } else {
            markdown::theme::light()
        };
        ui::column(BoxOptions {
            grow: 1.0,
            background: Some(theme.background),
            align_items: flex::Align::Stretch,
            children: vec![ui::column(BoxOptions {
                grow: 1.0,
                scroll: true,
                align_items: flex::Align::Center,
                padding: Edges {
                    top: 36.0,
                    bottom: 48.0,
                    ..Edges::default()
                },
                children: vec![ui::column(BoxOptions {
                    width: Some(720.0),
                    align_items: flex::Align::Stretch,
                    children: vec![markdown::streamdown(
                        &mut self.cache,
                        SAMPLE,
                        ViewProps {
                            render: RenderOptions {
                                theme,
                                content_width: 720.0,
                                ..RenderOptions::default()
                            },
                            ..ViewProps::default()
                        },
                    )],
                    ..BoxOptions::default()
                })],
                ..BoxOptions::default()
            })],
            ..BoxOptions::default()
        })
    }
}

impl ui::App for MarkdownApp {
    fn render(&mut self) -> ui::Node {
        self.build()
    }
}

/// Window title, size (860×1500) and theme of the example.
pub fn run_options(dark: bool) -> RunOptions {
    RunOptions {
        title: String::from("Zant Markdown"),
        window: WindowOptions {
            width: 860.0,
            height: 1500.0,
            ..WindowOptions::default()
        },
        theme: app_theme(dark),
        ..Default::default()
    }
}

/// The Ant Design light or dark theme.
pub fn app_theme(dark: bool) -> Theme {
    if dark {
        tokens::ant_dark()
    } else {
        tokens::ant_light()
    }
}
