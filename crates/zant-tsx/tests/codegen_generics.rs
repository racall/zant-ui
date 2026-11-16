//! Located codegen diagnostics for syntax reserved for monomorphization.

use zant_tsx::ast;
use zant_tsx::codegen::{Generator, functions, generate, model, types};
use zant_tsx::compile::{self, SourceFile};
use zant_tsx::error::Diag;
use zant_tsx::parser::Parser;

fn parse(source: &str) -> ast::Module {
    let mut p = Parser::init(source);
    p.parse_module()
        .unwrap_or_else(|_| panic!("parse failed: {:?}", p.diag))
}

fn diagnostic(source: &str, native: bool) -> Diag {
    let module = parse(source);
    let options = generate::Options {
        require_root: false,
        native,
        ..Default::default()
    };
    assert!(generate::generate("test.tsx", &module, &options).is_err());
    generate::last_diag().expect("located codegen diagnostic")
}

#[test]
fn declarations_and_arrows_parse_but_codegen_rejects_type_parameters() {
    for (source, at) in [
        ("function f<T>(x: T): T { return x; }", "function"),
        ("export function f<T>(x: T): T { return x; }", "function"),
        (
            "export default function App<T>() { return <Text />; }",
            "function",
        ),
        (
            "export default function<T>() { return <Text />; }",
            "function",
        ),
        ("const f = <T,>(x: T) => x;", "<T"),
        (
            "function f() { return <T extends unknown>(x: T) => x; }",
            "<T",
        ),
        (
            "function f() { const g = <T,>(x: T) => x; return 1; }",
            "<T",
        ),
        (
            "function f() { function g<T>(x: T) { return x; } return 1; }",
            "function g",
        ),
    ] {
        let diag = diagnostic(source, false);
        assert_eq!(
            diag.msg, "generic functions are not supported yet",
            "{source}"
        );
        assert_eq!(diag.loc as usize, source.find(at).unwrap(), "{source}");
    }
}

#[test]
fn unused_generic_type_declarations_generate_nothing() {
    // a generic declaration is emitted with its first use (`Box<number>`)
    for source in [
        "interface Box<T> { value: T }",
        "type Pair<A, B> = { first: A; second: B };",
    ] {
        let module = parse(source);
        let options = generate::Options {
            require_root: false,
            ..Default::default()
        };
        let generated = generate::generate("test.tsx", &module, &options).expect("generates");
        assert!(!generated.source.contains("pub struct"), "{source}");
    }
}

#[test]
fn generic_function_types_are_rejected_in_aliases_props_and_parameters() {
    for source in [
        "type F = <T>(x: T) => T;",
        "interface P { f: <T>(x: T) => T }",
        "function f(cb: <T>(x: T) => T) { return 1; }",
    ] {
        let diag = diagnostic(source, false);
        assert_eq!(diag.msg, "generic functions are not supported yet");
        assert_eq!(diag.loc as usize, source.find("<T>").unwrap());
    }
}

#[test]
fn generic_references_are_rejected_after_parsing_including_known_type_names() {
    for ty in [
        "Box<number>",
        "Record<string, number>",
        "Partial<Todo>",
        "Promise<void>",
        "Map<string, number[]>",
        "number<string>",
    ] {
        let source = format!("interface Box {{ value: number }} type Value = {ty};");
        let name = ty.split('<').next().unwrap();
        let diag = diagnostic(&source, false);
        assert_eq!(
            diag.msg,
            format!(
                "generic type `{name}<…>` is not supported yet; declare it in this file (`interface X<T>`) or use a concrete type"
            )
        );
        assert_eq!(diag.loc as usize, source.rfind(ty).unwrap());
    }
    let source = "type Value = Array<Box<number>>;";
    let diag = diagnostic(source, false);
    assert!(diag.msg.contains("generic type `Box<…>`"));
    assert_eq!(diag.loc as usize, source.find("Box").unwrap());
}

#[test]
fn calls_and_jsx_keep_the_takes_no_type_arguments_message() {
    for (source, name, at) in [
        (
            "function f(x: number): number { return x; } function g(): number { return f<number>(1); }",
            "f",
            "f<number>",
        ),
        (
            "export default function App() { return <Text>{useTheme<number>()}</Text>; }",
            "useTheme",
            "useTheme",
        ),
        (
            "function f() { return <List<Todo> items={items} />; }",
            "List",
            "<List",
        ),
        (
            "function f() { return <><List<Box<T>> /></>; }",
            "List",
            "<List",
        ),
        (
            "function f() { return <Text<number>>hello</Text>; }",
            "Text",
            "<Text",
        ),
    ] {
        let diag = diagnostic(source, false);
        assert_eq!(
            diag.msg,
            format!("`{name}` takes no type arguments"),
            "{source}"
        );
        assert_eq!(diag.loc as usize, source.find(at).unwrap());
    }
}

#[test]
fn native_generic_signatures_parse_and_then_report_native_restriction() {
    let source = "export declare function f<T>(x: T): T;";
    let diag = diagnostic(source, true);
    assert_eq!(diag.msg, "native functions are not generic");
    assert_eq!(diag.loc as usize, source.find("function").unwrap());
    let files = [SourceFile::native_spec("native/api.ts", source)];
    let error = compile::compile_project(&files).unwrap_err();
    assert_eq!(error.file, 0);
    assert_eq!(error.diag.msg, "native functions are not generic");
}

#[test]
fn generic_custom_hooks_fail_before_expansion_can_erase_them() {
    let source = "function useIdentity<T>(x: T) { return x; } export default function App() { const value = useIdentity(1); return <Text>{value}</Text>; }";
    let diag = compile::compile(source, "App.tsx").unwrap_err();
    assert_eq!(diag.msg, "generic functions are not supported yet");
    assert_eq!(diag.loc, 0);
    let files = [SourceFile::tsx("src/App.tsx", source)];
    let error = compile::compile_project(&files).unwrap_err();
    assert_eq!(error.diag, diag);
}

#[test]
fn core_type_resolution_and_function_collection_also_reject_generics() {
    for (source, message) in [
        (
            "interface Box<T> { value: T }",
            "`Box` needs a type argument for `T`",
        ),
        ("type F = <T>(x:T) => T;", "not supported yet"),
    ] {
        let m = parse(source);
        let mut diag = None;
        let mut g = Generator::new(&mut diag);
        g.type_decls = m.types.clone();
        g.resolving = vec![false; m.types.len()];
        assert!(model::named(&mut g, &m.types[0].name.name).is_err());
        assert!(diag.unwrap().msg.contains(message), "{source}");
    }
    let m = parse("type X = Box<number>;");
    let mut diag = None;
    let mut g = Generator::new(&mut diag);
    assert!(types::type_ref(&mut g, &m.types[0].ty, false).is_err());
    assert_eq!(diag.unwrap().loc, m.types[0].ty.loc);
    let m = parse("function f<T>() { return 1; }");
    let mut diag = None;
    let mut g = Generator::new(&mut diag);
    assert!(functions::collect(&mut g, &m).is_err());
    assert_eq!(diag.unwrap().msg, "generic functions are not supported yet");
}

#[test]
fn array_special_forms_still_generate_rust() {
    let m = parse("type Rows = ReadonlyArray<Array<number>>;");
    let mut diag = None;
    let mut g = Generator::new(&mut diag);
    g.type_decls = m.types.clone();
    g.resolving = vec![false];
    assert!(model::named(&mut g, b"Rows").unwrap().is_some());
    assert!(diag.is_none());
    // This is a codegen check, not merely a successful parser assertion.
    let generated = generate::generate(
        "types.tsx",
        &m,
        &generate::Options {
            require_root: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(syn::parse_file(&generated.source).is_ok());
}

#[test]
fn react_memo_normalization_preserves_generic_factory_arrows() {
    let source = "export default function App() { const f = useMemo(<T,>() => () => 1, []); return <Text />; }";
    let m = parse(source);
    let ast::Body::Block(ss) = &m.functions[0].func.body else {
        panic!("block")
    };
    let ast::StmtData::Decl(d) = &ss[0].data else {
        panic!("declaration")
    };
    assert!(matches!(
        d.init.as_ref().unwrap().data,
        ast::ExprData::Call(_)
    ));
    let diag = compile::compile(source, "App.tsx").unwrap_err();
    assert_eq!(diag.msg, "generic functions are not supported yet");
    assert_eq!(diag.loc as usize, source.find("<T,").unwrap());
}
