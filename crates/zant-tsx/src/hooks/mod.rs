//! Custom hooks (React's `function useToast() { … }`): every top-level
//! `useX(…)` call of a component is replaced by the hook's body before code
//! is generated.
//!
//! [`expand`] drives the inlining, [`rewrite`] copies and renames the hook's
//! body, [`resolve`] brings along the names the hook reads from its own file,
//! and [`leftover`] reports the calls that could not be inlined.

pub mod expand;
pub mod leftover;
pub mod resolve;
pub mod rewrite;

#[cfg(test)]
mod tests {
    use super::expand::{Failure, LOC_BITS, decode, expand, tag};
    use super::resolve::Unit;
    use crate::ast::{self, Body, ExprData, Pattern, Stmt, StmtData};
    use crate::dump::dump_program;
    use crate::parser::parse_ok;

    fn unit(name: &str, path: &str, src: &str) -> Unit {
        Unit {
            name: name.as_bytes().to_vec(),
            path: path.as_bytes().to_vec(),
            module: parse_ok(src),
            added: Vec::new(),
            exported: Vec::new(),
        }
    }

    /// The expansion of a single-file program (the compiler's first stage:
    /// parse and inline, without code generation).
    fn expand_source(src: &str) -> (Unit, Option<Failure>) {
        let mut units = vec![unit("App", "App.tsx", src)];
        let mut failure = None;
        let ok = expand(&mut units, &mut failure);
        let unit = units.pop().expect("one unit");
        (unit, if ok { None } else { failure })
    }

    fn expand_project(files: &[(&str, &str, &str)]) -> (Vec<Unit>, Option<Failure>) {
        let mut units: Vec<Unit> = files
            .iter()
            .map(|(name, path, src)| unit(name, path, src))
            .collect();
        let mut failure = None;
        let ok = expand(&mut units, &mut failure);
        (units, if ok { None } else { failure })
    }

    fn component_body<'a>(module: &'a ast::Module, name: &str) -> &'a [Box<Stmt>] {
        let top = module
            .functions
            .iter()
            .find(|top| top.func.name.as_slice() == name.as_bytes())
            .unwrap_or_else(|| panic!("no component `{name}`"));
        match &top.func.body {
            Body::Block(ss) => ss,
            Body::Expr(_) => panic!("a component's body is a block"),
        }
    }

    /// The `const [useX_name, set] = useState(…)` a hook's state became.
    fn state_decl<'a>(body: &'a [Box<Stmt>], name: &[u8]) -> Option<&'a ast::Call> {
        for s in body {
            let StmtData::Decl(d) = &s.data else {
                continue;
            };
            let Pattern::Array(names) = &d.pattern else {
                continue;
            };
            if !names.iter().flatten().any(|n| n.name == name) {
                continue;
            }
            let Some(init) = &d.init else {
                continue;
            };
            let ExprData::Call(call) = &init.data else {
                continue;
            };
            return Some(call);
        }
        None
    }

    fn assert_state_arg(body: &[Box<Stmt>], name: &str, expected: &ExprData) {
        let call = state_decl(body, name.as_bytes())
            .unwrap_or_else(|| panic!("no state declaration for `{name}`"));
        let arg = call.args.first().expect("the state has a value");
        assert_eq!(&arg.data, expected, "state `{name}`");
    }

    fn has_added(unit: &Unit, source: &str, imported: &str, local: &str) -> bool {
        unit.added.iter().any(|imp| {
            imp.source.as_slice() == source.as_bytes()
                && imp.names.iter().any(|n| {
                    n.imported.name.as_slice() == imported.as_bytes()
                        && n.local.name.as_slice() == local.as_bytes()
                })
        })
    }

    fn expect_error(src: &str, message: &str) {
        let mut units = vec![unit("App", "App.tsx", src)];
        let mut failure = None;
        let ok = expand(&mut units, &mut failure);
        if ok {
            panic!(
                "expected `{message}`, expanded:\n{}",
                dump_program(&units[0].module)
            );
        }
        let failure = failure.expect("a failure");
        assert!(
            failure.msg.contains(message),
            "missing `{message}` in `{}`",
            failure.msg
        );
    }

    // Code generation turns this into `useCounter_count: f64 = 5`; before
    // code generation that state is a `useState(5)` destructuring declaration.
    #[test]
    fn a_hook_in_the_same_file_becomes_the_components_state_and_effects() {
        let src = r#"import { useState, useEffect } from "zant";
function useCounter(start: number) {
  const [count, setCount] = useState(start);
  useEffect(() => { setCount(start); }, []);
  return { count, increment: () => setCount(count + 1) };
}
export default function App() {
  const { count, increment } = useCounter(5);
  const other = useCounter(1);
  return <Button onClick={increment}>{`${count} ${other.count}`}</Button>;
}
"#;
        let (unit, failure) = expand_source(src);
        assert!(failure.is_none(), "{failure:?}");
        let body = component_body(&unit.module, "App");
        assert_state_arg(body, "useCounter_count", &ExprData::Number(b"5".to_vec()));
        assert_state_arg(body, "useCounter2_count", &ExprData::Number(b"1".to_vec()));
        assert!(
            unit.module
                .functions
                .iter()
                .all(|top| top.func.name.as_slice() != b"useCounter"),
            "the hook is not code of its own"
        );
    }

    // The generated code reads `_Store.label(` and `_useClicks.step`; before
    // code generation those are the imports inlining added.
    #[test]
    fn a_hook_in_another_file_brings_the_imports_it_reads() {
        let store = r#"import { createStore } from "zant";
export interface Clicks { clicks: number }
export const store = createStore<Clicks>({ clicks: 0 });
export function label(n: number): string { return `${n} clicks`; }
"#;
        let hook = r#"import { useStore } from "zant";
import { store, label as describe } from "../Store";
import dot from "../../assets/dot.png";
const step = 1;
export function useClicks() {
  const s = useStore(store);
  const add = () => store.update({ clicks: s.clicks + step });
  return [describe(s.clicks), add, dot.length];
}
"#;
        let app = r#"import { useClicks } from "./hooks/useClicks";
export default function App() {
  const [text, add, size] = useClicks();
  return <Button onClick={add}>{`${text} ${size}`}</Button>;
}
"#;
        let (units, failure) = expand_project(&[
            ("Store", "src/Store.tsx", store),
            ("useClicks", "src/hooks/useClicks.tsx", hook),
            ("App", "src/App.tsx", app),
        ]);
        assert!(failure.is_none(), "{failure:?}");
        let app_unit = &units[2];
        assert!(
            has_added(app_unit, "./Store", "label", "describe"),
            "{:#?}",
            app_unit.added
        );
        assert!(
            has_added(app_unit, "./useClicks", "step", "step"),
            "{:#?}",
            app_unit.added
        );
        assert!(
            app_unit.added.iter().any(|imp| {
                imp.source.as_slice() == b"../assets/dot.png"
                    && imp
                        .default_name
                        .as_ref()
                        .is_some_and(|n| n.name.as_slice() == b"dot")
            }),
            "{:#?}",
            app_unit.added
        );
        // the hook file has no hook left, but keeps what the app imports from it
        assert!(
            !units[1]
                .module
                .functions
                .iter()
                .any(|top| top.func.name.as_slice() == b"useClicks")
        );
        assert!(
            units[1]
                .module
                .consts
                .iter()
                .any(|c| c.name.name.as_slice() == b"step" && c.exported)
        );
    }

    // Code generation turns the nested hook into `useFlag_on: bool = false`
    // and reports a broken hook in the hook's file; before code generation
    // those are the inlined state and the location tag `decode` reports
    // errors through.
    #[test]
    fn hooks_call_hooks_and_errors_point_into_the_hooks_file() {
        let hook = r#"import { useState } from "zant";
export function useFlag() { const [on, setOn] = useState(false); return [on, setOn]; }
export function useToggle() {
  const [on, setOn] = useFlag();
  return { on, toggle: () => setOn(!on) };
}
"#;
        let app = r#"import { useToggle } from "./useToggle";
export default function App() {
  const t = useToggle();
  return <Switch checked={t.on} onChange={t.toggle} />;
}
"#;
        let (units, failure) = expand_project(&[
            ("useToggle", "src/useToggle.tsx", hook),
            ("App", "src/App.tsx", app),
        ]);
        assert!(failure.is_none(), "{failure:?}");
        let body = component_body(&units[1].module, "App");
        assert_state_arg(body, "useFlag_on", &ExprData::Boolean(false));
        let dump = dump_program(&units[1].module);
        assert!(dump.contains("useFlag_on"), "{dump}");
        // `t.on` stands for the hook's value: the alias is final
        assert!(!dump.contains("useToggle_on"), "{dump}");
        // inlined code's locations carry the hook's file in their top bits
        let stmt = body
            .iter()
            .find(|s| {
                matches!(
                    &s.data,
                    StmtData::Decl(d)
                        if matches!(&d.pattern, Pattern::Array(names)
                            if names.iter().flatten().any(|n| n.name == b"useFlag_on"))
                )
            })
            .expect("the inlined state declaration");
        assert_ne!(0, stmt.loc & tag(0), "the inlined location is tagged");
        let (inlined_unit, inlined_loc) = decode(1, stmt.loc);
        assert_eq!(0, inlined_unit);
        assert_eq!(stmt.loc & ((1 << LOC_BITS) - 1), inlined_loc);
        // code of the component itself keeps the component's file
        let ret = body.last().expect("a return statement");
        assert_eq!(1, decode(1, ret.loc).0);
    }

    #[test]
    fn custom_hook_errors() {
        let hook = "import { useState } from \"zant\";\nfunction useN(n: number) { const [v, setV] = useState(n); return v; }\n";
        let app = "export default function App() {\n";
        expect_error(
            &format!("{hook}{app}if (true) {{ useN(1); }}\nreturn <View/>; }}"),
            "call it at the top level",
        );
        expect_error(
            &format!("{hook}{app}const f = () => useN(1);\nreturn <View/>; }}"),
            "call it at the top level",
        );
        expect_error(
            &format!("{hook}{app}const v = useN();\nreturn <Text>{{v}}</Text>; }}"),
            "`useN` takes 1 arguments",
        );
        expect_error(
            &format!("{hook}{app}const [a, b] = useN(1);\nreturn <Text>{{a}}</Text>; }}"),
            "an array literal",
        );
        expect_error(
            &format!("{hook}{app}const {{ a }} = useN(1);\nreturn <Text>{{a}}</Text>; }}"),
            "an object literal",
        );
        expect_error(
            &format!(
                "function useE(on: boolean) {{ if (on) {{ return 1; }} return 2; }}\n{app}const v = useE(true);\nreturn <Text>{{v}}</Text>; }}"
            ),
            "returns once, at its end",
        );
        expect_error(
            &format!(
                "const limit = 3;\nfunction useL() {{ return limit; }}\n{app}const limit = 4;\nconst v = useL();\nreturn <Text>{{v}}</Text>; }}"
            ),
            "`useL` reads `limit`, which App also declares",
        );
        expect_error(
            &format!(
                "function useO() {{ return {{ a: 1 }}; }}\n{app}const o = useO();\nreturn <Text>{{o}}</Text>; }}"
            ),
            "is the object a hook returned",
        );
    }

    // The repository's custom-hook project (`tests/fixtures/hooks/`) expands to
    // the local state the generated code declares for it:
    // `useCounter_count`, `useCounter2_count`, `useFlag_on`,
    // `useToggle_flips`, and `_config.step` imported from `./config`.
    #[test]
    fn repo_custom_hook_project_expands_to_prefixed_state() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = root.join("crates/zant-tsx/tests/fixtures/hooks");
        let read = |rel: &str| std::fs::read_to_string(dir.join(rel)).expect("read fixture");
        let mut units = vec![
            unit("App", "tests/fixtures/hooks/App.tsx", &read("App.tsx")),
            unit(
                "config",
                "tests/fixtures/hooks/config.tsx",
                &read("config.tsx"),
            ),
            unit(
                "useCounter",
                "tests/fixtures/hooks/hooks/useCounter.tsx",
                &read("hooks/useCounter.tsx"),
            ),
            unit(
                "useToggle",
                "tests/fixtures/hooks/hooks/useToggle.tsx",
                &read("hooks/useToggle.tsx"),
            ),
        ];
        let mut failure = None;
        assert!(expand(&mut units, &mut failure), "{failure:?}");
        let app = &units[0];
        let app_body = component_body(&app.module, "App");
        assert_state_arg(
            app_body,
            "useCounter_count",
            &ExprData::Number(b"2".to_vec()),
        );
        assert_state_arg(
            app_body,
            "useCounter2_count",
            &ExprData::Number(b"10".to_vec()),
        );
        assert_state_arg(app_body, "useFlag_on", &ExprData::Boolean(false));
        assert_state_arg(
            app_body,
            "useToggle_flips",
            &ExprData::Number(b"0".to_vec()),
        );
        assert!(
            has_added(app, "./config", "step", "step"),
            "{:#?}",
            app.added
        );
        let clicker = component_body(&app.module, "Clicker");
        assert_state_arg(
            clicker,
            "useCounter_count",
            &ExprData::Number(b"100".to_vec()),
        );
        // the two hook files hold no code of their own, but config exports
        // the value the app imports
        assert!(
            units[2]
                .module
                .functions
                .iter()
                .all(|top| top.func.name.as_slice() != b"useCounter")
        );
        assert!(
            units[3]
                .module
                .functions
                .iter()
                .all(|top| top.func.name.as_slice() != b"useToggle")
        );
        assert!(
            units[1]
                .module
                .consts
                .iter()
                .any(|c| c.name.name.as_slice() == b"step" && c.exported)
        );
    }

    // The AST dump of a component calling two custom hooks must be
    // deterministic (same names and order on every run).
    #[test]
    fn two_custom_hooks_expand_deterministically() {
        let src = r#"import { useState } from "zant";
function useA(n: number) {
  const [a, setA] = useState(n);
  return { a, set: setA };
}
function useB() {
  const [b, setB] = useState(0);
  return [b, () => setB(b + 1)];
}
export default function App() {
  const { a, set } = useA(1);
  const [b, bump] = useB();
  return <Button onClick={set} onMouseDown={bump}>{`${a} ${b}`}</Button>;
}
"#;
        let (first, failure) = expand_source(src);
        assert!(failure.is_none(), "{failure:?}");
        let (second, failure) = expand_source(src);
        assert!(failure.is_none(), "{failure:?}");
        let dump = dump_program(&first.module);
        assert_eq!(dump, dump_program(&second.module));
        assert!(dump.contains("useA_a"), "{dump}");
        assert!(dump.contains("useB_b"), "{dump}");
        assert!(!dump.contains("name=\"useA\""), "{dump}");
        assert!(!dump.contains("name=\"useB\""), "{dump}");
        assert!(dump.contains("name=\"App\""), "{dump}");
    }
}
