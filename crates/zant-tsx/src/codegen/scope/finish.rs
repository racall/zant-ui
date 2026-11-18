//! Finishing a scope's body: placeholders for captured clones and discarded bindings.

use super::*;

/// Resolve declaration placeholders and insert only the required discards.
pub fn finish(g: &mut Generator<'_>, c: &Context, body: String) -> Result<String> {
    let mut result = body;
    for binding in c.bindings.iter().rev() {
        let Some(init) = binding.init.as_ref() else {
            continue;
        };
        let marker = format!("{PLACEHOLDER_START}{}{PLACEHOLDER_END}", binding.serial);
        let kind = if binding.assigned { "let mut" } else { "let" };
        let ty = binding.ty.rust(g);
        let declaration = format!(
            "{kind} {rust}: {ty} = {init};\n{DISCARD_START}{}{DISCARD_END}",
            binding.serial,
            rust = binding.rust
        );
        result = result.replace(&marker, &declaration);
    }
    // Count identifiers in the surviving output, rather than in visited
    // expressions: a later replacement may have consumed an earlier value.
    let mut usage_body = result.clone();
    for binding in &c.bindings {
        if binding.init.is_some() {
            usage_body = usage_body.replace(
                &format!("{DISCARD_START}{}{DISCARD_END}", binding.serial),
                "",
            );
        }
    }
    let counts = rust_text::count_identifiers(&usage_body);
    for binding in &c.bindings {
        if binding.init.is_none() {
            continue;
        }
        let count = counts.get(&binding.rust).copied().unwrap_or(0);
        let discard = if count == 1 {
            format!("let _ = {};\n", binding.rust)
        } else {
            String::new()
        };
        result = result.replace(
            &format!("{DISCARD_START}{}{DISCARD_END}", binding.serial),
            &discard,
        );
    }
    let mut prefix = String::new();
    for index in &c.params {
        let binding = &c.bindings[*index];
        if counts.get(&binding.rust).copied().unwrap_or(0) == 0 {
            prefix.push_str(&format!("let _ = {};\n", binding.rust));
        }
    }
    for snapshot in &c.snapshots {
        let binding = &c.bindings[snapshot.binding];
        if counts.get(&binding.rust).copied().unwrap_or(0) == 0 {
            continue;
        }
        let state = g.states[snapshot.state].clone();
        // the value of the last render (React's per-render state), not one
        // a handler earlier in this event has just set
        let field = if state.kind == crate::codegen::StateKind::State {
            crate::codegen::generate::render_state_field(&state.name)
        } else {
            state_field(g, state.name.as_bytes())?
        };
        let init = if state.ty.copy(g) {
            format!("self.{field}")
        } else {
            format!("self.{field}.clone()")
        };
        prefix.push_str(&format!(
            "let {}: {} = {init};\n",
            binding.rust,
            state.ty.rust(g)
        ));
    }
    Ok(format!("{prefix}{result}"))
}
