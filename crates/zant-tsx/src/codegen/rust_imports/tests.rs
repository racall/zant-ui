//! Unit tests.

use super::*;
use crate::error::Diag;

fn file(source: &str) -> syn::File {
    syn::parse_file(source).expect("test Rust source parses")
}

/// Runs `f` on a fresh generator (the diagnostic slot is leaked: the
/// generator borrows it for its lifetime, and these tests are tiny).
fn with_generator<R>(f: impl FnOnce(&mut Generator<'static>) -> R) -> (Option<Diag>, R) {
    let diag: &'static mut Option<Diag> = Box::leak(Box::new(None));
    let mut g = Generator::new(diag);
    let result = f(&mut g);
    let diag = g.diag.clone();
    (diag, result)
}

fn signature_of(source: &str, name: &str) -> (Option<Diag>, Result<Function>) {
    let file = file(source);
    let function = find(&file, name).expect("fn");
    with_generator(|g| signature(g, "crate::format", &file, &function, 0))
}

#[test]
fn signatures_map_primitive_optional_and_slice_types() {
    let source = "\
fn hidden() {}
pub const value: u8 = 1;
pub fn plural(n: usize, word: &str) -> zant::Str { word.into() }
pub fn all(a: bool, b: Option<i32>, c: Vec<f64>, d: Vec<zant::Str>) -> Option<Vec<bool>> { None }
pub fn nothing() {}
";
    let module_file = file(source);
    assert!(find(&module_file, "hidden").is_none());
    assert!(find(&module_file, "value").is_none());
    with_generator(|g| {
        let plural = find(&module_file, "plural").expect("plural");
        let plural = signature(g, "crate::format", &module_file, &plural, 0).expect("plural");
        assert_eq!(vec![Type::Usize, Type::String], plural.params);
        assert_eq!(Type::String, plural.ret);
        let all = find(&module_file, "all").expect("all");
        let all = signature(g, "crate::format", &module_file, &all, 0).expect("all");
        assert_eq!(Type::Boolean, all.params[0]);
        assert_eq!(
            Type::I32,
            model::optional_child(g, all.params[1]).expect("optional")
        );
        assert_eq!(
            Type::F64,
            model::element(g, all.params[2]).expect("element")
        );
        assert_eq!(
            Type::String,
            model::element(g, all.params[3]).expect("element")
        );
        let inner = model::optional_child(g, all.ret).expect("optional");
        assert_eq!(Type::Boolean, model::element(g, inner).expect("element"));
        let nothing = find(&module_file, "nothing").expect("nothing");
        let nothing = signature(g, "crate::format", &module_file, &nothing, 0).expect("nothing");
        assert_eq!(Type::Void, nothing.ret);
        for (name, expected) in [
            ("u8", Type::U8),
            ("u16", Type::U16),
            ("u32", Type::U32),
            ("u64", Type::U64),
            ("i8", Type::I8),
            ("i16", Type::I16),
            ("i32", Type::I32),
            ("i64", Type::I64),
            ("usize", Type::Usize),
            ("isize", Type::Isize),
            ("f32", Type::F32),
            ("f64", Type::F64),
        ] {
            let source = format!("pub fn f(x: {name}) -> {name} {{ x }}");
            let numeric = file(&source);
            let function = find(&numeric, "f").expect("f");
            let function = signature(g, "crate::format", &numeric, &function, 0).expect("numeric");
            assert_eq!(expected, function.params[0], "{name}");
            assert_eq!(expected, function.ret, "{name}");
        }
        // plain (non-generic) references are followed to the file's types
        let references = file("pub fn name(p: Peer) -> Peer { p }");
        let function = find(&references, "name").expect("name");
        assert!(
            signature(g, "crate::devices", &references, &function, 0).is_err(),
            "unknown types are diagnostics"
        );
    });
}

#[test]
fn unsupported_signatures_report_the_import_name_and_offending_type() {
    for parameter in [
        "x: Foo",
        "x: *const u8",
        "x: impl Iterator<Item = u8>",
        "x: [u8; 4]",
        "x: (u8, u8)",
    ] {
        let source = format!("pub fn f({parameter}) {{}}");
        let (diag, result) = signature_of(&source, "f");
        assert!(result.is_err(), "{parameter} should be unsupported");
        let diag = diag.expect("diagnostic");
        assert_eq!(0, diag.loc);
        assert!(diag.msg.starts_with("`f` takes `"), "{}", diag.msg);
    }
    for ret in ["!", "Result<u8, ()>", "Foo", "Box<u8>"] {
        let source = format!("pub fn f() -> {ret} {{ todo!() }}");
        let (diag, result) = signature_of(&source, "f");
        assert!(result.is_err(), "{ret} should be unsupported");
        assert!(
            diag.expect("diagnostic").msg.starts_with("`f` returns `"),
            "ret {ret}"
        );
    }
}

#[test]
fn resolve_type_maps_pub_structs_and_enums() {
    let file = file(
        "pub struct Peer { pub name: String, pub online: bool, not_tsx: Box<u8> }\n\
             pub enum Kind { Laptop, Desktop }\n\
             struct Private { id: u8 }\n",
    );
    let (_, ()) = with_generator(|g| {
        let peer = resolve_type(g, "crate::devices", &file, "Peer")
            .expect("resolves")
            .expect("pub struct");
        let info = model::info(g, peer).expect("object");
        assert_eq!("crate::devices::Peer", info.rust);
        assert_eq!(2, info.fields.len());
        assert_eq!(Type::String, info.fields[0].ty);
        assert_eq!(Type::Boolean, info.fields[1].ty);
        let kind = resolve_type(g, "crate::devices", &file, "Kind")
            .expect("resolves")
            .expect("pub enum");
        assert!(model::enum_type(g, kind));
        assert!(
            resolve_type(g, "crate::devices", &file, "Private")
                .expect("resolves")
                .is_none()
        );
    });
}
