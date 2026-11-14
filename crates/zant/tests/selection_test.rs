//! Port of `tests/selection_test.zig` — text selection in rendered Markdown:
//! drag across blocks, word / block clicks, copied text (formulas as LaTeX,
//! markers, gutters and controls left out) plus the LaTeX delimiter and
//! streaming-formula parser cases.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use zant::core::events::{Event, Key, KeyEvent, PointerEvent};
use zant::core::node::{NodeData, NodeId, NodeKind};
use zant::image::Image;
use zant::markdown::ast::{Block, Document};
use zant::markdown::cache::{Cache, Mode};
use zant::markdown::parser;
use zant::markdown::render::{
    BlockInfo, BlockRenderer, ImageLoader, Options as RenderOptions, Props,
};
use zant::markdown::render_document;
use zant::markdown::streamdown;
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant::ui::Style;
use zant::ui::app::{App, RunOptions};
use zant::ui::element::Node;
use zant::ui::options::{BoxOptions, ParagraphOptions, TextOptions};
use zant::ui::{column, inset, paragraph, styled, text};

const DOC: &str = "Hello **world** and $x^2$ math\n\n- item one\n- item two\n\n```js\nlet a = 1\n```\n\nLast line";

struct DocApp {
    cache: Cache,
}

impl App for DocApp {
    fn render(&mut self) -> Node {
        column(BoxOptions {
            grow: 1.0,
            padding: inset(20.0),
            align_items: zant::layout::flex::Align::Stretch,
            children: zant::nodes![streamdown(
                &mut self.cache,
                DOC,
                Props {
                    render: RenderOptions {
                        key_prefix: "doc".into(),
                        content_width: 560.0,
                        ..Default::default()
                    },
                    ..Default::default()
                }
            )],
            ..Default::default()
        })
    }
}

fn frame(runtime: &mut Runtime, renderer: &mut Renderer) {
    let canvas = renderer.begin(1.0).expect("canvas");
    runtime.render_frame(canvas, 600.0, 700.0, 1.0);
    renderer.end();
}

fn frame_at(runtime: &mut Runtime, renderer: &mut Renderer, w: f32, h: f32) {
    let canvas = renderer.begin(1.0).expect("canvas");
    runtime.render_frame(canvas, w, h, 1.0);
    renderer.end();
}

fn first_rich(runtime: &Runtime) -> Option<NodeId> {
    for node in &runtime.tree.nodes {
        if !node.alive {
            continue;
        }
        if matches!(node.data, NodeData::RichText(_)) {
            return Some(node.id);
        }
    }
    None
}

fn pointer(x: f32, y: f32) -> PointerEvent {
    PointerEvent {
        x,
        y,
        ..Default::default()
    }
}

fn window(width: f32, height: f32) -> RunOptions {
    RunOptions {
        window: zant::platform::platform::WindowOptions {
            width,
            height,
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn selection_drag_across_markdown_blocks_copies_clean_text() {
    zant::motion::motion::set_enabled(false);
    let mut runtime = Runtime::init(
        DocApp {
            cache: Cache::new(),
        },
        window(600.0, 700.0),
    );
    let mut renderer = Renderer::create_raster(600, 700).expect("raster renderer");
    frame(&mut runtime, &mut renderer);

    let para = first_rich(&runtime).expect("text");
    let b = runtime.tree.get(para).bounds;
    // press at the very start of the first paragraph, drag far below
    runtime.dispatch(Event::PointerDown(pointer(b.x + 1.0, b.y + b.height / 2.0)));
    runtime.dispatch(Event::PointerMove(pointer(300.0, 400.0)));
    runtime.dispatch(Event::PointerMove(pointer(590.0, 690.0)));
    runtime.dispatch(Event::PointerUp(pointer(590.0, 690.0)));
    assert_eq!(
        runtime.selected_text(),
        "Hello world and $x^2$ math\nitem one\nitem two\nlet a = 1\nLast line"
    );
    frame(&mut runtime, &mut renderer);
    assert!(runtime.tree.get(para).text_sel.is_some());

    // a plain click clears it
    runtime.dispatch(Event::PointerDown(pointer(b.x + 1.0, b.y + b.height / 2.0)));
    runtime.dispatch(Event::PointerUp(pointer(b.x + 1.0, b.y + b.height / 2.0)));
    assert_eq!(runtime.selected_text(), "");
    assert!(runtime.tree.get(para).text_sel.is_none());

    // double click a word (after the multi-click window, a fresh count)
    runtime.last_click_ms -= 2000;
    {
        let width = runtime.tree.get(para).bounds.width;
        if let NodeData::RichText(data) = &mut runtime.tree.get_mut(para).data {
            let _ = data.ensure_layout(width);
        }
    }
    for _ in 0..2 {
        runtime.dispatch(Event::PointerDown(pointer(
            b.x + 60.0,
            b.y + b.height / 2.0,
        )));
        runtime.dispatch(Event::PointerUp(pointer(b.x + 60.0, b.y + b.height / 2.0)));
    }
    assert_eq!(runtime.selected_text(), "world");

    // Escape clears
    runtime.dispatch(Event::KeyDown(KeyEvent {
        key: Key::Escape,
        modifiers: Default::default(),
        repeat: false,
    }));
    assert_eq!(runtime.selected_text(), "");
    zant::motion::motion::set_enabled(true);
}

#[test]
fn markdown_custom_block_renderers_image_loader_and_data_uris() {
    let calls = Rc::new(Cell::new(0usize));
    let pixels = vec![0x80u8; 8 * 4 * 4];
    let loader_image = Image::from_pixels(&pixels, 8, 4, 32).expect("image");
    let source = concat!(
        "```widget {\"id\":3}\nchart\n```\n\n",
        "```widget\nskip\n```\n\n",
        "![remote](https://example.com/a.png)\n\n",
        "![pending](https://example.com/b.png)\n\n",
        "![inline](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==)"
    );
    let doc = parser::parse(source, parser::Options::default());
    let counters = Rc::clone(&calls);
    let renderers = vec![BlockRenderer::new("widget", move |info: &BlockInfo| {
        counters.set(counters.get() + 1);
        if info.code == "skip" {
            return None;
        }
        Some(text(
            format!("widget:{}:{}", info.meta, info.code),
            TextOptions {
                key: Some("widget-text".into()),
                ..Default::default()
            },
        ))
    })];
    let loader_fn: ImageLoader = {
        let url_pixels = pixels;
        let image_holder = Rc::new(RefCell::new(Some(loader_image)));
        Rc::new(move |url: &str| {
            if url != "https://example.com/a.png" {
                return None;
            }
            if let Some(image) = image_holder.borrow_mut().take() {
                return Some(image);
            }
            Image::from_pixels(&url_pixels, 8, 4, 32)
        })
    };
    let node = render_document(
        &doc,
        RenderOptions {
            block_renderers: renderers,
            image_loader: Some(loader_fn),
            ..Default::default()
        },
    );
    assert_eq!(calls.get(), 2);
    // walk the spec: the widget text, two images (loaded + data URI), one placeholder
    let mut found_widget = false;
    let mut images = 0usize;
    let mut placeholder = false;
    walk(&node, &mut found_widget, &mut images, &mut placeholder);
    assert!(found_widget);
    assert_eq!(images, 2);
    assert!(placeholder);
}

fn walk(node: &Node, widget: &mut bool, images: &mut usize, placeholder: &mut bool) {
    let e = node.element();
    if e.kind == NodeKind::Text && e.content == "widget:{\"id\":3}:chart" {
        *widget = true;
    }
    if e.kind == NodeKind::Image && e.image_id != 0 {
        *images += 1;
    }
    if e.kind == NodeKind::Text && e.content.contains("b.png") {
        *placeholder = true;
    }
    for child in &e.children {
        walk(child, widget, images, placeholder);
    }
}

#[test]
fn markdown_latex_parentheses_and_brackets_delimiters_opt_in() {
    let src = "Area is \\(\\pi r^2\\) here.\n\n\\[\n\\int_0^1 x\\,dx\n\\]\n\nInline \\[a+b\\] too. Escaped \\[x\\] stays with the option off.";
    let on = parser::parse(
        src,
        parser::Options {
            latex_delimiters: true,
            ..Default::default()
        },
    );
    let Block::Paragraph(spans) = &on.blocks[0] else {
        panic!("expected a paragraph");
    };
    let mut math_spans = 0usize;
    for span in spans {
        if span.style.math {
            math_spans += 1;
            assert_eq!(span.text, "\\pi r^2");
        }
    }
    assert_eq!(math_spans, 1);
    let Block::Math(math) = &on.blocks[1] else {
        panic!("expected a math block");
    };
    assert_eq!(math.source, "\\int_0^1 x\\,dx");
    let off = parser::parse(src, parser::Options::default());
    for block in &off.blocks {
        assert!(!matches!(block, Block::Math(_)));
    }
}

struct PanelApp {
    modal: bool,
}

impl App for PanelApp {
    fn render(&mut self) -> Node {
        let base = column(BoxOptions {
            key: Some("doc".into()),
            selectable: true,
            padding: inset(20.0),
            children: zant::nodes![paragraph(
                "Background text that must not be selected through the modal",
                ParagraphOptions::default()
            )],
            ..Default::default()
        });
        if !self.modal {
            return column(BoxOptions {
                grow: 1.0,
                children: zant::nodes![base],
                ..Default::default()
            });
        }
        column(BoxOptions {
            grow: 1.0,
            children: zant::nodes![
                base,
                styled(
                    column(BoxOptions {
                        key: Some("panel".into()),
                        block_pointer: true,
                        align_items: zant::layout::flex::Align::Stretch,
                        children: zant::nodes![column(BoxOptions {
                            key: Some("inner".into()),
                            selectable: true,
                            padding: inset(10.0),
                            align_items: zant::layout::flex::Align::Stretch,
                            children: zant::nodes![paragraph(
                                "Panel text",
                                ParagraphOptions::default()
                            )],
                            ..Default::default()
                        })],
                        ..Default::default()
                    }),
                    Style {
                        absolute: Some(true),
                        left: Some(0.0),
                        top: Some(0.0),
                        width: Some(400.0),
                        height: Some(300.0),
                        background: Some(0xffffffff),
                        ..Default::default()
                    },
                ),
            ],
            ..Default::default()
        })
    }
}

#[test]
fn selection_a_modal_over_selectable_text_blocks_it_text_inside_a_panel_still_selects() {
    let mut runtime = Runtime::init(PanelApp { modal: true }, window(400.0, 300.0));
    let mut renderer = Renderer::create_raster(400, 300).expect("raster renderer");
    frame_at(&mut runtime, &mut renderer, 400.0, 300.0);
    // blank part of the panel, over the background text: nothing selected
    runtime.dispatch(Event::PointerDown(pointer(30.0, 200.0)));
    runtime.dispatch(Event::PointerMove(pointer(380.0, 200.0)));
    runtime.dispatch(Event::PointerUp(pointer(380.0, 200.0)));
    assert!(runtime.sel_root.is_none());
    // the panel's own text selects
    runtime.dispatch(Event::PointerDown(pointer(12.0, 22.0)));
    runtime.dispatch(Event::PointerMove(pointer(300.0, 22.0)));
    runtime.dispatch(Event::PointerUp(pointer(300.0, 22.0)));
    assert_eq!(runtime.selected_text(), "Panel text");
}

fn cache_options(streaming: bool) -> zant::markdown::cache::Options {
    zant::markdown::cache::Options {
        mode: if streaming {
            Mode::Streaming
        } else {
            Mode::Static
        },
        parse: parser::Options {
            latex_delimiters: true,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn first_block_is_math(doc: &Document) -> bool {
    matches!(doc.blocks.first(), Some(Block::Math(_)))
}

#[test]
fn markdown_a_multi_line_formula_with_equals_and_plus_lines_stays_one_block() {
    let mut cache = Cache::new();
    let src = "套正弦和角公式：\n\n\\[\n\\sin75^\\circ\n=\n\\sin45^\\circ\\cos30^\\circ\n+\n\\cos45^\\circ\\sin30^\\circ\n\\]\n\n代入特殊角值：";
    {
        let entries = cache.update(src, cache_options(true));
        assert_eq!(entries.len(), 3);
        let Block::Math(math) = &entries[1].doc.blocks[0] else {
            panic!("expected a math block");
        };
        assert_eq!(
            math.source,
            "\\sin75^\\circ\n=\n\\sin45^\\circ\\cos30^\\circ\n+\n\\cos45^\\circ\\sin30^\\circ"
        );
    }
    // while streaming, an unclosed formula is one growing block too
    let cut = src.find('+').expect("plus") + 2;
    let partial = cache.update(&src[..cut], cache_options(true));
    assert_eq!(partial.len(), 2);
    assert!(first_block_is_math(&partial[1].doc));
}

#[test]
fn markdown_a_streaming_formula_hides_its_half_arrived_closing_delimiter() {
    let cases: [(&str, &str); 3] = [
        // `\]` cut after the backslash
        ("\\[\nf(x) = a + c\\", "f(x) = a + c"),
        // `\\` is a line break
        (
            "\\[\n\\begin{aligned} a &= b \\\\\n&= c \\\\",
            "\\begin{aligned} a &= b \\\\\n&= c \\\\",
        ),
        // `$$` cut after one `$`
        ("$$\nx^2\n$", "x^2"),
    ];
    for (source, expected) in cases {
        let doc = parser::parse(
            source,
            parser::Options {
                latex_delimiters: true,
                ..Default::default()
            },
        );
        let Block::Math(math) = &doc.blocks[0] else {
            panic!("expected a math block for {source:?}");
        };
        assert!(!math.closed);
        assert_eq!(math.source, expected);
    }
    // through the streaming pipeline (remend + block cache) as an app sees it
    let mut cache = Cache::new();
    let entries = cache.update("代回：\n\n\\[\nf(x) = a + c\\", cache_options(true));
    let last = entries.last().expect("entry");
    let Block::Math(math) = &last.doc.blocks[0] else {
        panic!("expected a math block");
    };
    assert_eq!(math.source, "f(x) = a + c");
}
