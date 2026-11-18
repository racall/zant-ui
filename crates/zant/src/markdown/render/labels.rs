//! Interface strings of the renderer (Streamdown's `translations`).
//!
//! The defaults come from the framework catalog (`i18n::builtin`) in the
//! app's current language, so `Labels::default()` follows `setLocale`; build
//! the options in `render` (as TSX does) rather than keeping them across a
//! locale change. Labels take part in `Options::hash`, so blocks rendered
//! with the old language rebuild.

use crate::Str;
use crate::i18n::builtin::{self, Language};

/// Streamdown `translations`.
#[derive(Clone, Debug)]
pub struct Labels {
    pub copy: Str,
    pub copy_markdown: Str,
    pub copy_csv: Str,
    pub download: Str,
    pub footnotes: Str,
    pub image: Str,
    pub loading_image: Str,
    pub open_link: Str,
    pub copy_link: Str,
    pub cancel: Str,
    pub link_title: Str,
    pub link_message: Str,
    pub unsupported_diagram: Str,
}

impl Default for Labels {
    /// The labels in the app's current language.
    fn default() -> Self {
        Labels::for_language(Language::current())
    }
}

impl Labels {
    /// The built-in labels in `language`.
    pub fn for_language(language: Language) -> Labels {
        Labels {
            copy: builtin::MARKDOWN_COPY.str(language),
            copy_markdown: builtin::MARKDOWN_COPY_MARKDOWN.str(language),
            copy_csv: builtin::MARKDOWN_COPY_CSV.str(language),
            download: builtin::MARKDOWN_DOWNLOAD.str(language),
            footnotes: builtin::MARKDOWN_FOOTNOTES.str(language),
            image: builtin::MARKDOWN_IMAGE.str(language),
            loading_image: builtin::MARKDOWN_LOADING_IMAGE.str(language),
            open_link: builtin::MARKDOWN_OPEN_LINK.str(language),
            copy_link: builtin::MARKDOWN_COPY_LINK.str(language),
            cancel: builtin::MARKDOWN_CANCEL.str(language),
            link_title: builtin::MARKDOWN_LINK_TITLE.str(language),
            link_message: builtin::MARKDOWN_LINK_MESSAGE.str(language),
            unsupported_diagram: builtin::MARKDOWN_UNSUPPORTED_DIAGRAM.str(language),
        }
    }

    /// The built-in labels for a BCP-47 tag (`"zh-CN"`; languages the
    /// framework does not ship are English).
    pub fn for_locale(tag: &str) -> Labels {
        Labels::for_language(Language::for_tag(tag))
    }

    /// Chinese labels.
    #[deprecated(
        note = "defaults follow the app locale; use `Labels::default()` or `Labels::for_locale(\"zh-CN\")`"
    )]
    pub fn zh() -> Labels {
        Labels::for_language(Language::Chinese)
    }

    /// Every label, in declaration order (hashing, tests).
    pub(crate) fn values(&self) -> [&Str; 13] {
        [
            &self.copy,
            &self.copy_markdown,
            &self.copy_csv,
            &self.download,
            &self.footnotes,
            &self.image,
            &self.loading_image,
            &self.open_link,
            &self.copy_link,
            &self.cancel,
            &self.link_title,
            &self.link_message,
            &self.unsupported_diagram,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(deprecated)]
    fn the_chinese_preset_comes_from_the_catalog() {
        let zh = Labels::zh();
        assert_eq!(&*zh.copy, builtin::MARKDOWN_COPY.zh);
        assert_eq!(&*zh.link_title, "打开外部链接？");
        assert_eq!(&*Labels::for_locale("zh-Hans-SG").download, "下载");
        assert_eq!(&*Labels::for_locale("de").download, "Download");
        // no app catalog on this thread: English
        assert_eq!(&*Labels::default().copy_csv, "Copy CSV");

        let editor = crate::markdown::editor::Labels::zh();
        assert_eq!(&*editor.edit, builtin::EDITOR_EDIT.zh);
        assert_eq!(&*editor.tools[5], "标题");
        assert!(
            editor.tools[0].starts_with("粗体 "),
            "{}",
            &*editor.tools[0]
        );
    }

    #[test]
    fn defaults_follow_the_app_locale_and_change_the_options_hash() {
        use crate::markdown::render::options::Options;
        builtin::testing::use_locale("en");
        let english = Options::default();
        assert_eq!(&*english.labels.copy, "Copy");
        builtin::testing::use_locale("zh-CN");
        let chinese = Options::default();
        assert_eq!(&*chinese.labels.copy, "复制");
        assert_ne!(english.hash(), chinese.hash(), "cached blocks rebuild");
        assert_eq!(&*crate::markdown::editor::Labels::default().preview, "预览");
        // explicit labels are kept in any language
        let explicit = Labels {
            copy: "Kopieren".into(),
            ..Labels::default()
        };
        assert_eq!(&*explicit.copy, "Kopieren");
        assert_eq!(&*explicit.download, "下载");
    }
}
