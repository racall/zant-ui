//! Compile located locale catalogs into the runtime's static description.

use super::{catalog, contract, json, message, names};
use crate::error::Diag;
use std::collections::BTreeMap;
use std::fmt::Write;

#[derive(Debug)]
pub struct Compiled {
    /// Rust source of one `pub static DESCRIPTION` item.
    pub rust: String,
    /// Sorted default-catalog keys and contracts; the index is the key id.
    pub keys: Vec<(String, contract::Contract)>,
    /// Sorted canonical locale tags.
    pub locales: Vec<String>,
    /// The default locale's index.
    pub default: usize,
    /// One line per missing translation.
    pub warnings: Vec<String>,
}

fn error(path: &str, loc: u32, msg: impl Into<String>) -> catalog::CatalogError {
    catalog::CatalogError {
        path: path.into(),
        diag: Diag {
            loc,
            msg: msg.into(),
        },
        related: None,
    }
}

/// Compile parsed catalogs, using their source text to locate ICU diagnostics.
pub fn compile(
    catalogs: &[(catalog::Catalog, String)],
    default: &str,
) -> Result<Compiled, catalog::CatalogError> {
    let mut catalogs: Vec<_> = catalogs.iter().collect();
    catalogs.sort_by(|a, b| a.0.locale.cmp(&b.0.locale));
    let locales: Vec<_> = catalogs.iter().map(|c| c.0.locale.clone()).collect();
    let Some(default_index) = locales.iter().position(|tag| tag == default) else {
        // catalog paths use `/` on every platform (`locales/en.json`)
        let dir = catalogs
            .first()
            .and_then(|c| c.0.path.rsplit_once(['/', '\\']).map(|(dir, _)| dir))
            .unwrap_or("locales");
        return Err(error(
            &format!("{dir}/{default}.json"),
            0,
            format!(
                "default language `{default}` has no catalog; available languages: {}",
                locales.join(", ")
            ),
        ));
    };
    let mut parsed = Vec::new();
    for (catalog, src) in &catalogs {
        let mut messages = BTreeMap::new();
        for entry in &catalog.entries {
            // Reserved framework overrides are intentionally ignored until supported.
            if entry.key.starts_with("zant.") {
                continue;
            }
            let located = |err: message::MessageError| {
                error(
                    &catalog.path,
                    json::source_offset(src, entry.value_loc, err.offset),
                    err.msg,
                )
            };
            let message = message::parse(&entry.value).map_err(located)?;
            let contract = contract::contract(&message).map_err(located)?;
            messages.insert(entry.key.clone(), (entry, message, contract));
        }
        parsed.push(messages);
    }
    let keys: Vec<_> = parsed[default_index]
        .iter()
        .map(|(key, (_, _, contract))| (key.clone(), contract.clone()))
        .collect();
    let mut warnings = Vec::new();
    let mut out = String::from(
        "pub static DESCRIPTION: zant::i18n::catalog::Description = zant::i18n::catalog::Description { keys: &[",
    );
    for (key, _) in &keys {
        literal(&mut out, key);
        out.push(',');
    }
    out.push_str("], locales: &[");
    for (index, (catalog, src)) in catalogs.iter().enumerate() {
        for (key, (entry, _, translated)) in &parsed[index] {
            let Some((_, _, original)) = parsed[default_index].get(key) else {
                return Err(error(
                    &catalog.path,
                    entry.key_loc,
                    format!(
                        "`{key}` is not in the default language (`{default}`); add it there or remove it"
                    ),
                ));
            };
            contract::check_translation(original, translated).map_err(|err| {
                error(
                    &catalog.path,
                    json::source_offset(src, entry.value_loc, err.offset),
                    err.msg,
                )
            })?;
        }
        out.push_str("zant::i18n::catalog::LocaleDesc { tag: ");
        literal(&mut out, &catalog.locale);
        out.push_str(", name: ");
        literal(&mut out, names::name(&catalog.locale));
        out.push_str(", plural: &[");
        let rules = match zant_base::plural::cldr_rules(&catalog.locale) {
            Some(rules) => rules,
            None => {
                warnings.push(format!(
                    "{}: CLDR has no plural rules for `{}`; plural messages always use `other`",
                    catalog.path, catalog.locale
                ));
                Vec::new()
            }
        };
        // checked here so the runtime never meets a rule it cannot parse
        if let Err(why) = zant_base::plural::Rules::parse(&rules) {
            return Err(error(
                &catalog.path,
                0,
                format!("plural rules of `{}` do not parse: {why}", catalog.locale),
            ));
        }
        for (category, rule) in rules {
            write!(out, "(zant::i18n::plural::Category::{category:?},").unwrap();
            literal(&mut out, rule);
            out.push_str("),");
        }
        out.push_str("], messages: &[");
        for (key, contract) in &keys {
            if let Some((_, message, _)) = parsed[index].get(key) {
                out.push_str("Some(");
                ops(&mut out, &message.parts, contract);
                out.push_str("),");
            } else {
                out.push_str("None,");
                warnings.push(format!(
                    "{}: missing translation for `{key}` (falls back to {default})",
                    catalog.path
                ));
            }
        }
        out.push_str("] },");
    }
    write!(out, "], default: {default_index} }};").unwrap();
    // Formatting also verifies the generated item before handing it to callers.
    let syntax = syn::parse_file(&out).expect("emitted description is valid Rust");
    Ok(Compiled {
        rust: prettyplease::unparse(&syntax),
        keys,
        locales,
        default: default_index,
        warnings,
    })
}

fn literal(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.escape_debug().next() == Some('\\') => {
                write!(out, "\\u{{{:x}}}", c as u32).unwrap();
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn ops(out: &mut String, parts: &[message::Part], contract: &contract::Contract) {
    use message::Part;
    let arg = |name: &str| contract.params.iter().position(|p| p.name == name).unwrap();
    out.push_str("&[");
    for part in parts {
        out.push_str("zant::i18n::catalog::Op::");
        match part {
            Part::Text(text) => {
                out.push_str("Text(");
                literal(out, text);
                out.push(')');
            }
            Part::Arg { name, .. } => write!(out, "Arg({})", arg(name)).unwrap(),
            Part::Number { name, .. } => write!(out, "Number({})", arg(name)).unwrap(),
            Part::Pound { .. } => out.push_str("Pound"),
            Part::Plural {
                name,
                exact,
                categories,
                ..
            } => {
                write!(out, "Plural {{ arg: {}, exact: &[", arg(name)).unwrap();
                for (n, parts) in exact {
                    write!(out, "({n},").unwrap();
                    ops(out, parts, contract);
                    out.push_str("),");
                }
                out.push_str("], categories: &[");
                for (category, parts) in categories {
                    write!(out, "(zant::i18n::plural::Category::{category:?},").unwrap();
                    ops(out, parts, contract);
                    out.push_str("),");
                }
                out.push_str("] }");
            }
            Part::Select { name, cases, .. } => {
                write!(out, "Select {{ arg: {}, cases: &[", arg(name)).unwrap();
                for (key, parts) in cases {
                    out.push('(');
                    literal(out, key);
                    out.push(',');
                    ops(out, parts, contract);
                    out.push_str("),");
                }
                out.push_str("] }");
            }
        }
        out.push(',');
    }
    out.push(']');
}

#[cfg(test)]
mod tests {
    use super::literal;

    #[test]
    fn literals_round_trip_controls_and_non_printing_unicode() {
        let text = "\"\\\n\r\t\0\u{8}\u{7f}\u{85}\u{200b}\u{2028}\u{2029}中'";
        let mut rust = String::new();
        literal(&mut rust, text);
        assert!(rust.contains("\\u{200b}"));
        assert_eq!(syn::parse_str::<syn::LitStr>(&rust).unwrap().value(), text);
    }
}
