//! The app's current language: the installed [`Catalog`], the user's
//! preference (`"system"` or a tag), the selected locale and its fallback
//! chain. It lives on the UI thread in the framework library, so it survives
//! hot reloads (which install the new catalog and keep the preference);
//! [`current_tag`] mirrors the selection for background threads.
//!
//! Selection: an explicit preference that the app has, else the system's
//! preferred languages negotiated against the app's (`locale::negotiate`),
//! else the default. A chosen preference is remembered across launches in
//! the app's data directory unless the app turns `remember` off.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::RwLock;

use super::catalog::{Catalog, Description};
use super::format::{self, Arg};
use super::locale;
use crate::Str;

/// How the app installs its catalog (from `app.config.json`).
#[derive(Clone, Debug, Default)]
pub struct Setup {
    /// Persist the user's choice (`setLocale`) across launches.
    pub remember: bool,
    /// The bundle id, naming the data directory the choice is saved in.
    pub app_id: String,
}

#[derive(Default)]
struct Context {
    catalog: Option<Rc<Catalog>>,
    /// `None`: follow the system.
    preference: Option<String>,
    selected: usize,
    chain: Vec<usize>,
    revision: u64,
    setup: Setup,
}

thread_local! {
    static CONTEXT: RefCell<Context> = RefCell::new(Context::default());
}

static CURRENT: RwLock<String> = RwLock::new(String::new());

const SYSTEM: &str = "system";

fn preference_file(setup: &Setup) -> Option<PathBuf> {
    if !setup.remember || setup.app_id.is_empty() {
        return None;
    }
    #[cfg(test)]
    if let Some(dir) = tests::DATA_DIR.with(|dir| dir.borrow().clone()) {
        return Some(dir.join(&setup.app_id).join("locale"));
    }
    Some(crate::core::paths::app_data_dir(&setup.app_id)?.join("locale"))
}

/// Installs the app's catalog (at startup and after each hot reload) and
/// selects the locale.
pub fn install(description: &Description, setup: Setup) {
    install_with(description, setup, crate::platform::languages::preferred());
}

/// [`install`] with the system's preferred languages given (tests).
pub fn install_with(description: &Description, setup: Setup, system: Vec<String>) {
    let catalog = Rc::new(Catalog::new(description));
    CONTEXT.with(|context| {
        let mut context = context.borrow_mut();
        // the first install reads the saved choice; a hot reload keeps the
        // one in memory
        if context.catalog.is_none()
            && let Some(saved) =
                preference_file(&setup).and_then(|path| std::fs::read_to_string(path).ok())
        {
            let saved = saved.trim();
            context.preference = (saved != SYSTEM && !saved.is_empty()).then(|| saved.to_owned());
        }
        context.catalog = Some(catalog);
        context.setup = setup;
        select(&mut context, &system);
    });
}

fn select(context: &mut Context, system: &[String]) {
    let Some(catalog) = context.catalog.clone() else {
        return;
    };
    let tags = catalog.tags();
    let default = tags.get(catalog.default).copied().unwrap_or_default();
    let wanted = context
        .preference
        .as_deref()
        .and_then(locale::canonical)
        .filter(|tag| tags.contains(&tag.as_str()));
    let tag = match wanted {
        Some(tag) => tag,
        None => {
            let requested: Vec<&str> = system.iter().map(String::as_str).collect();
            locale::negotiate(&requested, &tags, default)
        }
    };
    context.selected = catalog.index_of(&tag).unwrap_or(catalog.default);
    context.chain = locale::fallback_chain(&tag, &tags, default)
        .iter()
        .filter_map(|tag| catalog.index_of(tag))
        .collect();
    context.revision += 1;
    if let Ok(mut current) = CURRENT.write() {
        current.clear();
        current.push_str(&tag);
    }
}

/// Whether an app catalog is installed.
pub fn installed() -> bool {
    CONTEXT.with(|context| context.borrow().catalog.is_some())
}

/// Formats message `key`; a message that cannot be found is a setup bug
/// (no catalog installed, or a catalog of another build), so it is reported
/// once and shows the key name (or nothing without a catalog) instead of
/// leaving the UI silently blank.
fn message(key: u32, args: &[Arg<'_>]) -> Str {
    CONTEXT.with(|context| {
        let context = context.borrow();
        let Some(catalog) = context.catalog.as_ref() else {
            report_once(|| {
                "[zant] i18n: t() ran before the app's message catalog was installed".to_owned()
            });
            return Str::default();
        };
        match catalog.lookup(&context.chain, key as usize) {
            Some((message, locale)) => format::format(message, locale, args),
            None => {
                let name = catalog.keys.get(key as usize).cloned().unwrap_or_default();
                report_once(|| {
                    format!("[zant] i18n: no message for key `{name}` (id {key}) in the installed catalog")
                });
                name
            }
        }
    })
}

fn report_once(text: impl FnOnce() -> String) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static REPORTED: AtomicBool = AtomicBool::new(false);
    if !REPORTED.swap(true, Ordering::Relaxed) {
        eprintln!("{}", text());
    }
}

/// The message `key` (a key id) without arguments.
pub fn text(key: u32) -> Str {
    message(key, &[])
}

/// The message `key` with `args` in its parameter order.
pub fn format(key: u32, args: &[Arg<'_>]) -> Str {
    message(key, args)
}

/// The selected locale's tag (empty when no catalog is installed).
pub fn locale() -> Str {
    CONTEXT.with(|context| {
        let context = context.borrow();
        context
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.locales.get(context.selected))
            .map(|locale| locale.tag.clone())
            .unwrap_or_default()
    })
}

/// The user's choice: `"system"` or a tag.
pub fn preference() -> Str {
    CONTEXT.with(|context| match &context.borrow().preference {
        Some(tag) => Str::from(tag.clone()),
        None => Str::from(SYSTEM),
    })
}

/// The app's locales, sorted by tag.
pub fn locales() -> Vec<Str> {
    CONTEXT.with(|context| {
        context
            .borrow()
            .catalog
            .as_ref()
            .map(|catalog| {
                catalog
                    .locales
                    .iter()
                    .map(|locale| locale.tag.clone())
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// A locale's own name (`简体中文`), or the tag when it is not the app's.
pub fn name(tag: &str) -> Str {
    CONTEXT.with(|context| {
        let context = context.borrow();
        context
            .catalog
            .as_ref()
            .and_then(|catalog| {
                catalog
                    .index_of(tag)
                    .map(|index| catalog.locales[index].name.clone())
            })
            .unwrap_or_else(|| Str::copy(tag))
    })
}

/// Selects `choice` (`"system"` or one of the app's tags, any case) for every
/// window and remembers it. Returns false (changing nothing) for a tag the
/// app does not have.
pub fn set_locale(choice: &str) -> bool {
    set_locale_with(choice, crate::platform::languages::preferred())
}

/// [`set_locale`] with the system's languages given (tests).
pub fn set_locale_with(choice: &str, system: Vec<String>) -> bool {
    let changed = CONTEXT.with(|context| {
        let mut context = context.borrow_mut();
        let Some(catalog) = context.catalog.clone() else {
            return false;
        };
        let preference = if choice == SYSTEM {
            None
        } else {
            match locale::canonical(choice) {
                Some(tag) if catalog.index_of(&tag).is_some() => Some(tag),
                _ => return false,
            }
        };
        if let Some(path) = preference_file(&context.setup) {
            let text = preference.as_deref().unwrap_or(SYSTEM);
            let saved = path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&path, text));
            if let Err(error) = saved {
                eprintln!(
                    "zant: cannot remember the language in {}: {error}",
                    path.display()
                );
            }
        }
        context.preference = preference;
        select(&mut context, &system);
        true
    });
    if changed {
        crate::desktop::invalidate_all();
    }
    changed
}

/// Counts locale changes and catalog installs (cache keys include it).
pub fn revision() -> u64 {
    CONTEXT.with(|context| context.borrow().revision)
}

/// The selected tag, readable from any thread (empty before install).
pub fn current_tag() -> String {
    CURRENT.read().map(|tag| tag.clone()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::super::catalog::{LocaleDesc, Op};
    use super::super::plural::Category;
    use super::*;

    static DESCRIPTION: Description = Description {
        keys: &["hello", "items"],
        locales: &[
            LocaleDesc {
                tag: "en",
                name: "English",
                plural: &[(Category::One, "i = 1 and v = 0")],
                messages: &[
                    Some(&[Op::Text("Hello")]),
                    Some(&[Op::Plural {
                        arg: 0,
                        exact: &[],
                        categories: &[
                            (Category::One, &[Op::Pound, Op::Text(" item")]),
                            (Category::Other, &[Op::Pound, Op::Text(" items")]),
                        ],
                    }]),
                ],
            },
            LocaleDesc {
                tag: "zh-CN",
                name: "简体中文",
                plural: &[],
                messages: &[Some(&[Op::Text("你好")]), None],
            },
        ],
        default: 0,
    };

    fn system(tags: &[&str]) -> Vec<String> {
        tags.iter().map(|tag| (*tag).to_owned()).collect()
    }

    #[test]
    fn the_system_language_is_negotiated_and_messages_fall_back() {
        install_with(
            &DESCRIPTION,
            Setup::default(),
            system(&["zh-Hans-US", "en-US"]),
        );
        assert_eq!(&*locale(), "zh-CN");
        // (`current_tag` is process-wide; parallel tests would race on it)
        assert_eq!(&*text(0), "你好");
        // untranslated in zh-CN: English, with English plural rules
        assert_eq!(&*format(1, &[Arg::Num(1.0)]), "1 item");
        assert_eq!(&*name("zh-CN"), "简体中文");
        assert_eq!(
            locales().iter().map(|tag| &**tag).collect::<Vec<_>>(),
            ["en", "zh-CN"]
        );
    }

    #[test]
    fn a_message_missing_everywhere_shows_its_key() {
        static GAPS: Description = Description {
            keys: &["present", "gone"],
            locales: &[LocaleDesc {
                tag: "en",
                name: "English",
                plural: &[],
                messages: &[Some(&[Op::Text("here")]), None],
            }],
            default: 0,
        };
        install_with(&GAPS, Setup::default(), system(&["en"]));
        assert_eq!(&*text(0), "here");
        assert_eq!(&*text(1), "gone");
        assert_eq!(&*text(9), "");
        install_with(&DESCRIPTION, Setup::default(), system(&["en"]));
    }

    #[test]
    fn choices_switch_and_survive_reinstall() {
        install_with(&DESCRIPTION, Setup::default(), system(&["fr"]));
        assert_eq!(&*locale(), "en");
        let before = revision();
        assert!(set_locale_with("ZH-cn", system(&["fr"])));
        assert_eq!(&*locale(), "zh-CN");
        assert_eq!(&*preference(), "zh-CN");
        assert!(revision() > before);
        assert!(!set_locale_with("de", system(&["fr"])));
        assert_eq!(&*locale(), "zh-CN");
        // a hot reload keeps the choice
        install_with(&DESCRIPTION, Setup::default(), system(&["fr"]));
        assert_eq!(&*locale(), "zh-CN");
        assert!(set_locale_with("system", system(&["fr"])));
        assert_eq!(&*preference(), "system");
        assert_eq!(&*locale(), "en");
    }

    #[test]
    fn memo_hashes_change_with_the_locale() {
        let hash = || {
            let node = crate::ui::memo(7u64, || {
                crate::ui::text("x", crate::ui::TextOptions::default())
            });
            crate::ui::memo_hash(&node).unwrap()
        };
        install_with(&DESCRIPTION, Setup::default(), system(&["en"]));
        let english = hash();
        assert!(set_locale_with("zh-CN", system(&["en"])));
        assert_ne!(hash(), english);
    }

    thread_local! {
        /// Where this thread's tests keep the saved locale instead of the
        /// user's application data directory (which may be read-only, e.g.
        /// in a sandbox, and must not collect test files).
        pub(super) static DATA_DIR: std::cell::RefCell<Option<std::path::PathBuf>> =
            const { std::cell::RefCell::new(None) };
    }

    /// A fresh directory under the system temp dir, removed on drop.
    struct TempDataDir(std::path::PathBuf);

    impl TempDataDir {
        fn new(name: &str) -> TempDataDir {
            let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            DATA_DIR.with(|slot| *slot.borrow_mut() = Some(dir.clone()));
            TempDataDir(dir)
        }
    }

    impl Drop for TempDataDir {
        fn drop(&mut self) {
            DATA_DIR.with(|slot| *slot.borrow_mut() = None);
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_remembered_choice_is_read_on_the_first_install() {
        let data = TempDataDir::new("zant-i18n-test");
        let setup = Setup {
            remember: true,
            app_id: "dev.zant.i18n-test".to_owned(),
        };
        let file = preference_file(&setup).expect("a preference file");
        assert!(file.starts_with(&data.0), "{}", file.display());
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "zh-CN\n").unwrap();
        install_with(&DESCRIPTION, setup.clone(), system(&["en"]));
        assert_eq!(&*locale(), "zh-CN");
        assert!(set_locale_with("en", system(&["en"])));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "en");
    }
}
