//! Pure locale normalization, negotiation, and message fallback.
//!
//! Requested preferences are tried in order; an exact, case-insensitive match
//! wins. Otherwise only the same language and compatible maximized scripts match.
//! Likely scripts are zh: Hant in TW/HK/MO, Hans elsewhere; sr: Cyrl;
//! pa: Arab in PK, Guru elsewhere; uz: Arab in AF, Latn elsewhere.
//! Explicit scripts take precedence. Other languages have unknown likely scripts,
//! which match any script of that language; different known scripts never match.
//! Candidates rank by same region, then no region (bare language or language-script),
//! then lexicographic tag order, independently of catalog order. Only after all
//! preferences fail is the default used. Message fallback truncates the selected
//! tag, keeping available parents with compatible scripts, then appends the default
//! exactly once, even when it has a different script or language.

/// Canonical BCP-47 `language[-Script][-REGION]` from a catalog or system language.
///
/// Normalizes ASCII case and accepts `_` separators. POSIX `.charset` and
/// `@modifier` suffixes, variants, extensions, and private use are dropped.
/// Empty, `C`, `POSIX`, `und`, and invalid language subtags return `None`.
pub fn canonical(tag: &str) -> Option<String> {
    let base = tag.trim().split(['.', '@']).next()?;
    let mut parts = base.split(['-', '_']).peekable();
    let language = parts.next()?;
    if !(2..=8).contains(&language.len())
        || !language.bytes().all(|c| c.is_ascii_alphabetic())
        || language.eq_ignore_ascii_case("posix")
        || language.eq_ignore_ascii_case("und")
    {
        return None;
    }
    let mut result = language.to_ascii_lowercase();
    if let Some(script) =
        parts.next_if(|part| part.len() == 4 && part.bytes().all(|c| c.is_ascii_alphabetic()))
    {
        let script = script.to_ascii_lowercase();
        result.push('-');
        result.push_str(&script[..1].to_ascii_uppercase());
        result.push_str(&script[1..]);
    }
    if let Some(region) = parts.next_if(|part| {
        (part.len() == 2 && part.bytes().all(|c| c.is_ascii_alphabetic()))
            || (part.len() == 3 && part.bytes().all(|c| c.is_ascii_digit()))
    }) {
        result.push('-');
        result.push_str(&region.to_ascii_uppercase());
    }
    Some(result)
}

struct Tag<'a> {
    language: &'a str,
    script: Option<&'a str>,
    region: Option<&'a str>,
}

impl<'a> Tag<'a> {
    // Inputs here have already been canonicalized.
    fn parse(tag: &'a str) -> Self {
        let mut parts = tag.split('-').peekable();
        Self {
            language: parts.next().unwrap_or_default(),
            script: parts.next_if(|part| part.len() == 4),
            region: parts.next(),
        }
    }

    fn maximized_script(&self) -> Option<&str> {
        self.script.or(match self.language {
            "zh" => Some(if matches!(self.region, Some("TW" | "HK" | "MO")) {
                "Hant"
            } else {
                "Hans"
            }),
            "sr" => Some("Cyrl"),
            "pa" => Some(if self.region == Some("PK") {
                "Arab"
            } else {
                "Guru"
            }),
            "uz" => Some(if self.region == Some("AF") {
                "Arab"
            } else {
                "Latn"
            }),
            _ => None,
        })
    }

    fn compatible(&self, other: &Self) -> bool {
        self.language == other.language
            && match (self.maximized_script(), other.maximized_script()) {
                (Some(a), Some(b)) => a == b,
                _ => true,
            }
    }
}

/// Return the first acceptable app locale for ordered user preferences, or default.
/// `available` and `default` must be canonical tags.
pub fn negotiate(requested: &[&str], available: &[&str], default: &str) -> String {
    for request in requested {
        let Some(request) = canonical(request) else {
            continue;
        };
        if let Some(exact) = available
            .iter()
            .filter(|tag| tag.eq_ignore_ascii_case(&request))
            .min()
        {
            return (*exact).to_owned();
        }
        let request = Tag::parse(&request);
        let best = available
            .iter()
            .filter(|tag| request.compatible(&Tag::parse(tag)))
            .min_by_key(|tag| {
                let candidate = Tag::parse(tag);
                (
                    candidate.region != request.region,
                    candidate.region.is_some(),
                    **tag,
                )
            });
        if let Some(best) = best {
            return (*best).to_owned();
        }
    }
    default.to_owned()
}

/// Message locales, best first: selected, available compatible truncated parents,
/// then default, with no duplicates. All inputs must be canonical tags.
pub fn fallback_chain(selected: &str, available: &[&str], default: &str) -> Vec<String> {
    let mut chain = vec![selected.to_owned()];
    let selected_tag = Tag::parse(selected);
    let mut parent = selected;
    while let Some((prefix, _)) = parent.rsplit_once('-') {
        parent = prefix;
        if available.contains(&parent) && selected_tag.compatible(&Tag::parse(parent)) {
            chain.push(parent.to_owned());
        }
    }
    chain.retain(|tag| tag != default);
    chain.push(default.to_owned());
    chain
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_tags() {
        for (input, expected) in [
            ("EN-us", Some("en-US")),
            ("zh-hans-cn", Some("zh-Hans-CN")),
            ("zh_CN.UTF-8", Some("zh-CN")),
            ("sr_RS.UTF-8@latin", Some("sr-RS")),
            ("en-US-u-ca-gregory", Some("en-US")),
            ("sr-Latn-RS", Some("sr-Latn-RS")),
            ("sl-rozaj-biske", Some("sl")),
            ("de-1901", Some("de")),
            ("en-x-us", Some("en")),
            ("es_419", Some("es-419")),
            ("C", None),
            ("C.UTF-8", None),
            ("POSIX", None),
            ("", None),
            ("und", None),
            ("UND-US", None),
            ("123", None),
            ("中文", None),
        ] {
            assert_eq!(canonical(input).as_deref(), expected, "{input}");
        }
    }

    #[test]
    fn negotiate_preferences() {
        let cases: &[(&[&str], &[&str], &str, &str)] = &[
            (&["zh-Hans-CN"], &["en", "zh-CN"], "en", "zh-CN"),
            (&["zh-TW"], &["en", "zh-CN"], "en", "en"),
            (&["zh-HK"], &["zh-TW", "zh-CN"], "en", "zh-TW"),
            (&["zh"], &["zh-TW", "zh-CN"], "en", "zh-CN"),
            (&["pt-PT"], &["pt-BR", "en"], "en", "pt-BR"),
            (&["en-AU"], &["en-GB", "en-US"], "en", "en-GB"),
            (&["fr-CA", "de"], &["de", "en"], "en", "de"),
            (&[], &["de", "en"], "en", "en"),
            (&["C", "und", "DE_de"], &["de", "en"], "en", "de"),
            (&["EN-us"], &["en", "en-US"], "en", "en-US"),
            (&["zh-Hant"], &["zh-HK", "zh-TW"], "en", "zh-HK"),
            (
                &["zh-CN"],
                &["zh", "zh-Hans", "zh-Hans-CN"],
                "en",
                "zh-Hans-CN",
            ),
            (&["en-AU"], &["en", "en-GB"], "en", "en"),
            (&["zh-CN"], &["zh-Hans", "zh-SG"], "en", "zh-Hans"),
            (&["sr-Latn"], &["sr", "sr-RS"], "en", "en"),
            (&["pa-PK"], &["pa", "pa-Arab"], "en", "pa-Arab"),
            (&["uz-AF"], &["uz", "uz-Arab"], "en", "uz-Arab"),
            (&["sr"], &["sr-Latn", "sr-Cyrl"], "en", "sr-Cyrl"),
            (&["pa"], &["pa-Arab", "pa-Guru"], "en", "pa-Guru"),
            (&["uz"], &["uz-Arab", "uz-Latn"], "en", "uz-Latn"),
            (&["en"], &["en-Latn"], "de", "en-Latn"),
            (&["en-Latn"], &["en-Cyrl"], "de", "de"),
        ];
        for &(requested, available, default, expected) in cases {
            assert_eq!(negotiate(requested, available, default), expected);
            let mut shuffled = available.to_vec();
            for shift in 0..available.len() {
                shuffled.rotate_left(shift);
                shuffled.reverse();
                assert_eq!(negotiate(requested, &shuffled, default), expected);
            }
        }
    }

    #[test]
    fn fallback_parents() {
        let cases: &[(&str, &[&str], &str, &[&str])] = &[
            (
                "en-GB",
                &["en", "en-GB", "zh-CN"],
                "zh-CN",
                &["en-GB", "en", "zh-CN"],
            ),
            ("zh-TW", &["zh", "zh-TW", "en"], "en", &["zh-TW", "en"]),
            (
                "zh-Hant-TW",
                &["zh-Hant", "zh"],
                "en",
                &["zh-Hant-TW", "zh-Hant", "en"],
            ),
            (
                "sr-Latn-RS",
                &["sr-Latn", "sr"],
                "en",
                &["sr-Latn-RS", "sr-Latn", "en"],
            ),
            ("en-US", &["en", "en"], "en", &["en-US", "en"]),
            ("en", &["en"], "en", &["en"]),
            ("en-US", &[], "de", &["en-US", "de"]),
        ];
        for &(selected, available, default, expected) in cases {
            assert_eq!(fallback_chain(selected, available, default), expected);
        }
    }
}
