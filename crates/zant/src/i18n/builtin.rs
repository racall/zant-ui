//! The framework's built-in strings (`zant.*` keys): the default labels of
//! Markdown, the Markdown editor, `PdfViewer`, `PdfPreview` and
//! `HotkeyInput`. This table is the only place the framework keeps
//! translations; component defaults and the (deprecated) Chinese presets are
//! built from it.
//!
//! Defaults follow the app's selected locale ([`Language::current`]): `zh`,
//! `zh-CN`, `zh-Hans*` (and other Simplified Chinese tags such as `zh-SG`)
//! select Chinese, everything else English. An app without an i18n catalog
//! stays English. The locale is read from the UI thread's i18n context, so a
//! `setLocale` takes effect at the next render.

use std::cell::Cell;

use super::{context, locale};
use crate::Str;

/// A language the framework ships its strings in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Language {
    #[default]
    English,
    /// Simplified Chinese.
    Chinese,
}

const TAGS: [&str; 2] = ["en", "zh-CN"];

impl Language {
    /// The language for a BCP-47 tag, negotiated like the app's own locales
    /// (`zh-Hans-US` → Chinese; `zh-TW` is Traditional → English).
    pub fn for_tag(tag: &str) -> Language {
        match locale::negotiate(&[tag], &TAGS, TAGS[0]).as_str() {
            "zh-CN" => Language::Chinese,
            _ => Language::English,
        }
    }

    /// The language of the app's selected locale (English without a catalog).
    /// Cached per locale revision: cheap enough to call in every render.
    pub fn current() -> Language {
        thread_local! {
            static CACHED: Cell<(u64, Language)> = const { Cell::new((0, Language::English)) };
        }
        let revision = context::revision();
        if revision == 0 {
            return Language::English;
        }
        CACHED.with(|cached| {
            let (seen, language) = cached.get();
            if seen == revision {
                return language;
            }
            let language = Language::for_tag(&context::locale());
            cached.set((revision, language));
            language
        })
    }

    /// The canonical tag (`en`, `zh-CN`).
    pub fn tag(self) -> &'static str {
        match self {
            Language::English => TAGS[0],
            Language::Chinese => TAGS[1],
        }
    }
}

/// One built-in string in every language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Text {
    /// `zant.<component>.<name>`.
    pub key: &'static str,
    pub en: &'static str,
    pub zh: &'static str,
}

impl Text {
    /// The string in `language`.
    pub const fn get(self, language: Language) -> &'static str {
        match language {
            Language::English => self.en,
            Language::Chinese => self.zh,
        }
    }

    /// The string in `language`, as a `Str` (no allocation).
    pub const fn str(self, language: Language) -> Str {
        Str::Static(self.get(language))
    }

    /// The string in the current language ([`Language::current`]).
    pub fn current(self) -> Str {
        self.str(Language::current())
    }
}

macro_rules! catalog {
    ($($name:ident = $key:literal, $en:literal, $zh:literal;)*) => {
        $(
            #[doc = concat!("`", $key, "`: ", $en)]
            pub const $name: Text = Text { key: $key, en: $en, zh: $zh };
        )*
        /// Every built-in string, in declaration order.
        pub static ALL: &[Text] = &[$($name),*];
    };
}

catalog! {
    // Markdown (`markdown::Labels`, Streamdown `translations`)
    MARKDOWN_COPY = "zant.markdown.copy", "Copy", "复制";
    MARKDOWN_COPY_MARKDOWN = "zant.markdown.copyMarkdown", "Copy Markdown", "复制 Markdown";
    MARKDOWN_COPY_CSV = "zant.markdown.copyCsv", "Copy CSV", "复制 CSV";
    MARKDOWN_DOWNLOAD = "zant.markdown.download", "Download", "下载";
    MARKDOWN_FOOTNOTES = "zant.markdown.footnotes", "Footnotes", "脚注";
    MARKDOWN_IMAGE = "zant.markdown.image", "Image", "图片";
    MARKDOWN_LOADING_IMAGE = "zant.markdown.loadingImage", "Loading image…", "图片加载中…";
    MARKDOWN_OPEN_LINK = "zant.markdown.openLink", "Open link", "打开链接";
    MARKDOWN_COPY_LINK = "zant.markdown.copyLink", "Copy link", "复制链接";
    MARKDOWN_CANCEL = "zant.markdown.cancel", "Cancel", "取消";
    MARKDOWN_LINK_TITLE = "zant.markdown.linkTitle", "Open external link?", "打开外部链接？";
    MARKDOWN_LINK_MESSAGE = "zant.markdown.linkMessage",
        "You're about to visit an external website.", "即将访问外部网站，请确认链接可信。";
    MARKDOWN_UNSUPPORTED_DIAGRAM = "zant.markdown.unsupportedDiagram",
        "Diagram type not supported", "暂不支持此图表类型";

    // Markdown editor (`markdown::EditorLabels`)
    EDITOR_EDIT = "zant.editor.edit", "Edit", "编辑";
    EDITOR_SPLIT = "zant.editor.split", "Split", "分栏";
    EDITOR_PREVIEW = "zant.editor.preview", "Preview", "预览";
    EDITOR_BOLD = "zant.editor.bold", "Bold", "粗体";
    EDITOR_ITALIC = "zant.editor.italic", "Italic", "斜体";
    EDITOR_STRIKETHROUGH = "zant.editor.strikethrough", "Strikethrough", "删除线";
    EDITOR_INLINE_CODE = "zant.editor.inlineCode", "Inline code", "行内代码";
    EDITOR_LINK = "zant.editor.link", "Link", "链接";
    EDITOR_HEADING = "zant.editor.heading", "Heading", "标题";
    EDITOR_QUOTE = "zant.editor.quote", "Quote", "引用";
    EDITOR_BULLETED_LIST = "zant.editor.bulletedList", "Bulleted list", "无序列表";
    EDITOR_NUMBERED_LIST = "zant.editor.numberedList", "Numbered list", "有序列表";
    EDITOR_TASK_LIST = "zant.editor.taskList", "Task list", "任务列表";
    EDITOR_CODE_BLOCK = "zant.editor.codeBlock", "Code block", "代码块";
    EDITOR_INLINE_MATH = "zant.editor.inlineMath", "Inline math", "行内公式";
    EDITOR_MATH_BLOCK = "zant.editor.mathBlock", "Math block", "公式块";
    EDITOR_DIVIDER = "zant.editor.divider", "Divider", "分隔线";
    EDITOR_TABLE = "zant.editor.table", "Table", "表格";

    // PDF reader (`pdf::Labels`, `PdfViewer`)
    PDF_SEARCH = "zant.pdf.search", "Search", "搜索";
    PDF_NO_RESULTS = "zant.pdf.noResults", "No results", "无结果";
    PDF_PREVIOUS_PAGE = "zant.pdf.previousPage", "Previous page", "上一页";
    PDF_NEXT_PAGE = "zant.pdf.nextPage", "Next page", "下一页";
    PDF_ZOOM_IN = "zant.pdf.zoomIn", "Zoom in", "放大";
    PDF_ZOOM_OUT = "zant.pdf.zoomOut", "Zoom out", "缩小";
    PDF_FIT_WIDTH = "zant.pdf.fitWidth", "Fit width", "适合宽度";
    PDF_FIT_PAGE = "zant.pdf.fitPage", "Fit page", "适合页面";
    PDF_PREVIOUS_MATCH = "zant.pdf.previousMatch", "Previous match", "上一个";
    PDF_NEXT_MATCH = "zant.pdf.nextMatch", "Next match", "下一个";
    PDF_COPY = "zant.pdf.copy", "Copy", "复制";
    PDF_CANNOT_OPEN = "zant.pdf.cannotOpen", "Cannot open this PDF", "无法打开此 PDF";
    PDF_PASSWORD = "zant.pdf.password", "This PDF is password protected", "此 PDF 受密码保护";
    PDF_HIGHLIGHT = "zant.pdf.highlight", "Highlight", "高亮";
    PDF_UNDERLINE = "zant.pdf.underline", "Underline", "下划线";
    PDF_STRIKEOUT = "zant.pdf.strikeout", "Strikethrough", "删除线";
    PDF_PEN = "zant.pdf.pen", "Pen", "画笔";
    PDF_NOTE = "zant.pdf.note", "Note", "便签";
    PDF_SELECT = "zant.pdf.select", "Select", "选择";
    PDF_EMPTY = "zant.pdf.empty", "Choose a PDF to read", "选择要阅读的 PDF";
    PDF_LOADING = "zant.pdf.loading", "Opening PDF…", "正在打开 PDF…";
    PDF_ERROR = "zant.pdf.error", "This PDF can't be opened", "无法打开此 PDF";

    // Single-page preview (`ui::pdf_preview`)
    PDF_PREVIEW_UNAVAILABLE = "zant.pdfPreview.unavailable", "Can't preview this PDF", "无法预览此 PDF";
    PDF_PREVIEW_COPY_TEXT = "zant.pdfPreview.copyText", "Copy text", "复制文本";

    // Shortcut recorder (`ui::hotkey_input`)
    HOTKEY_PLACEHOLDER = "zant.hotkeyInput.placeholder", "Click to set a shortcut", "点击设置快捷键";
    HOTKEY_RECORDING = "zant.hotkeyInput.recording", "Press a shortcut…", "请按下快捷键…";
    HOTKEY_CLEAR = "zant.hotkeyInput.clear", "Clear", "清除";
}

/// The built-in string `key` (`zant.pdf.search`), if there is one.
pub fn lookup(key: &str) -> Option<Text> {
    ALL.iter().copied().find(|text| text.key == key)
}

/// The built-in string `key` in the current language; unknown keys show the
/// key itself (like a missing `t()` message).
pub fn text(key: &str) -> Str {
    match lookup(key) {
        Some(text) => text.current(),
        None => Str::copy(key),
    }
}

/// Test helpers: an app catalog with `en` (default), `ja` and `zh-CN`.
#[cfg(test)]
pub(crate) mod testing {
    use super::super::catalog::{Description, LocaleDesc, Op};
    use super::super::context::{self, Setup};

    pub static APP: Description = Description {
        keys: &["hello"],
        locales: &[
            LocaleDesc {
                tag: "en",
                name: "English",
                plural: &[],
                messages: &[Some(&[Op::Text("Hello")])],
            },
            LocaleDesc {
                tag: "ja",
                name: "日本語",
                plural: &[],
                messages: &[Some(&[Op::Text("こんにちは")])],
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

    /// Installs [`APP`] on this thread (its i18n context is per thread, so
    /// parallel tests do not interfere) and selects `tag`.
    pub fn use_locale(tag: &str) {
        context::install_with(&APP, Setup::default(), vec!["en".to_owned()]);
        assert!(context::set_locale_with(tag, Vec::new()), "{tag}");
    }
}

#[cfg(test)]
mod tests {
    use super::testing::use_locale;
    use super::*;

    #[test]
    fn tags_resolve_to_a_language() {
        for tag in ["zh", "zh-CN", "zh-Hans", "zh-Hans-US", "zh_CN", "ZH-sg"] {
            assert_eq!(Language::for_tag(tag), Language::Chinese, "{tag}");
        }
        for tag in [
            "en", "en-GB", "ja", "zh-TW", "zh-Hant", "zh-HK", "", "fr-CA",
        ] {
            assert_eq!(Language::for_tag(tag), Language::English, "{tag}");
        }
        assert_eq!(Language::Chinese.tag(), "zh-CN");
    }

    #[test]
    fn keys_are_unique_namespaced_and_translated() {
        let mut keys: Vec<&str> = ALL.iter().map(|text| text.key).collect();
        keys.sort_unstable();
        let count = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), count, "duplicate key");
        for text in ALL {
            assert!(text.key.starts_with("zant."), "{}", text.key);
            assert!(!text.en.is_empty() && !text.zh.is_empty(), "{}", text.key);
        }
        assert_eq!(lookup("zant.pdf.search"), Some(PDF_SEARCH));
        assert_eq!(&*text("zant.no.such.key"), "zant.no.such.key");
    }

    #[test]
    fn the_current_language_follows_the_app_locale() {
        use_locale("en");
        assert_eq!(Language::current(), Language::English);
        assert_eq!(&*MARKDOWN_COPY.current(), "Copy");
        use_locale("zh-CN");
        assert_eq!(Language::current(), Language::Chinese);
        assert_eq!(&*text("zant.markdown.copy"), "复制");
        // a language the framework does not ship falls back to English
        use_locale("ja");
        assert_eq!(Language::current(), Language::English);
        assert_eq!(&*HOTKEY_CLEAR.current(), "Clear");
    }
}
