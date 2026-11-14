//! Import clauses, including React's ignored default / namespace bindings.
//!
//! Direct translation of `tools/tsx/import_parser.zig`. `assets.isAsset`
//! (from `tools/tsx/assets.zig`) is inlined here as [`is_asset`]: the Rust
//! `assets` module belongs to a later WP.

use crate::ast::{Import, ImportName, Name};
use crate::lexer::{self, Kind};
use crate::parser::{Parser, bytes_lossy};

pub type Error = crate::error::ParseError;

/// Parse an import; only React permits default and namespace clauses.
pub fn parse(p: &mut Parser) -> Result<Import, Error> {
    let loc = p.cur.start;
    p.advance(); // import
    let mut type_only = false;
    if p.is_kw("type") {
        type_only = true;
        p.advance();
    }
    let mut names: Vec<ImportName> = Vec::new();
    let mut default_name: Option<Name> = None;
    let mut namespace_name: Option<Name> = None;
    if p.cur.kind == Kind::String {
        return Err(p.fail(p.cur.start, "side-effect imports are not supported"));
    }
    if p.cur.kind == Kind::Identifier && !p.is_kw("from") {
        default_name = Some(p.expect_ident("a local name")?);
        if !p.eat_p(",") {
            if !p.is_kw("from") {
                return Err(p.unexpected("from"));
            }
        } else if !p.is_p("{") && !p.is_p("*") {
            return Err(p.unexpected("{ or *"));
        }
    }
    if p.eat_p("*") {
        if !p.is_kw("as") {
            return Err(p.unexpected("as"));
        }
        p.advance();
        namespace_name = Some(p.expect_ident("a local name")?);
    } else if default_name.is_none() || p.is_p("{") {
        p.expect_p("{")?;
        while !p.is_p("}") {
            let mut name_type_only = false;
            if p.is_kw("type") {
                // `import { type Foo }`: retain the modifier for Rust imports
                let saved = p.save();
                p.advance();
                if p.cur.kind != Kind::Identifier {
                    p.restore(saved);
                } else {
                    name_type_only = true;
                }
            }
            let imported = p.expect_ident("an imported name")?;
            let mut local = imported.clone();
            if p.is_kw("as") {
                p.advance();
                local = p.expect_ident("a local name")?;
            }
            names.push(ImportName {
                imported,
                local,
                type_only: name_type_only,
            });
            if !p.eat_p(",") {
                break;
            }
        }
        p.expect_p("}")?;
    }
    if !p.is_kw("from") {
        return Err(p.unexpected("from"));
    }
    p.advance();
    if p.cur.kind != Kind::String {
        return Err(p.unexpected("a module name"));
    }
    let source_loc = p.cur.start;
    let source = match lexer::decode_string(p.text().as_bytes()) {
        Ok(source) => source,
        Err(_) => return Err(p.fail(source_loc, "invalid escape in string")),
    };
    p.advance();
    p.end_statement()?;
    let asset = is_asset(&source);
    if asset && (!names.is_empty() || namespace_name.is_some()) {
        let msg = format!(
            "an asset is imported by a default name: import icon from \"{}\"",
            bytes_lossy(&source)
        );
        return Err(p.fail(source_loc, msg));
    }
    if asset && default_name.is_none() {
        let msg = format!(
            "import an asset by a default name: import icon from \"{}\"",
            bytes_lossy(&source)
        );
        return Err(p.fail(source_loc, msg));
    }
    if source.as_slice() != b"react".as_slice() && !asset {
        if let Some(name) = &default_name {
            return Err(p.fail(
                name.loc,
                "default imports are only supported from \"react\" and for assets (import icon from \"./icon.png\")",
            ));
        }
        if let Some(name) = &namespace_name {
            return Err(p.fail(
                name.loc,
                "namespace imports are only supported from \"react\"",
            ));
        }
    }
    Ok(Import {
        loc,
        source,
        source_loc,
        type_only,
        names,
        default_name,
        namespace_name,
    })
}

/// `assets.isAsset`: a relative import whose extension is not code names a
/// file whose bytes are embedded.
fn is_asset(source: &[u8]) -> bool {
    let relative = source.starts_with(b"./") || source.starts_with(b"../");
    if !relative {
        return false;
    }
    let ext = extension(source);
    if ext.is_empty() {
        return false;
    }
    for code in [&b".tsx"[..], b".ts", b".zig", b".jsx", b".js"] {
        if ext == code {
            return false;
        }
    }
    true
}

/// `std.fs.path.basename` for `/`-separated module specifiers.
fn basename(path: &[u8]) -> &[u8] {
    match path.iter().rposition(|&b| b == b'/') {
        Some(i) => &path[i + 1..],
        None => path,
    }
}

/// `std.fs.path.extension`: empty for a name without a dot or a leading dot.
fn extension(path: &[u8]) -> &[u8] {
    let filename = basename(path);
    match filename.iter().rposition(|&b| b == b'.') {
        None => b"",
        Some(0) => b"",
        Some(i) => &filename[i..],
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::Body;
    use crate::parser::parse_ok;

    fn text(value: &[u8]) -> &str {
        std::str::from_utf8(value).expect("text is UTF-8")
    }

    // Ported from `tools/tsx/parser_test.zig`,
    // "React default, namespace and mixed imports retain their bindings".
    #[test]
    fn react_default_namespace_and_mixed_imports_retain_their_bindings() {
        let m = parse_ok(
            r#"import R from "react";
import * as NS from "react";
import React, { useState, type ReactNode } from "react";
import { useState, useEffect } from "react";
import Default, * as Space from "react";
function App() { return <React.Fragment><Text/></React.Fragment>; }"#,
        );
        assert_eq!(
            "R",
            text(&m.imports[0].default_name.as_ref().expect("default").name)
        );
        assert_eq!(
            "NS",
            text(
                &m.imports[1]
                    .namespace_name
                    .as_ref()
                    .expect("namespace")
                    .name
            )
        );
        assert_eq!(
            "React",
            text(&m.imports[2].default_name.as_ref().expect("default").name)
        );
        assert_eq!(m.imports[2].names.len(), 2);
        assert!(m.imports[2].names[1].type_only);
        assert_eq!(m.imports[3].names.len(), 2);
        assert_eq!(
            "Default",
            text(&m.imports[4].default_name.as_ref().expect("default").name)
        );
        assert_eq!(
            "Space",
            text(
                &m.imports[4]
                    .namespace_name
                    .as_ref()
                    .expect("namespace")
                    .name
            )
        );
        let body = match &m.functions[0].func.body {
            Body::Block(stmts) => stmts,
            _ => panic!("expected a block body"),
        };
        let jsx = match &body[0].data {
            crate::ast::StmtData::Ret(Some(e)) => match &e.data {
                crate::ast::ExprData::Jsx(j) => j,
                _ => panic!("expected JSX"),
            },
            _ => panic!("expected a return"),
        };
        assert_eq!("", text(&jsx.name));
    }

    // Ported from `tools/tsx/assets.zig`, "asset imports by extension"; the
    // helper is inlined in this module for now.
    #[test]
    fn is_asset_by_extension() {
        use super::is_asset;
        assert!(is_asset(b"../assets/tray.png"));
        assert!(is_asset(b"./data/words.txt"));
        assert!(!is_asset(b"./components/Row"));
        assert!(!is_asset(b"./Row.tsx"));
        assert!(!is_asset(b"./sync.zig"));
        assert!(!is_asset(b"zant"));
        assert!(!is_asset(b"./.hidden"));
    }
}
