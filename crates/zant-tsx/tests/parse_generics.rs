//! Generic syntax is preserved independently of monomorphization/codegen.

use zant_tsx::ast::*;
use zant_tsx::dump::dump_program;
use zant_tsx::hooks::rewrite::{Free, Kind, Rewriter};
use zant_tsx::parser::Parser;

fn parse(src: &str) -> Module {
    let mut p = Parser::init(src);
    p.parse_module()
        .unwrap_or_else(|_| panic!("{src}\n{:?}", p.diag))
}

fn arrow(e: &Expr) -> &Function {
    let ExprData::Arrow(f) = &e.data else {
        panic!("expected arrow: {e:?}")
    };
    f
}

fn jsx(e: &Expr) -> &Jsx {
    let ExprData::Jsx(j) = &e.data else {
        panic!("expected JSX: {e:?}")
    };
    j
}

#[test]
fn function_declarations_preserve_type_parameters_and_exports() {
    for prefix in ["", "export ", "export default "] {
        let m = parse(&format!(
            "{prefix}function id<T, U extends Box<T> = Box<string>, const V>(x: T): U {{ return x; }}"
        ));
        let top = &m.functions[0];
        assert_eq!(top.exported, !prefix.is_empty());
        assert_eq!(top.default, prefix.contains("default"));
        let ps = &top.func.type_params;
        assert_eq!(ps.len(), 3);
        assert_eq!(ps[0].name.name, b"T");
        let constraint = ps[1].constraint.as_ref().unwrap();
        assert_eq!(constraint.name, b"Box");
        assert_eq!(constraint.args[0].name, b"T");
        assert_eq!(ps[1].default.as_ref().unwrap().args[0].name, b"string");
        assert!(ps[2].is_const);
        assert!(!ps[0].is_const);
        assert!(ps[0].constraint.is_none() && ps[0].default.is_none());
    }
    let m = parse("export default function<T>(x: T): T { return x; }");
    assert_eq!(m.functions[0].func.type_params[0].name.name, b"T");
}

#[test]
fn declared_native_functions_keep_generics_in_the_ast() {
    let m = parse("export declare function read<const T extends Foo = Bar>(x: T): T;");
    assert!(m.declared[0].exported);
    let p = &m.declared[0].func.type_params[0];
    assert!(p.is_const);
    assert_eq!(p.constraint.as_ref().unwrap().name, b"Foo");
    assert_eq!(p.default.as_ref().unwrap().name, b"Bar");
}

#[test]
fn generic_interfaces_aliases_and_variance_parse() {
    let m = parse(
        "export interface Box<out T> { value: T } interface B<in out T> extends A<T>, C<Box<T>> { value: T } type Pair<A, B = string> = { first: A; second: B }; type Contextual<in, out> = { input: in; output: out };",
    );
    assert!(m.types[0].exported);
    assert_eq!(m.types[0].type_params[0].name.name, b"T");
    let TypeDetail::Intersection(bases) = m.types[1].ty.detail.as_deref().unwrap() else {
        panic!("expected bases")
    };
    assert_eq!(bases[0].name, b"A");
    assert_eq!(bases[0].args[0].name, b"T");
    assert_eq!(bases[1].args[0].args[0].name, b"T");
    assert_eq!(m.types[2].type_params.len(), 2);
    assert_eq!(m.types[3].type_params[0].name.name, b"in");
    assert_eq!(m.types[3].type_params[1].name.name, b"out");
}

#[test]
fn named_type_arguments_include_previously_rejected_builtins() {
    for (source, name, count) in [
        ("Box<number>", "Box", 1),
        ("Map<string, number[]>", "Map", 2),
        ("Record<K, V>", "Record", 2),
        ("Partial<Todo>", "Partial", 1),
        ("Promise<void>", "Promise", 1),
        ("ns.Box<T,>", "ns.Box", 1),
    ] {
        let m = parse(&format!("type Value = {source};"));
        let t = &m.types[0].ty;
        assert_eq!(t.name, name.as_bytes());
        assert_eq!(t.args.len(), count);
        assert!(t.detail.is_none());
    }
}

#[test]
fn nested_closers_and_array_special_forms_preserve_structure() {
    let m = parse(
        "type A = Box<Box<T>>; type B = Box<Box<Box<T>>>; type C = Array<Box<T>>; type D = ReadonlyArray<Array<Box<T>>>;",
    );
    assert_eq!(m.types[0].ty.args[0].args[0].name, b"T");
    assert_eq!(m.types[1].ty.args[0].args[0].args[0].name, b"T");
    let TypeDetail::Array(t) = m.types[2].ty.detail.as_deref().unwrap() else {
        panic!("array")
    };
    assert_eq!(t.name, b"Box");
    assert_eq!(t.args[0].name, b"T");
    let TypeDetail::Array(t) = m.types[3].ty.detail.as_deref().unwrap() else {
        panic!("array")
    };
    let TypeDetail::Array(t) = t.detail.as_deref().unwrap() else {
        panic!("array")
    };
    assert_eq!(t.args[0].name, b"T");
    assert!(m.types[2].ty.args.is_empty());
    let m = parse(
        "const value: Box<Box<T>>=f<Box<Box<T>>>(1); const comparison = a < b; const greater = a >= b;",
    );
    let ExprData::Call(c) = &m.consts[0].value.data else {
        panic!("call")
    };
    assert_eq!(c.type_args[0].args[0].args[0].name, b"T");
    assert!(matches!(m.consts[1].value.data, ExprData::Binary(_)));
    assert!(matches!(&m.consts[2].value.data, ExprData::Binary(b) if b.op == b">="));
}

#[test]
fn tsx_generic_arrows_require_a_disambiguator() {
    for head in [
        "<T,>",
        "<T extends unknown>",
        "<T = string>",
        "<const T,>",
        "<T, U>",
    ] {
        let m = parse(&format!(
            "const f = {head}(x: T): T => x; function outer() {{ return {head}(x: T) => x; }}"
        ));
        assert!(!m.functions[0].func.type_params.is_empty());
        let Body::Block(ss) = &m.functions[1].func.body else {
            panic!("block")
        };
        let StmtData::Ret(Some(e)) = &ss[0].data else {
            panic!("return")
        };
        assert_eq!(arrow(e).type_params[0].name.name, b"T");
    }
    let m = parse("const view = <T>(x: T) =&gt; x</T>;");
    assert_eq!(jsx(&m.consts[0].value).name, b"T");
    let mut p = Parser::init("const f = <T>(x: T) => x;");
    assert!(p.parse_module().is_err());
    assert!(p.diag.unwrap().msg.contains("JSX text"));
    let m = parse("const view = <T extends={value} />;");
    assert!(jsx(&m.consts[0].value).type_args.is_empty());
}

#[test]
fn generic_function_types_work_in_all_type_positions() {
    let m = parse(
        "type Identity = <T>(x: T) => T; interface Props { map: <const T extends Box<U> = Box<string>, U = string>(x: T) => Box<T> } function use(f: <T>(x:T) => T) { return f; }",
    );
    let TypeDetail::Func(f) = m.types[0].ty.detail.as_deref().unwrap() else {
        panic!("function type")
    };
    assert_eq!(f.type_params[0].name.name, b"T");
    assert_eq!(f.ret.name, b"T");
    let TypeDetail::Object(fields) = m.types[1].ty.detail.as_deref().unwrap() else {
        panic!("object")
    };
    let TypeDetail::Func(f) = fields[0].ty.detail.as_deref().unwrap() else {
        panic!("function type")
    };
    assert!(f.type_params[0].is_const);
    assert_eq!(f.type_params.len(), 2);
    assert_eq!(f.ret.args[0].name, b"T");
    assert!(matches!(
        m.functions[0].func.params[0]
            .ty
            .as_ref()
            .unwrap()
            .detail
            .as_deref(),
        Some(TypeDetail::Func(_))
    ));
}

#[test]
fn calls_keep_nested_and_multiple_type_arguments() {
    let m = parse(
        "const a = f<number>(1); const b = ns.f<Map<string, number[]>, Box<Box<T>>>(x); const c = f?.<number>(1); const d = items[0]<number>(1);",
    );
    let ExprData::Call(a) = &m.consts[0].value.data else {
        panic!("call")
    };
    assert_eq!(a.type_args[0].name, b"number");
    let ExprData::Call(b) = &m.consts[1].value.data else {
        panic!("call")
    };
    assert_eq!(b.type_args.len(), 2);
    assert_eq!(b.type_args[0].name, b"Map");
    assert_eq!(b.type_args[1].args[0].args[0].name, b"T");
    let ExprData::Call(c) = &m.consts[2].value.data else {
        panic!("call")
    };
    assert!(c.optional);
    assert_eq!(c.type_args[0].name, b"number");
    let ExprData::Call(d) = &m.consts[3].value.data else {
        panic!("call")
    };
    assert_eq!(d.type_args[0].name, b"number");
}

#[test]
fn jsx_type_arguments_work_with_attributes_children_and_fragments() {
    let m = parse(
        "const a = <List<Todo> items={items} />; const b = <><ns.List<Box<Box<T>>, string> items={items}><Text>child</Text></ns.List><List<Array<Todo>> /></>;",
    );
    let a = jsx(&m.consts[0].value);
    assert_eq!(a.type_args[0].name, b"Todo");
    assert_eq!(a.attrs[0].name.name, b"items");
    let b = jsx(&m.consts[1].value);
    assert!(b.type_args.is_empty());
    let Child::Element(list) = &b.children[0] else {
        panic!("element")
    };
    assert_eq!(list.name, b"ns.List");
    assert_eq!(list.type_args.len(), 2);
    assert_eq!(list.type_args[0].args[0].args[0].name, b"T");
    assert_eq!(list.children.len(), 1);
    let Child::Element(list) = &b.children[1] else {
        panic!("element")
    };
    assert!(matches!(
        list.type_args[0].detail.as_deref(),
        Some(TypeDetail::Array(_))
    ));
}

#[test]
fn generic_function_dump_includes_modifiers_constraints_and_defaults() {
    let source = "function id<const T extends Foo = Bar>(x: T): T { return x; }";
    let dump = dump_program(&parse(source));
    let expected = format!(
        "module\n  imports\n  functions\n    top-function exported=false default=false\n      function loc=0 name=\"id\"\n        type-params\n          type-param name=\"T\" loc={} const=true\n            constraint\n              type-ref loc={} name=\"Foo\"\n            default\n              type-ref loc={} name=\"Bar\"\n        params\n          param name=\"x\" loc={} optional=false\n            ty\n              type-ref loc={} name=\"T\"\n        ret\n          type-ref loc={} name=\"T\"\n        body block\n          stmt loc={} ret\n            expr loc={} ident \"x\"\n  types\n  consts\n  declared\n",
        source.find("T extends").unwrap(),
        source.find("Foo").unwrap(),
        source.find("Bar").unwrap(),
        source.find("x:").unwrap(),
        source.find("T)").unwrap(),
        source.find("T {").unwrap(),
        source.find("return").unwrap(),
        source.rfind('x').unwrap()
    );
    assert_eq!(dump, expected);
    let dump = dump_program(&parse(
        "type F = <T>(x:T) => Box<T>; const el = <List<T> />;",
    ));
    assert!(dump.contains("type-params"));
    assert!(dump.contains("args\n"));
    assert!(dump.contains("type-args\n"));
}

struct RenameTypes;
impl Free for RenameTypes {
    fn resolve(
        &mut self,
        name: &[u8],
        _: Loc,
        kind: Kind,
    ) -> Result<Vec<u8>, zant_tsx::hooks::rewrite::Failure> {
        if kind == Kind::TypeName {
            Ok([b"Imported_".as_slice(), name].concat())
        } else {
            Ok(name.to_vec())
        }
    }
}

#[test]
fn rewriter_preserves_type_bindings_and_rewrites_nested_arguments() {
    let m = parse(
        "function f<T extends Box<External> = Default>(T: number, x: T): Box<T> { return <List<T>>{g<External>(T)}</List>; }",
    );
    let mut free = RenameTypes;
    let mut rw = Rewriter {
        env: Vec::new(),
        type_names: Vec::new(),
        prefix: None,
        loc_tag: 0,
        free: Some(&mut free),
    };
    rw.prefix = Some(b"local_".to_vec());
    rw.loc_tag = 1 << 24;
    let f = rw.function(&m.functions[0].func).unwrap();
    let t = &f.type_params[0];
    assert_eq!(t.name.name, b"T");
    assert_ne!(t.name.loc & rw.loc_tag, 0);
    assert_eq!(t.constraint.as_ref().unwrap().name, b"Imported_Box");
    assert_eq!(
        t.constraint.as_ref().unwrap().args[0].name,
        b"Imported_External"
    );
    assert_eq!(t.default.as_ref().unwrap().name, b"Imported_Default");
    assert_eq!(f.params[1].ty.as_ref().unwrap().name, b"T");
    assert_eq!(f.ret.as_ref().unwrap().args[0].name, b"T");
    let Body::Block(ss) = &f.body else {
        panic!("block")
    };
    let StmtData::Ret(Some(e)) = &ss[0].data else {
        panic!("return")
    };
    let j = jsx(e);
    assert_eq!(j.type_args[0].name, b"T");
    let Child::Expr(Some(e)) = &j.children[0] else {
        panic!("expression")
    };
    let ExprData::Call(c) = &e.data else {
        panic!("call")
    };
    assert_eq!(c.type_args[0].name, b"Imported_External");
    assert!(matches!(&c.args[0].data, ExprData::Ident(n) if n == b"local_T"));
    assert!(rw.type_names.is_empty());
}

#[test]
fn rewriter_scopes_generic_function_type_names() {
    let m = parse("type F = <T extends Bound = Fallback>(x: T) => Box<T>; type Outside = T;");
    let mut free = RenameTypes;
    let mut rw = Rewriter {
        env: Vec::new(),
        type_names: Vec::new(),
        prefix: None,
        loc_tag: 0,
        free: Some(&mut free),
    };
    let ty = rw.type_ref(&m.types[0].ty).unwrap();
    let TypeDetail::Func(f) = ty.detail.as_deref().unwrap() else {
        panic!("function type")
    };
    assert_eq!(
        f.type_params[0].constraint.as_ref().unwrap().name,
        b"Imported_Bound"
    );
    assert_eq!(
        f.type_params[0].default.as_ref().unwrap().name,
        b"Imported_Fallback"
    );
    assert_eq!(f.params[0].ty.as_ref().unwrap().name, b"T");
    assert_eq!(f.ret.args[0].name, b"T");
    assert_eq!(rw.type_ref(&m.types[1].ty).unwrap().name, b"Imported_T");
}
