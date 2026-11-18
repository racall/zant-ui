//! The `tsx::hot::Hot` impls that carry component state across a hot reload.

use super::*;

/// Emit field dispatch with concrete types so autoref specialization can skip
/// bookkeeping and unsupported application values.
pub(super) fn hot_fields(fields: &str) -> (String, String) {
    let mut save = String::new();
    let mut load = String::new();
    for line in fields.lines() {
        let Some((field, _)) = line.trim().split_once(':') else {
            continue;
        };
        save.push_str(&format!(
            "zant::hot_put!(snapshot, {field:?}, self.{field});\n"
        ));
        load.push_str(&format!(
            "zant::hot_get!(restore, {field:?}, self.{field});\n"
        ));
    }
    (save, load)
}

pub(in crate::codegen) fn hot_impl(name: &str, fields: &str, strict: bool) -> String {
    if fields.trim().is_empty() {
        return parse_and_format(&format!("impl tsx::hot::Hot for {name} {{
            fn save(&self) -> tsx::hot::Value {{ tsx::hot::Snapshot::new().value() }}
            fn load(value: &tsx::hot::Value) -> Option<Self> {{ value.as_object()?; Some(Self::default()) }}
        }}\n"));
    }
    let (save, load) = hot_fields(fields);
    let load = load.replace("self.", "instance.");
    let load = if strict {
        load.lines()
            .map(|line| format!("if !{} {{ return None; }}\n", line.trim_end_matches(';')))
            .collect::<String>()
    } else {
        load
    };
    parse_and_format(&format!("impl tsx::hot::Hot for {name} {{
        fn save(&self) -> tsx::hot::Value {{ let mut snapshot = tsx::hot::Snapshot::new(); {save} snapshot.value() }}
        fn load(value: &tsx::hot::Value) -> Option<Self> {{
            value.as_object()?;
            let restore = tsx::hot::Restore::parse(&value.to_string());
            let mut instance = Self::default(); {load} Some(instance)
        }}
    }}\n"))
}
