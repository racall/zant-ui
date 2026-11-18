//! Unit tests.

use super::*;

#[test]
fn extra_hooks_are_use_ref_use_memo_and_use_effect() {
    assert!(extra_hook("useRef"));
    assert!(extra_hook("useMemo"));
    assert!(extra_hook("useEffect"));
    assert!(!extra_hook("useState"));
    assert!(!extra_hook("useStore"));
}

#[test]
fn a_non_dependency_fails() {
    let mut parser = crate::parser::Parser::init(
        "function App() { const a = 1; useEffect(() => {}, [<Text>a</Text>]); return <Text>a</Text>; }\n",
    );
    let module = parser.parse_module().expect("test source parses");
    let mut diag = None;
    let mut g = Generator::new(&mut diag);
    let mut c = scope::Context {
        render: true,
        ..scope::Context::default()
    };
    let function = &module.functions[0].func;
    let statements = match &function.body {
        ast::Body::Block(statements) => statements,
        ast::Body::Expr(_) => panic!("block body"),
    };
    let decl = match &statements[1].data {
        ast::StmtData::Expr(call) => match &call.data {
            ast::ExprData::Call(call) => call.args[1].clone(),
            _ => panic!("effect call"),
        },
        _ => panic!("effect statement"),
    };
    assert!(dep_list(&mut g, &mut c, &decl).is_err());
    assert!(g.diag.is_some());
}
