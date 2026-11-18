//! Unit tests.

use super::*;
use crate::codegen::{Generator, types};

fn parse(src: &str) -> ast::Module {
    let mut parser = crate::parser::Parser::init(src);
    parser.parse_module().expect("test source parses")
}

fn with_module(src: &str, f: impl FnOnce(&mut Generator<'_>, &ast::Module)) {
    let module = parse(src);
    let mut diag = None;
    let mut g = Generator::new(&mut diag);
    g.type_decls = module.types.clone();
    g.resolving = vec![false; module.types.len()];
    f(&mut g, &module);
}

#[test]
fn edit_distance_is_levenshtein() {
    assert_eq!(3, edit_distance(b"kitten", b"sitting"));
    assert_eq!(0, edit_distance(b"same", b"same"));
    assert_eq!(3, edit_distance(b"abc", b""));
    assert_eq!(usize::MAX, edit_distance(&[b'a'; 65], b"a"));
}

#[test]
fn arrays_and_optionals_are_interned() {
    let mut diag = None;
    let mut g = Generator::new(&mut diag);
    let list = array(&mut g, Type::F64).unwrap();
    assert_eq!(list, array(&mut g, Type::F64).unwrap());
    assert_eq!(Some(Type::F64), element(&g, list));
    assert_eq!("tsx::List<f64>", list.rust(&g));
    let maybe = optional(&mut g, list).unwrap();
    assert_eq!(maybe, optional(&mut g, list).unwrap());
    assert_eq!(Some(list), optional_child(&g, maybe));
    assert!(maybe.rust(&g).starts_with("Option<"));
    // `T | null` around a node uses the dedicated primitive
    assert_eq!(Type::OptionalNode, optional(&mut g, Type::Node).unwrap());
    assert_eq!(Type::Null, optional(&mut g, Type::Null).unwrap());
}

#[test]
fn string_unions_declare_copy_enums_with_as_str() {
    with_module(r#"type Mode = "a" | "in-progress";"#, |g, _| {
        let ty = named(g, b"Mode").unwrap().expect("resolved");
        assert!(enum_type(g, ty));
        assert!(ty.copy(g));
        let declaration = declarations(g);
        assert!(
            declaration.contains("#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]"),
            "{declaration}"
        );
        assert!(declaration.contains("InProgress,"), "{declaration}");
        assert!(
            declaration.contains("Mode::InProgress => \"in-progress\","),
            "{declaration}"
        );
    });
}

#[test]
fn interfaces_declare_structs_in_field_order() {
    with_module(
        r#"interface Point { x: number; y: number; label?: string }"#,
        |g, _| {
            let ty = named(g, b"Point").unwrap().expect("resolved");
            let declaration = declarations(g);
            assert!(
                declaration.contains("#[derive(Clone, PartialEq, Default)]"),
                "{declaration}"
            );
            let x = declaration.find("pub x: f64").unwrap();
            let y = declaration.find("pub y: f64").unwrap();
            assert!(x < y);
            assert!(declaration.contains("pub label: Option<zant::Str>,"));
            assert_eq!(Some(Type::F64), field_type(g, ty, b"x", 0).ok());
            let _ = types::Type::F64;
        },
    );
}

#[test]
fn unknown_fields_suggest_the_closest_name() {
    with_module(
        r#"interface Row { title: string; done: boolean }"#,
        |g, _| {
            let ty = named(g, b"Row").unwrap().expect("resolved");
            assert!(field_type(g, ty, b"titel", 0).is_err());
            let diag = g.diag.as_ref().expect("a diagnostic");
            assert!(diag.msg.contains("did you mean `title`"), "{}", diag.msg);
        },
    );
}

#[test]
fn recursive_types_are_a_diagnostic() {
    with_module(r#"type A = B; type B = A;"#, |g, _| {
        assert!(named(g, b"A").is_err());
        let diag = g.diag.as_ref().expect("a diagnostic");
        assert!(
            diag.msg.contains("recursive types are not supported"),
            "{}",
            diag.msg
        );
    });
}
