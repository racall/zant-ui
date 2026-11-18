//! Unit tests.

use super::*;

const PINNED_SRC: &str = r#"import { View } from "zant";
function App({ title }: Props) {
  const [n, setN] = useState(0);
  if (n > 0) return <View>{n}</View>;
  for (let i = 0; i < 3; i++) n++;
  return null;
}"#;

const PINNED_DUMP: &str = "\
module
  imports
    import loc=0 source_loc=21 type_only=false source=\"zant\"
      import-name type_only=false imported=\"View\" local=\"View\"
  functions
    top-function exported=false default=false
      function loc=29 name=\"App\"
        params
          param name=\"props\" loc=42 optional=false
            ty
              type-ref loc=53 name=\"Props\"
            destructure \"title\"
        body block
          stmt loc=64 decl const=true
            pattern array
              binding \"n\"
              binding \"setN\"
            init
              expr loc=82 call
                callee
                  expr loc=82 ident \"useState\"
                arg
                  expr loc=91 number \"0\"
          stmt loc=97 if
            cond
              expr loc=101 binary op=\">\"
                lhs
                  expr loc=101 ident \"n\"
                rhs
                  expr loc=105 number \"0\"
            then
              stmt loc=108 ret
                expr loc=115 jsx
                  jsx loc=115 name=\"View\"
                    child expr
                      expr loc=122 ident \"n\"
          stmt loc=135 for
            init
              stmt loc=140 decl const=false
                pattern ident \"i\" loc=144
                init
                  expr loc=148 number \"0\"
            cond
              expr loc=151 binary op=\"<\"
                lhs
                  expr loc=151 ident \"i\"
                rhs
                  expr loc=155 number \"3\"
            update
              expr loc=158 update op=\"++\"
                target
                  expr loc=158 ident \"i\"
            body
              stmt loc=163 expr
                expr loc=163 update op=\"++\"
                  target
                    expr loc=163 ident \"n\"
          stmt loc=170 ret
            expr loc=177 null
  types
  consts
  declared
";

/// The pinned shape: byte-for-byte dump of a component (the golden form).
#[test]
fn dump_program_pins_a_component_shape() {
    let m = crate::parser::parse_ok(PINNED_SRC);
    assert_eq!(PINNED_DUMP, dump_program(&m));
}

// Total coverage: every node kind shows up in the dump.
#[test]
fn dump_program_covers_every_node_kind() {
    let m = crate::parser::parse_ok(
        r#"import Def, * as NS from "react";
import { type ReactNode, x as y } from "zant";
import icon from "./icon.png";
export declare function copy(row: i64): CopyResult;
export interface Todo { id: number; title?: string; tags: string[]; done: boolean | null; onToggle: (id: number) => void; }
export type Filter = "all" | "active";
export type Rows = Array<Todo>;
export const LIMIT: u32 = 3;
const later = (a: number = 1): string => `x${a}`;
export default function App({ size = 14 }: Props) {
  function helper() { return; }
  const { a, b } = obj;
  let [c, , d] = arr;
  const flag = true;
  const nothing = null;
  const t = !a;
  const u = -2;
  const v = a + b * 2;
  const w = a ?? b;
  const x1 = a === b ? 1 : 2;
  const x2 = f<Todo>(1, 2);
  const x3 = obj.prop?.inner;
  const x4 = arr[idx];
  const x5 = value!;
  const x6 = value as u32;
  const g = items.map((x: number) => x);
  a = 1;
  a += 2;
  ++a;
  if (a) { b(); } else c();
  for (let i = 0; i < 2; i++) continue;
  for (const item of items) break;
  while (a) a--;
  do a++; while (a);
  switch (a) { case 1: break; default: continue; }
  const obj2 = { key: 1, ...rest, short };
  const arr2 = [1, ...items];
  const esc = "\u00e9";
  const el = <View key={a} {...rest} disabled>text {a} <Text /></View>;
  const frag = <><Text /></>;
  return;
}"#,
    );
    let dump = dump_program(&m);

    // expression kinds
    for kind in [
        "number", "string", "boolean", "null", "ident", "template", "unary", "binary", "cond",
        "call", "member", "arrow", "jsx", "object", "array", "spread", "index", "non-null", "as",
        "assign", "update",
    ] {
        assert!(has_expr(&dump, kind), "missing expression kind {kind}");
    }
    // statement kinds
    for kind in [
        "decl", "func", "ret", "if", "for", "for-of", "while", "switch", "break", "continue",
        "expr", "block",
    ] {
        assert!(has_stmt(&dump, kind), "missing statement kind {kind}");
    }
    // type detail kinds
    for kind in ["array", "union", "null", "object", "func"] {
        assert!(
            dump.lines().any(|l| l.trim() == kind),
            "missing type detail {kind}"
        );
    }
    assert!(
        dump.lines()
            .any(|l| l.trim_start().starts_with("string-lit ")),
        "missing type detail string-lit"
    );
    // patterns, imports, JSX and other node kinds
    for line in [
        "pattern ident",
        "pattern array",
        "pattern object",
        "binding",
        "hole",
        "object-binding",
    ] {
        assert!(
            dump.lines().any(|l| l.trim_start().starts_with(line)),
            "missing pattern line {line}"
        );
    }
    for line in [
        "default ",
        "namespace ",
        "import-name",
        "type-decl",
        "top-const",
        "top-function",
        "type-field",
        "attr",
        "child text",
        "child expr",
        "child element",
        "quasi",
        "subexpr",
        "property",
        "case",
        "func",
        "string-lit",
    ] {
        assert!(
            dump.lines().any(|l| l.trim_start().starts_with(line)),
            "missing node line {line}"
        );
    }
    assert!(dump.contains("spread=true"));
    assert!(dump.contains("optional=true"));
    assert!(dump.contains("type_only=true"));
    assert!(dump.contains("exported=true"));
    assert!(dump.contains("default=true"));
    assert!(dump.contains("do-first=true"));
    assert!(dump.contains("const=true"));
    assert!(dump.contains("name=\"\""));
    assert!(dump.contains("\\x"));
}

fn has_expr(dump: &str, kind: &str) -> bool {
    dump.lines().any(|line| {
        let mut parts = line.split_whitespace();
        parts.next() == Some("expr")
            && parts.next().is_some_and(|loc| loc.starts_with("loc="))
            && parts.next() == Some(kind)
    })
}

fn has_stmt(dump: &str, kind: &str) -> bool {
    dump.lines().any(|line| {
        let mut parts = line.split_whitespace();
        parts.next() == Some("stmt")
            && parts.next().is_some_and(|loc| loc.starts_with("loc="))
            && parts.next() == Some(kind)
    })
}
