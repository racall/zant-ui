//! An app's translations: `locales/<tag>.json` compiled into the static
//! description the runtime installs (inlined into `zant_app.rs`), the key
//! table `t(…)` calls are checked against, and the app's
//! `zant.i18n.d.ts` text for the editor.
//!
//! `app.config.json` may set `"i18n": { "default_locale": "en",
//! "remember": true }`. Without it, a single catalog is the default and
//! several catalogs are an error asking for one; no `locales/` (or an empty
//! one) means the app has no translations.

use std::path::{Path, PathBuf};

use zant_tsx::codegen::i18n_calls::Index;
use zant_tsx::error::render_diag;
use zant_tsx::i18n::{catalog, emit, types};

use crate::{Error, read_text};

/// What the build needs from an app's `locales/`.
pub struct AppI18n {
    pub compiled: emit::Compiled,
    /// `i18n.remember` (default true): keep `setLocale`'s choice.
    pub remember: bool,
    /// The catalog files (rerun-if-changed).
    pub files: Vec<PathBuf>,
}

impl AppI18n {
    /// The key table for the TSX compiler.
    pub fn index(&self) -> Index {
        Index {
            keys: self.compiled.keys.clone(),
            locales: self.compiled.locales.clone(),
            default: self.compiled.locales[self.compiled.default].clone(),
        }
    }

    /// The app's `zant.i18n.d.ts`.
    pub fn declarations(&self) -> String {
        let locales: Vec<&str> = self.compiled.locales.iter().map(String::as_str).collect();
        let messages: Vec<(&str, &_)> = self
            .compiled
            .keys
            .iter()
            .map(|(key, contract)| (key.as_str(), contract))
            .collect();
        types::declarations(&locales, &messages)
    }
}

struct Settings {
    default: Option<String>,
    remember: bool,
}

fn settings(app_root: &Path) -> Result<Settings, Error> {
    let path = app_root.join("app.config.json");
    let mut settings = Settings {
        default: None,
        remember: true,
    };
    if !path.is_file() {
        return Ok(settings);
    }
    let text = read_text(&path)?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| Error::Config {
        path: path.clone(),
        message: error.to_string(),
    })?;
    let Some(i18n) = value.get("i18n") else {
        return Ok(settings);
    };
    let invalid = |message: &str| Error::Config {
        path: path.clone(),
        message: format!("i18n: {message}"),
    };
    let Some(i18n) = i18n.as_object() else {
        return Err(invalid(
            "expected an object such as { \"default_locale\": \"en\" }",
        ));
    };
    if let Some(default) = i18n.get("default_locale") {
        let tag = default
            .as_str()
            .ok_or_else(|| invalid("default_locale is a language tag such as \"en\""))?;
        let tag = catalog::canonical_tag(tag).map_err(|message| invalid(&message))?;
        settings.default = Some(tag);
    }
    if let Some(remember) = i18n.get("remember") {
        settings.remember = remember
            .as_bool()
            .ok_or_else(|| invalid("remember is true or false"))?;
    }
    Ok(settings)
}

/// `path:line:col: error: …` for a catalog error (plus the first occurrence
/// of a duplicate).
fn located(sources: &[(String, String)], error: &catalog::CatalogError) -> Error {
    let render =
        |path: &str, diag: &zant_tsx::error::Diag| match sources.iter().find(|(p, _)| p == path) {
            Some((_, src)) => render_diag(path, src, diag),
            None => format!("{path}: error: {}\n", diag.msg),
        };
    let mut text = render(&error.path, &error.diag);
    if let Some((path, diag)) = &error.related {
        text.push_str(&render(path, diag));
    }
    Error::Compile(text)
}

/// Compiles `<app_root>/locales/*.json`; `None` when there are none.
pub fn load(app_root: &Path) -> Result<Option<AppI18n>, Error> {
    let dir = app_root.join("locales");
    let files = catalog::discover(&dir).map_err(|error| located(&[], &error))?;
    if files.is_empty() {
        return Ok(None);
    }
    let settings = settings(app_root)?;
    let mut sources: Vec<(String, String)> = Vec::new();
    let mut catalogs = Vec::new();
    for file in &files {
        let relative = format!(
            "locales/{}",
            file.file_name().unwrap_or_default().to_string_lossy()
        );
        let src = read_text(file)?;
        sources.push((relative.clone(), src.clone()));
        let parsed = catalog::parse(&relative, &src).map_err(|error| located(&sources, &error))?;
        catalogs.push((parsed, src));
    }
    let default = match settings.default {
        Some(default) => default,
        None if catalogs.len() == 1 => catalogs[0].0.locale.clone(),
        None => {
            let tags: Vec<&str> = catalogs
                .iter()
                .map(|(catalog, _)| catalog.locale.as_str())
                .collect();
            return Err(Error::Message(format!(
                "locales/ has several languages ({}); choose the default in app.config.json: \"i18n\": {{ \"default_locale\": \"{}\" }}",
                tags.join(", "),
                tags[0]
            )));
        }
    };
    let compiled = emit::compile(&catalogs, &default).map_err(|error| located(&sources, &error))?;
    Ok(Some(AppI18n {
        compiled,
        remember: settings.remember,
        files,
    }))
}

/// The app's `zant.i18n.d.ts` text (for `zant` to refresh next to
/// `zant.d.ts`); `None` without translations.
pub fn declarations(app_root: &Path) -> Result<Option<String>, Error> {
    Ok(load(app_root)?.map(|i18n| i18n.declarations()))
}
