//! Interface strings of the reader. The defaults come from the framework
//! catalog (`i18n::builtin`) in the app's current language, so
//! `Labels::default()` follows `setLocale` (the reader is rebuilt every
//! render, nothing caches them).

use crate::Str;
use crate::i18n::builtin::{self, Language};

/// Interface strings (`Labels::default()` is the app's current language).
#[derive(Clone)]
pub struct Labels {
    pub search: Str,
    pub no_results: Str,
    pub previous_page: Str,
    pub next_page: Str,
    pub zoom_in: Str,
    pub zoom_out: Str,
    pub fit_width: Str,
    pub fit_page: Str,
    pub previous_match: Str,
    pub next_match: Str,
    pub copy: Str,
    pub cannot_open: Str,
    pub password: Str,
    pub highlight: Str,
    pub underline: Str,
    pub strikeout: Str,
    pub pen: Str,
    pub note: Str,
    pub select: Str,
}

impl Labels {
    /// Chinese labels.
    #[deprecated(
        note = "defaults follow the app locale; use `Labels::default()` or `Labels::for_locale(\"zh-CN\")`"
    )]
    pub const ZH: Labels = Labels::for_language(Language::Chinese);

    /// The built-in labels in `language`.
    pub const fn for_language(language: Language) -> Labels {
        use builtin::*;
        Labels {
            search: PDF_SEARCH.str(language),
            no_results: PDF_NO_RESULTS.str(language),
            previous_page: PDF_PREVIOUS_PAGE.str(language),
            next_page: PDF_NEXT_PAGE.str(language),
            zoom_in: PDF_ZOOM_IN.str(language),
            zoom_out: PDF_ZOOM_OUT.str(language),
            fit_width: PDF_FIT_WIDTH.str(language),
            fit_page: PDF_FIT_PAGE.str(language),
            previous_match: PDF_PREVIOUS_MATCH.str(language),
            next_match: PDF_NEXT_MATCH.str(language),
            copy: PDF_COPY.str(language),
            cannot_open: PDF_CANNOT_OPEN.str(language),
            password: PDF_PASSWORD.str(language),
            highlight: PDF_HIGHLIGHT.str(language),
            underline: PDF_UNDERLINE.str(language),
            strikeout: PDF_STRIKEOUT.str(language),
            pen: PDF_PEN.str(language),
            note: PDF_NOTE.str(language),
            select: PDF_SELECT.str(language),
        }
    }

    /// The built-in labels for a BCP-47 tag (`"zh-CN"`; languages the
    /// framework does not ship are English).
    pub fn for_locale(tag: &str) -> Labels {
        Labels::for_language(Language::for_tag(tag))
    }
}

impl Default for Labels {
    /// The labels in the app's current language.
    fn default() -> Labels {
        Labels::for_language(Language::current())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(deprecated)]
    fn the_chinese_preset_comes_from_the_catalog() {
        assert_eq!(&*Labels::ZH.search, builtin::PDF_SEARCH.zh);
        assert_eq!(&*Labels::ZH.password, "此 PDF 受密码保护");
        assert_eq!(&*Labels::for_locale("zh").fit_page, "适合页面");
        assert_eq!(&*Labels::for_locale("en-US").fit_page, "Fit page");
        // no app catalog on this thread: English
        assert_eq!(&*Labels::default().zoom_in, "Zoom in");
    }
}
