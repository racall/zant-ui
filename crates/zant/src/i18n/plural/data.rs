use std::collections::BTreeMap;
use std::sync::OnceLock;

use super::Category;

pub(super) const JSON: &str = include_str!("../../../data/cldr/plurals.json");
type Locales = BTreeMap<String, Vec<(Category, &'static str)>>;

pub(super) fn locales() -> &'static Locales {
    static DATA: OnceLock<Locales> = OnceLock::new();
    DATA.get_or_init(|| {
        // Borrow rule strings directly from the embedded JSON. The pinned CLDR
        // file has no escaped strings, so serde_json can borrow every value.
        #[derive(serde::Deserialize)]
        struct Document<'a> {
            #[serde(borrow)]
            supplemental: Supplemental<'a>,
        }
        #[derive(serde::Deserialize)]
        struct Supplemental<'a> {
            #[serde(rename = "plurals-type-cardinal", borrow)]
            cardinal: BTreeMap<String, BTreeMap<String, &'a str>>,
        }
        let data: Document<'static> = serde_json::from_str(JSON).expect("valid embedded CLDR JSON");
        data.supplemental
            .cardinal
            .into_iter()
            .map(|(locale, rules)| {
                let mut rules: Vec<_> = rules
                    .into_iter()
                    .map(|(key, text)| {
                        let name = key
                            .strip_prefix("pluralRule-count-")
                            .expect("CLDR rule key");
                        (Category::from_str(name).expect("CLDR category"), text)
                    })
                    .collect();
                rules.sort_by_key(|(category, _)| *category);
                (locale, rules)
            })
            .collect()
    })
}

/// Look up embedded CLDR cardinal rule strings for a BCP-47 language tag.
///
/// Tries the exact tag, removes trailing subtags (region before script), then
/// tries the language and `root`. Matching is ASCII case-insensitive. Unicode
/// extensions and private-use subtags do not affect lookup. Returns `None` if
/// CLDR has neither the language nor a root entry.
pub fn cldr_rules(tag: &str) -> Option<Vec<(Category, &'static str)>> {
    let mut parts: Vec<_> = tag.split('-').collect();
    if let Some(extension) = parts.iter().position(|part| part.len() == 1) {
        parts.truncate(extension);
    }
    let data = locales();
    while !parts.is_empty() {
        let candidate = parts.join("-");
        if let Some((_, rules)) = data
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(&candidate))
        {
            return Some(rules.clone());
        }
        parts.pop();
    }
    data.get("root").cloned()
}
