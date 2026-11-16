//! Discover locale files and flatten located translation strings.

use super::json::{self, Kind, Member};
use crate::error::Diag;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub value: String,
    pub key_loc: u32,
    pub value_loc: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog {
    pub locale: String,
    pub path: String,
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogError {
    pub path: String,
    pub diag: Diag,
    pub related: Option<(String, Diag)>,
}

impl CatalogError {
    fn new(path: &str, loc: u32, msg: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            diag: Diag {
                loc,
                msg: msg.into(),
            },
            related: None,
        }
    }

    fn related(mut self, path: &str, loc: u32) -> Self {
        self.related = Some((
            path.into(),
            Diag {
                loc,
                msg: "first occurrence is here".into(),
            },
        ));
        self
    }
}

/// Normalize a catalog filename: the runtime's `canonical` (one
/// normalization for files and system languages), strict about anything it
/// would drop (variants, extensions, charsets).
pub fn canonical_tag(stem: &str) -> Result<String, String> {
    let invalid =
        || "expected language[-Script][-REGION], such as `en-US` or `zh-Hans-CN`".to_string();
    if stem.contains('_') {
        let replacement = stem.replace('_', "-");
        let suggestion = canonical_tag(&replacement).unwrap_or(replacement);
        return Err(format!("use `{suggestion}` with `-` separators"));
    }
    match zant::i18n::locale::canonical(stem) {
        // 2–3 letter languages (4 is reserved; 5–8 are rare registrations
        // that no CLDR plural data covers)
        Some(tag)
            if tag.eq_ignore_ascii_case(stem)
                && tag
                    .split('-')
                    .next()
                    .is_some_and(|language| language.len() <= 3) =>
        {
            Ok(tag)
        }
        _ => Err(invalid()),
    }
}

fn locale(path: &Path) -> Result<String, CatalogError> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "expected a UTF-8 locale filename".to_string())
        .and_then(canonical_tag)
        .map_err(|msg| CatalogError::new(&path.to_string_lossy(), 0, msg))
}

/// Return direct, visible `.json` files ordered by canonical locale tag.
pub fn discover(dir: &Path) -> Result<Vec<PathBuf>, CatalogError> {
    let failure =
        |err: std::io::Error| CatalogError::new(&dir.to_string_lossy(), 0, err.to_string());
    let children = match std::fs::read_dir(dir) {
        Ok(children) => children,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(failure(err)),
    };
    let mut paths = Vec::new();
    for child in children {
        let child = child.map_err(&failure)?;
        let path = child.path();
        if path
            .file_name()
            .is_some_and(|s| s.to_string_lossy().starts_with('.'))
            || path.extension().is_none_or(|s| s != "json")
        {
            continue;
        }
        if child.file_type().map_err(&failure)?.is_file() {
            paths.push(path);
        }
    }
    order_paths(paths)
}

fn order_paths(mut paths: Vec<PathBuf>) -> Result<Vec<PathBuf>, CatalogError> {
    paths.sort();
    let mut locales: BTreeMap<String, PathBuf> = BTreeMap::new();
    for path in paths {
        let tag = locale(&path)?;
        if let Some(first) = locales.get(&tag) {
            return Err(CatalogError::new(
                &path.to_string_lossy(),
                0,
                format!(
                    "duplicate locale `{tag}` in `{}` and `{}`",
                    first.display(),
                    path.display()
                ),
            )
            .related(&first.to_string_lossy(), 0));
        }
        locales.insert(tag, path);
    }
    Ok(locales.into_values().collect())
}

/// Flatten a catalog; offsets refer to quotes in the original UTF-8 source.
pub fn parse(path: &str, src: &str) -> Result<Catalog, CatalogError> {
    if src.len() > 4 * 1024 * 1024 {
        return Err(CatalogError::new(path, 0, "catalog file exceeds 4 MiB"));
    }
    let locale = locale(Path::new(path))?;
    let root = json::parse(src).map_err(|diag| CatalogError {
        path: path.into(),
        diag,
        related: None,
    })?;
    let Kind::Object(members) = root.kind else {
        return Err(CatalogError::new(
            path,
            root.loc,
            "catalog root must be an object",
        ));
    };
    let mut flatten = Flatten {
        path,
        entries: BTreeMap::new(),
        groups: BTreeMap::new(),
    };
    flatten.object("", members)?;
    Ok(Catalog {
        locale,
        path: path.into(),
        entries: flatten.entries.into_values().collect(),
    })
}

struct Flatten<'a> {
    path: &'a str,
    entries: BTreeMap<String, Entry>,
    // Each prefix points to the first message that established the group.
    groups: BTreeMap<String, u32>,
}

impl Flatten<'_> {
    fn object(&mut self, prefix: &str, members: Vec<Member>) -> Result<(), CatalogError> {
        let mut seen = BTreeMap::new();
        for member in members {
            let loc = member.key_loc;
            if member.key.is_empty() {
                return Err(CatalogError::new(
                    self.path,
                    loc,
                    "translation keys must be non-empty",
                ));
            }
            let key = if prefix.is_empty() {
                member.key.clone()
            } else {
                format!("{prefix}.{}", member.key)
            };
            if key.len() > 256 {
                return Err(CatalogError::new(
                    self.path,
                    loc,
                    "translation key exceeds 256 bytes",
                ));
            }
            if let Some(first) = seen.insert(member.key, loc) {
                return Err(
                    CatalogError::new(self.path, loc, format!("duplicate key `{key}`"))
                        .related(self.path, first),
                );
            }
            match member.value.kind {
                Kind::Object(children) => self.object(&key, children)?,
                Kind::String(value) => self.message(Entry {
                    key,
                    value,
                    key_loc: loc,
                    value_loc: member.value.loc,
                })?,
                _ => {
                    return Err(CatalogError::new(
                        self.path,
                        member.value.loc,
                        "translation values must be strings",
                    ));
                }
            }
        }
        Ok(())
    }

    fn message(&mut self, entry: Entry) -> Result<(), CatalogError> {
        let error =
            |msg, first| CatalogError::new(self.path, entry.key_loc, msg).related(self.path, first);
        if let Some(first) = self.entries.get(&entry.key) {
            return Err(error(
                format!("duplicate key `{}`", entry.key),
                first.key_loc,
            ));
        }
        if let Some(first) = self.groups.get(&entry.key) {
            return Err(error(
                format!("`{}` is both a message and a group", entry.key),
                *first,
            ));
        }
        for (at, _) in entry.key.match_indices('.') {
            let prefix = &entry.key[..at];
            if let Some(first) = self.entries.get(prefix) {
                return Err(error(
                    format!("`{prefix}` is both a message and a group"),
                    first.key_loc,
                ));
            }
        }
        if entry.value.len() > 64 * 1024 {
            return Err(CatalogError::new(
                self.path,
                entry.value_loc,
                "translation value exceeds 64 KiB",
            ));
        }
        if self.entries.len() == 20_000 {
            return Err(CatalogError::new(
                self.path,
                entry.key_loc,
                "catalog exceeds 20000 entries",
            ));
        }
        for (at, _) in entry.key.match_indices('.') {
            self.groups
                .entry(entry.key[..at].into())
                .or_insert(entry.key_loc);
        }
        self.entries.insert(entry.key.clone(), entry);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_use_canonical_case_and_reject_unsupported_shapes() {
        for (input, expected) in [
            ("en", "en"),
            ("EN-us", "en-US"),
            ("zh-hans-cn", "zh-Hans-CN"),
            ("es-419", "es-419"),
            ("DEU-latn", "deu-Latn"),
        ] {
            assert_eq!(canonical_tag(input).unwrap(), expected);
        }
        assert!(canonical_tag("zh_CN").unwrap_err().contains("use `zh-CN`"));
        for tag in [
            "",
            "e",
            "engl",
            "en-US-x-foo",
            "en-",
            "en--US",
            "en-1234",
            "éé",
            "en-US-Latn",
        ] {
            assert!(
                canonical_tag(tag)
                    .unwrap_err()
                    .contains("language[-Script][-REGION]"),
                "{tag}"
            );
        }
    }

    #[test]
    fn flat_and_nested_entries_are_equivalent_and_sorted() {
        let flat = parse("EN-us.json", r#"{"z":"last","a.b.c":"first"}"#).unwrap();
        let nested = parse(
            "en-US.json",
            r#"{"a":{"b.c":"first"},"empty":{},"z":"last"}"#,
        )
        .unwrap();
        let pairs = |c: Catalog| {
            c.entries
                .into_iter()
                .map(|e| (e.key, e.value))
                .collect::<Vec<_>>()
        };
        assert_eq!(flat.locale, "en-US");
        assert_eq!(pairs(flat), pairs(nested));
        assert_eq!(parse("en.json", "{}").unwrap().entries.len(), 0);
    }

    #[test]
    fn duplicate_forms_point_to_second_and_first_keys() {
        for (src, second, first) in [
            (r#"{"a":"x","a":"y"}"#, 9, 1),
            (r#"{"a.b":"x","a":{"b":"y"}}"#, 16, 1),
            (r#"{"a":{"b":"x"},"a.b":"y"}"#, 15, 6),
            (r#"{"a":{},"a":{}}"#, 8, 1),
        ] {
            let err = parse("en.json", src).unwrap_err();
            assert_eq!(err.diag.loc, second, "{src}");
            assert_eq!(err.related.unwrap().1.loc, first, "{src}");
        }
    }

    #[test]
    fn canonical_duplicates_are_checked_independently_of_filesystem_case() {
        let err = order_paths(vec!["en-US.json".into(), "EN-us.json".into()]).unwrap_err();
        assert!(err.diag.msg.contains("EN-us.json"));
        assert!(err.diag.msg.contains("en-US.json"));
        assert_eq!(err.path, "en-US.json");
        assert_eq!(err.related.unwrap().0, "EN-us.json");
    }

    #[test]
    fn message_group_conflicts_work_in_both_orders() {
        for src in [
            r#"{"home":"x","home.title":"y"}"#,
            r#"{"home.title":"y","home":"x"}"#,
        ] {
            let err = parse("en.json", src).unwrap_err();
            assert_eq!(err.diag.msg, "`home` is both a message and a group");
            assert_eq!(err.related.unwrap().1.loc, 1);
            assert!(err.diag.loc > 1);
        }
    }

    #[test]
    fn non_string_leaves_report_the_value_offset() {
        for leaf in ["[]", "42", "true", "false", "null"] {
            let err = parse("en.json", &format!("{{\"a\": {leaf}}}")).unwrap_err();
            assert_eq!(err.diag.loc, 6);
            assert_eq!(err.diag.msg, "translation values must be strings");
        }
        assert!(
            parse("en.json", "[]")
                .unwrap_err()
                .diag
                .msg
                .contains("root")
        );
        assert_eq!(parse("en.json", r#"{"":"x"}"#).unwrap_err().diag.loc, 1);
    }
}
