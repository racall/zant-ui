//! Loop and switch syntax, kept separate from the module and expression parsers.

use crate::ast::{Case, Decl, ExprData, For, ForOf, Name, Pattern, Stmt, StmtData, Switch, While};
use crate::expr_parser;
use crate::lexer::Kind;
use crate::parser::Parser;

pub type Error = crate::error::ParseError;

/// Parse a statement whose leading keyword has already been recognized.
pub fn parse(p: &mut Parser) -> Result<Box<Stmt>, Error> {
    let loc = p.cur.start;
    if p.is_kw("for") {
        return parse_for(p);
    }
    if p.is_kw("switch") {
        return parse_switch(p);
    }
    if p.is_kw("break") || p.is_kw("continue") {
        let is_break = p.is_kw("break");
        p.advance();
        if !p.cur.nl_before && p.cur.kind == Kind::Identifier {
            return Err(p.fail(p.cur.start, "labeled break and continue are not supported"));
        }
        p.end_statement()?;
        return Ok(Box::new(Stmt {
            loc,
            data: if is_break {
                StmtData::Break
            } else {
                StmtData::Continue
            },
        }));
    }
    let do_first = p.is_kw("do");
    p.advance();
    let body = if do_first {
        Some(p.parse_stmt()?)
    } else {
        None
    };
    if do_first {
        if !p.is_kw("while") {
            return Err(p.unexpected("while"));
        }
        p.advance();
    }
    p.expect_p("(")?;
    let cond = expr_parser::parse_expr(p)?;
    p.expect_p(")")?;
    let body = match body {
        Some(body) => body,
        None => p.parse_stmt()?,
    };
    let result = Box::new(Stmt {
        loc,
        data: StmtData::While(While {
            cond,
            body,
            do_first,
        }),
    });
    if do_first {
        p.end_statement()?;
    }
    Ok(result)
}

fn parse_for(p: &mut Parser) -> Result<Box<Stmt>, Error> {
    let loc = p.cur.start;
    p.advance();
    p.expect_p("(")?;
    let mut init: Option<Box<Stmt>> = None;
    if p.is_kw("let") || p.is_kw("const") {
        let decl_loc = p.cur.start;
        let is_const = p.is_kw("const");
        p.advance();
        // `for (const [key, value] of entries)`: a hidden loop variable whose
        // elements the body destructures first
        let elements = if p.is_p("[") {
            Some(parse_elements(p)?)
        } else {
            None
        };
        let name: Name = match &elements {
            Some(_) => Name {
                name: format!("_entry{decl_loc}").into_bytes(),
                loc: decl_loc,
            },
            None => p.expect_ident("a loop variable name")?,
        };
        if p.is_kw("of") {
            p.advance();
            let iterable = expr_parser::parse_expr(p)?;
            p.expect_p(")")?;
            let mut body = p.parse_stmt()?;
            if let Some(elements) = elements {
                let unpack = Box::new(Stmt {
                    loc: decl_loc,
                    data: StmtData::Decl(Decl {
                        is_const,
                        pattern: Pattern::Array(elements),
                        ty: None,
                        init: Some(p.expr(decl_loc, ExprData::Ident(name.name.clone()))),
                    }),
                });
                body = Box::new(Stmt {
                    loc: body.loc,
                    data: StmtData::Block(vec![unpack, body]),
                });
            }
            return Ok(Box::new(Stmt {
                loc,
                data: StmtData::ForOf(ForOf {
                    name,
                    is_const,
                    iterable,
                    body,
                }),
            }));
        }
        if elements.is_some() {
            return Err(p.fail(decl_loc, "destructuring loop variables need `of`"));
        }
        if is_const {
            return Err(p.fail(decl_loc, "C-style for initializer must use let"));
        }
        let ty = if p.eat_p(":") {
            Some(p.parse_type()?)
        } else {
            None
        };
        p.expect_p("=")?;
        let value = expr_parser::parse_expr(p)?;
        init = Some(Box::new(Stmt {
            loc: decl_loc,
            data: StmtData::Decl(Decl {
                is_const: false,
                pattern: Pattern::Ident(name),
                ty,
                init: Some(value),
            }),
        }));
    } else if !p.is_p(";") {
        let value = expr_parser::parse_expr(p)?;
        if !matches!(&value.data, ExprData::Assign(_)) {
            return Err(p.fail(
                value.loc,
                "for initializer must be a let declaration or assignment",
            ));
        }
        init = Some(Box::new(Stmt {
            loc: value.loc,
            data: StmtData::Expr(value),
        }));
    }
    p.expect_p(";")?;
    let cond = if !p.is_p(";") {
        Some(expr_parser::parse_expr(p)?)
    } else {
        None
    };
    p.expect_p(";")?;
    let update = if !p.is_p(")") {
        Some(expr_parser::parse_expr(p)?)
    } else {
        None
    };
    if let Some(value) = &update {
        let ok = matches!(&value.data, crate::ast::ExprData::Assign(_))
            || matches!(&value.data, crate::ast::ExprData::Update(_));
        if !ok {
            return Err(p.fail(value.loc, "for update must be an assignment, ++ or --"));
        }
    }
    p.expect_p(")")?;
    Ok(Box::new(Stmt {
        loc,
        data: StmtData::For(For {
            init,
            cond,
            update,
            body: p.parse_stmt()?,
        }),
    }))
}

fn parse_switch(p: &mut Parser) -> Result<Box<Stmt>, Error> {
    let loc = p.cur.start;
    p.advance();
    p.expect_p("(")?;
    let value = expr_parser::parse_expr(p)?;
    p.expect_p(")")?;
    p.expect_p("{")?;
    let mut cases: Vec<Case> = Vec::new();
    let mut has_default = false;
    while !p.is_p("}") {
        let case_loc = p.cur.start;
        let is_default = p.is_kw("default");
        if !is_default && !p.is_kw("case") {
            return Err(p.unexpected("case or default"));
        }
        if is_default && has_default {
            return Err(p.fail(case_loc, "switch has more than one default"));
        }
        if is_default {
            has_default = true;
        }
        p.advance();
        let test_expr = if is_default {
            None
        } else {
            Some(expr_parser::parse_expr(p)?)
        };
        p.expect_p(":")?;
        let mut body: Vec<Box<Stmt>> = Vec::new();
        while !p.is_p("}") && !p.is_kw("case") && !p.is_kw("default") {
            if p.eat_p(";") {
                continue;
            }
            body.push(p.parse_stmt()?);
        }
        cases.push(Case {
            loc: case_loc,
            value: test_expr,
            body,
        });
    }
    p.advance();
    Ok(Box::new(Stmt {
        loc,
        data: StmtData::Switch(Switch { value, cases }),
    }))
}

/// `[a, , b]` in a loop head.
fn parse_elements(p: &mut Parser) -> Result<Vec<Option<Name>>, Error> {
    p.advance(); // [
    let mut names: Vec<Option<Name>> = Vec::new();
    while !p.is_p("]") {
        if p.is_p(",") {
            p.advance();
            names.push(None);
            continue;
        }
        names.push(Some(p.expect_ident("a name")?));
        if !p.eat_p(",") {
            break;
        }
    }
    p.expect_p("]")?;
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Body, ExprData, Pattern};
    use crate::parser::{expect_syntax_error, parse_ok};

    fn block(f: &crate::ast::Function) -> &[Box<Stmt>] {
        match &f.body {
            Body::Block(stmts) => stmts,
            Body::Expr(_) => panic!("expected a block body"),
        }
    }

    fn text(value: &[u8]) -> &str {
        std::str::from_utf8(value).expect("text is UTF-8")
    }

    #[test]
    fn loop_headers_single_statements_and_switch_groups() {
        let m = parse_ok(
            r#"function f() {
  for (let i: number = 0; i < 4; i++) total += i;
  for (i = 0; ; --i) { break; }
  for (;;) break;
  for (const x of xs) { continue; }
  for (let y of [1, 2]) y++;
  while (ok) count++;
  do { count--; } while (ok)
  next()
  switch (value) { case 1: break; case 2: case 3: return 4; default: return 0; }
}"#,
        );
        let ss = block(&m.functions[0].func);
        assert_eq!(ss.len(), 9);
        let classic = match &ss[0].data {
            StmtData::For(v) => v,
            _ => panic!("expected a for loop"),
        };
        let init = classic.init.as_ref().expect("initializer");
        let decl = match &init.data {
            StmtData::Decl(d) => d,
            _ => panic!("expected a declaration"),
        };
        match &decl.pattern {
            Pattern::Ident(name) => assert_eq!("i", text(&name.name)),
            _ => panic!("expected an identifier pattern"),
        }
        assert_eq!("number", text(&decl.ty.as_ref().expect("type").name));
        assert!(matches!(
            &classic.cond.as_ref().expect("condition").data,
            ExprData::Binary(_)
        ));
        assert!(matches!(
            &classic.update.as_ref().expect("update").data,
            ExprData::Update(_)
        ));
        assert!(matches!(&classic.body.data, StmtData::Expr(_)));
        assert!(matches!(&ss[1].data, StmtData::For(v) if v.cond.is_none()));
        assert!(matches!(
            &ss[1].data,
            StmtData::For(v) if matches!(&v.init.as_ref().expect("init").data, StmtData::Expr(_))
        ));
        assert!(matches!(
            &ss[2].data,
            StmtData::For(v) if v.init.is_none() && v.update.is_none()
        ));
        assert!(matches!(&ss[3].data, StmtData::ForOf(v) if v.is_const));
        assert!(matches!(&ss[4].data, StmtData::ForOf(v) if !v.is_const));
        assert!(matches!(&ss[5].data, StmtData::While(v) if !v.do_first));
        assert!(matches!(&ss[6].data, StmtData::While(v) if v.do_first));
        let cases = match &ss[8].data {
            StmtData::Switch(v) => &v.cases,
            _ => panic!("expected a switch"),
        };
        assert_eq!(cases.len(), 4);
        assert!(matches!(&cases[0].body[0].data, StmtData::Break));
        assert!(cases[1].body.is_empty());
        assert!(cases[3].value.is_none());
    }

    #[test]
    fn loop_syntax_diagnostics_and_jump_asi() {
        expect_syntax_error(
            "function f() { while (true {} }",
            "expected `)`, found `{`",
            "{}",
        );
        expect_syntax_error(
            "function f() { for (let i=0; i<3; i++ {} }",
            "expected `)`, found `{`",
            "{}",
        );
        expect_syntax_error(
            "function f() { do count++; while (true }",
            "expected `)`, found `}`",
            "}",
        );
        expect_syntax_error(
            "function f() { break label; }",
            "labeled break and continue are not supported",
            "label",
        );
        expect_syntax_error(
            "function f() { continue label; }",
            "labeled break and continue are not supported",
            "label",
        );
        expect_syntax_error(
            "function f() { for (const i=0;;) {} }",
            "C-style for initializer must use let",
            "const",
        );
        expect_syntax_error(
            "function f() { for (;; step()) {} }",
            "for update must be an assignment, ++ or --",
            "step()",
        );
        expect_syntax_error(
            "function f() { switch (x) { default: default: } }",
            "switch has more than one default",
            "default: }",
        );
        let m = parse_ok("function f() { break\nnext(); continue\nother(); }");
        let ss = block(&m.functions[0].func);
        assert_eq!(ss.len(), 4);
        assert!(matches!(&ss[0].data, StmtData::Break));
        assert!(matches!(&ss[2].data, StmtData::Continue));
    }
}
