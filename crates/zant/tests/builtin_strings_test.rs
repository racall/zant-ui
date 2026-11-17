//! Framework strings (`i18n::builtin`) follow the app locale at runtime:
//! a `setLocale` re-renders Markdown blocks with the new default labels (the
//! block memo cache is invalidated), explicit labels win, and the PDF
//! reader's placeholder texts switch too.

use zant::components::widgets::{MarkdownOptions, PdfViewerOptions, markdown, pdf_viewer};
use zant::core::node::NodeData;
use zant::i18n::catalog::{Description, LocaleDesc, Op};
use zant::i18n::context::{self, Setup};
use zant::markdown::render::view;
use zant::runtime::Runtime;
use zant::ui;
use zant::ui::app::{App, RunOptions};
use zant::ui::element::Node;
use zant::ui::options::BoxOptions;

static APP: Description = Description {
    keys: &["hello"],
    locales: &[
        LocaleDesc {
            tag: "en",
            name: "English",
            plural: &[],
            messages: &[Some(&[Op::Text("Hello")])],
        },
        LocaleDesc {
            tag: "zh-CN",
            name: "简体中文",
            plural: &[],
            messages: &[Some(&[Op::Text("你好")])],
        },
    ],
    default: 0,
};

/// Installs the app catalog on this test's thread (the i18n context is per
/// thread) with `tag` selected.
fn use_locale(tag: &str) {
    if !context::installed() {
        context::install_with(&APP, Setup::default(), vec!["en".to_owned()]);
    }
    assert!(context::set_locale_with(tag, Vec::new()));
}

struct Doc {
    labels: Option<ui::MarkdownLabels>,
}

impl App for Doc {
    fn render(&mut self) -> Node {
        let mut options = MarkdownOptions {
            source: "# Code\n\n```rust\nlet x = 1;\n```".into(),
            ..Default::default()
        };
        if let Some(labels) = &self.labels {
            options.labels = labels.clone();
        }
        ui::column(BoxOptions {
            children: vec![markdown(0x6931_386e, options)],
            ..Default::default()
        })
    }
}

/// The code block's copy button label in the live tree.
fn copy_label(runtime: &Runtime) -> String {
    runtime
        .tree
        .nodes
        .iter()
        .filter(|node| node.alive)
        .filter(|node| {
            node.key
                .as_deref()
                .is_some_and(|key| key.ends_with("-copy"))
        })
        .find_map(|node| match &node.data {
            NodeData::Button(button) => Some(button.label.to_string()),
            _ => None,
        })
        .expect("a copy button")
}

#[test]
fn markdown_default_labels_switch_with_the_locale_and_rebuild_cached_blocks() {
    use_locale("en");
    let mut runtime = Runtime::init(Doc { labels: None }, RunOptions::default());
    runtime.rebuild_for_test();
    assert_eq!(copy_label(&runtime), "Copy");

    // nothing changed: every block memo is reused
    view::reset_memo_build_count();
    runtime.needs_build = true;
    runtime.rebuild_for_test();
    assert_eq!(view::memo_build_count(), 0);

    // a locale switch invalidates the cached blocks and they pick up the
    // Chinese defaults (no restart, same runtime)
    use_locale("zh-CN");
    view::reset_memo_build_count();
    runtime.needs_build = true;
    runtime.rebuild_for_test();
    assert_eq!(view::memo_build_count(), 2, "both blocks rebuild");
    assert_eq!(copy_label(&runtime), "复制");

    use_locale("en");
    runtime.needs_build = true;
    runtime.rebuild_for_test();
    assert_eq!(copy_label(&runtime), "Copy");
}

#[test]
fn explicit_markdown_labels_win_over_the_catalog() {
    use_locale("zh-CN");
    let labels = ui::MarkdownLabels {
        copy: "Kopieren".into(),
        ..Default::default()
    };
    let mut runtime = Runtime::init(
        Doc {
            labels: Some(labels),
        },
        RunOptions::default(),
    );
    runtime.rebuild_for_test();
    assert_eq!(copy_label(&runtime), "Kopieren");
    use_locale("en");
    runtime.needs_build = true;
    runtime.rebuild_for_test();
    assert_eq!(copy_label(&runtime), "Kopieren");
}

fn pdf_placeholder(options: PdfViewerOptions) -> String {
    ui::begin_frame();
    let node = pdf_viewer(0x7064_6676, options);
    ui::end_frame();
    // facade column → message column → [icon, text]
    let message = node.element().children[0].element().children[1].element();
    message.content.to_string()
}

#[test]
fn pdf_viewer_texts_follow_the_locale_unless_given() {
    use_locale("zh-CN");
    assert_eq!(
        pdf_placeholder(PdfViewerOptions::default()),
        "选择要阅读的 PDF"
    );
    assert_eq!(&*ui::PdfLabels::default().search, "搜索");
    assert_eq!(
        pdf_placeholder(PdfViewerOptions {
            empty_text: "Pick a file".into(),
            ..Default::default()
        }),
        "Pick a file"
    );
    use_locale("en");
    assert_eq!(
        pdf_placeholder(PdfViewerOptions::default()),
        "Choose a PDF to read"
    );
    assert_eq!(&*ui::PdfLabels::default().search, "Search");
}
