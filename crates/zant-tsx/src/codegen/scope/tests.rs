//! Unit tests.

use super::*;
use crate::codegen::{Generator, State, StateKind};

fn with_generator(f: impl FnOnce(&mut Generator<'_>)) {
    let mut diag = None;
    let mut g = Generator::new(&mut diag);
    f(&mut g);
}

#[test]
fn bind_avoids_shadowing_and_temporary_names() {
    with_generator(|g| {
        let mut c = Context::default();
        let first = bind(g, &mut c, b"x", Type::F64, false).unwrap();
        let second = bind(g, &mut c, b"x", Type::F64, false).unwrap();
        assert_eq!("x", c.bindings[first].rust);
        assert_eq!("x_2", c.bindings[second].rust);
        // reserved names and import module names are never reused
        let third = bind(g, &mut c, b"self", Type::F64, false).unwrap();
        assert_eq!("self_2", c.bindings[third].rust);
    });
}

#[test]
fn finish_expands_declarations_and_discards_unused_locals() {
    with_generator(|g| {
        let mut c = Context::default();
        let used = bind(g, &mut c, b"used", Type::F64, false).unwrap();
        c.bindings[used].init = Some("1.0f64".to_owned());
        let unused = bind(g, &mut c, b"unused", Type::F64, false).unwrap();
        c.bindings[unused].init = Some("2.0f64".to_owned());
        let body = format!(
            "{start}{used}{end}\n{start}{unused}{end}\nlet total: f64 = used;\n",
            start = PLACEHOLDER_START,
            end = PLACEHOLDER_END
        );
        let finished = finish(g, &c, body).unwrap();
        assert!(finished.contains("let used: f64 = 1.0f64;"), "{finished}");
        assert!(finished.contains("let unused: f64 = 2.0f64;"), "{finished}");
        assert!(finished.contains("let _ = unused;"), "{finished}");
        assert!(!finished.contains("let _ = used;"), "{finished}");
    });
}

#[test]
fn finish_discards_unused_parameters() {
    with_generator(|g| {
        let mut c = Context::default();
        let parameter = bind(g, &mut c, b"arg", Type::F64, false).unwrap();
        c.params.push(parameter);
        let finished = finish(g, &c, "return;\n".to_owned()).unwrap();
        assert_eq!("let _ = arg;\nreturn;\n", finished);
    });
}

#[test]
fn state_reads_snapshot_inside_handlers_and_borrow_in_render() {
    with_generator(|g| {
        g.states.push(State {
            name: "count".to_owned(),
            setter: "setCount".to_owned(),
            ty: Type::U32,
            initial: "0u32".to_owned(),
            kind: StateKind::State,
        });
        let mut render = Context {
            render: true,
            ..Context::default()
        };
        assert_eq!(
            "self.count",
            read(g, &mut render, b"count", 0).unwrap().code
        );
        let mut handler = Context::default();
        let value = read(g, &mut handler, b"count", 0).unwrap();
        assert_eq!("count", value.code);
        let finished = finish(g, &handler, "count = count + 1u32;\n".to_owned()).unwrap();
        // the value of the last render (React's per-render state)
        assert!(
            finished.starts_with("let count: u32 = self._render_count;\n"),
            "{finished}"
        );
    });
}

#[test]
fn non_copy_reads_clone() {
    with_generator(|g| {
        let mut c = Context::default();
        bind(g, &mut c, b"name", Type::String, false).unwrap();
        assert_eq!("name.clone()", read(g, &mut c, b"name", 0).unwrap().code);
    });
}

#[test]
fn type_index_lookups_match_names() {
    with_generator(|g| {
        g.states.push(State {
            name: "count".to_owned(),
            setter: "setCount".to_owned(),
            ty: Type::U32,
            initial: "0u32".to_owned(),
            kind: StateKind::State,
        });
        assert_eq!(Some(0), state_index(g, b"count"));
        assert_eq!(Some(0), setter_index(g, b"setCount"));
        assert_eq!(None, setter_index(g, b"count"));
    });
}

#[test]
fn rust_module_paths_need_no_renaming() {
    with_generator(|g| {
        let raw = "ui::button(ui::ButtonOptions { ..Default::default() })\n".to_owned();
        assert_eq!(raw, rename_modules(g, raw.clone()).unwrap());
    });
}
