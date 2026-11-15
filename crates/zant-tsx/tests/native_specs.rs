//! Native module specs (`native/<name>.ts`) compiled to Rust traits.
//!
//! Covers the spec side (the app-side imports are tested with the application
//! layer), the golden `native/spec.rs` fixture — asserted byte-for-byte and
//! `include!`d, so the generated trait and types are compiled and type-checked
//! by rustc — and a registry round-trip with the generated `NAME` key.

use zant_tsx::ast;
use zant_tsx::error::Diag;
use zant_tsx::native_specs::{self, Imported, Spec};
use zant_tsx::parser::Parser;

/// `native/clips.ts`: a spec with the constructs the Rust trait must map
/// (optional args, `T | null`, named aliases, a shared type).
const CLIPS: &str = r#"import type { i64, u32 } from "zant";
import type { Kind } from "./kinds";
export interface ListEntry {
  row: i64;
  kind: Kind;
  preview: string;
  from: string | null;
  pinned?: boolean;
}
export type CopyResult = "ok" | "not_found" | "missing_content";
export type Entries = ListEntry[];
export type Probe = { hint: string };
export type ProbeList = Probe[];
export declare function entries(query: string): ListEntry[];
export declare function copy(row: i64): CopyResult;
export declare function clear(): void;
export declare function count(kind: Kind, limit?: u32): i64;
export declare function line(row: i64): string | null;
"#;

const KINDS: &str = r#"export type Kind = "text" | "image" | "files";
"#;

/// The golden generated `native/spec.rs` for [`CLIPS`] + [`KINDS`].
const GOLDEN: &str = include_str!("fixtures/native_spec.rs");

/// The golden file compiled into this test: the generated trait must be
/// implementable, and a mismatch would be a rustc error right here.
mod golden {
    include!("fixtures/native_spec.rs");
}

use golden::clips::ClipsSpec;

fn parse(src: &str) -> ast::Module {
    let mut parser = Parser::init(src);
    match parser.parse_module() {
        Ok(module) => module,
        Err(_) => match parser.diag {
            Some(diag) => panic!("parse error at {}: {}", diag.loc, diag.msg),
            None => panic!("parse error without a diagnostic"),
        },
    }
}

/// The project's specs. `clips` imports a type from `kinds`.
fn project<'a>(clips: &'a ast::Module, kinds: &'a ast::Module) -> Vec<Spec<'a>> {
    vec![
        Spec {
            name: "clips",
            module: clips,
        },
        Spec {
            name: "kinds",
            module: kinds,
        },
    ]
}

fn generate(clips: &ast::Module, specs: &[Spec<'_>]) -> String {
    let mut diag = None;
    let source = native_specs::generate(clips, "clips", specs, &mut diag)
        .unwrap_or_else(|_| panic!("generation failed: {diag:?}"));
    assert!(diag.is_none(), "unexpected diagnostic: {diag:?}");
    source
}

/// The first diagnostic of a spec that cannot generate.
fn error(clips_src: &str) -> Diag {
    let clips = parse(clips_src);
    let mut diag = None;
    let result = native_specs::generate(&clips, "clips", &[], &mut diag);
    assert!(result.is_err(), "`{clips_src}` must be rejected");
    diag.expect("a rejected spec without a diagnostic")
}

// A spec compiles to its types and a conformance check: the `ClipsSpec` trait.
#[test]
fn a_spec_compiles_to_its_types_and_a_trait() {
    let clips = parse(CLIPS);
    let kinds = parse(KINDS);
    let specs = project(&clips, &kinds);
    let spec = generate(&clips, &specs);
    // the shared enum lives in the other spec's module
    assert!(
        spec.contains("pub kind: super::kinds::Kind"),
        "the imported type is spelled through the spec module: {spec}"
    );
    assert!(spec.contains("pub struct ListEntry"), "{spec}");
    assert!(spec.contains("pub enum CopyResult"), "{spec}");
    // the conformance check: one trait method per declared function
    assert!(spec.contains("pub trait ClipsSpec"), "{spec}");
    assert!(
        spec.contains("fn entries(query: zant::Str) -> tsx::List<ListEntry>;"),
        "{spec}"
    );
    assert!(spec.contains("fn copy(row: i64) -> CopyResult;"), "{spec}");
    // `void` returns no value
    assert!(spec.contains("fn clear();"), "{spec}");
    // an optional argument is `Option<T>`
    assert!(
        spec.contains("fn count(kind: super::kinds::Kind, limit: Option<u32>) -> i64;"),
        "{spec}"
    );
    // `T | null` is `Option<T>`
    assert!(
        spec.contains("fn line(row: i64) -> Option<zant::Str>;"),
        "{spec}"
    );
    assert!(
        spec.contains(r#"pub const NAME: &str = "clips";"#),
        "{spec}"
    );
    // the generated module parses as Rust
    syn::parse_file(&spec).unwrap_or_else(|error| panic!("{error}\n{spec}"));
    // the application reaches the same types through the spec module, and the
    // `.tsx` call becomes a trait call: no second struct
    assert!(
        spec.contains("pub type Entries = tsx::List<ListEntry>;"),
        "{spec}"
    );
    let kinds = generate(&kinds, &specs);
    assert!(kinds.contains("pub enum Kind"), "{kinds}");
}

// The spec root lists every native module; a spec without declared functions
// (a shared types file) has no trait to implement, so no empty conformance
// check.
#[test]
fn the_spec_root_lists_every_spec() {
    let clips = parse(CLIPS);
    let kinds = parse(KINDS);
    let specs = project(&clips, &kinds);
    let mut diag = None;
    let root = native_specs::spec_root(&specs, &mut diag).expect("generates");
    assert!(diag.is_none());
    assert!(root.contains("// @generated by zant-tsx"), "{root}");
    assert!(root.contains("pub mod clips"), "{root}");
    assert!(root.contains("pub mod kinds"), "{root}");
    assert!(root.contains("pub trait ClipsSpec"), "{root}");
    if std::env::var_os("ZANT_TSX_WRITE_GOLDENS").is_some() {
        std::fs::write(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/native_spec.rs"),
            &root,
        )
        .unwrap();
    } else {
        assert_eq!(GOLDEN, root, "the golden fixture is the generated output");
    }
    // deterministic: name order, not project order
    let mut sorted = specs.clone();
    sorted.reverse();
    let mut diag = None;
    assert_eq!(
        root,
        native_specs::spec_root(&sorted, &mut diag).expect("generates")
    );
    // a types-only spec has no trait and no registry key
    let kinds = generate(&kinds, &specs);
    assert!(!kinds.contains("pub trait"), "{kinds}");
    assert!(!kinds.contains("NAME"), "{kinds}");
    assert!(!native_specs::has_functions(&parse(KINDS)));
    assert!(native_specs::has_functions(&parse(CLIPS)));
    // two specs with one name would generate one module twice
    let duplicates = vec![specs[0], specs[0]];
    let mut diag = None;
    assert!(native_specs::spec_root(&duplicates, &mut diag).is_err());
    assert_eq!(
        "duplicate native module spec `clips`",
        diag.expect("a diagnostic").msg
    );
}

// The app-side import errors (missing export / missing spec) are produced here
// by the helpers the application layer calls; the `declare function` outside a
// spec stays in the module validation of the component layer.
#[test]
fn spec_errors_point_at_the_spec() {
    // a function with a body belongs in the implementation
    let diag = error("export function copy(row: number): number { return row; }");
    assert_eq!(
        "a spec declares functions (`export declare function copy(…): …;`); write them in native/clips.rs",
        diag.msg
    );
    // ... and the caret points at the spec, not at the app
    assert!(diag.loc > 0 && (diag.loc as usize) < "export function copy".len() + 1);

    // constants belong in the implementation
    let diag = error("export const version: string = \"1\";");
    assert_eq!(
        "a spec has types and declared functions only; constants belong in native/clips.rs",
        diag.msg
    );
    // the declared function must be exported
    let diag = error("declare function copy(row: i64): void;");
    assert_eq!(
        "export the declared function: `export declare function copy`",
        diag.msg
    );
    // both sides name object and union types
    let diag = error("export interface A { nested: { row: i64 } }");
    assert_eq!(
        "name an object type the Rust side uses: `export type Name = …;` / `export interface Name { … }`",
        diag.msg
    );
    let diag = error(r#"export interface A { kind: "text" | "image" }"#);
    assert_eq!(
        "name a union the Rust side uses: `export type Name = …;` / `export interface Name { … }`",
        diag.msg
    );
    let diag = error("export interface A { on_click: () => void }");
    assert_eq!(
        "native functions take and return data, not callbacks",
        diag.msg
    );

    // app side: `import { copy } from "native/clips"`
    let clips = parse(CLIPS);
    let kinds = parse(KINDS);
    let specs = project(&clips, &kinds);
    let clips_spec = native_specs::find(&specs, "clips").expect("found");
    let imported = ast::ImportName {
        type_only: false,
        imported: ast::Name {
            name: b"paste".to_vec(),
            loc: 10,
        },
        local: ast::Name {
            name: b"paste".to_vec(),
            loc: 10,
        },
    };
    let diag = native_specs::import(clips_spec, &imported, false).unwrap_err();
    assert_eq!("native/clips.ts declares no exported `paste`", diag.msg);
    assert_eq!(10, diag.loc);
    // a function imported as a type (and the other way around)
    let function = ast::ImportName {
        type_only: true,
        imported: ast::Name {
            name: b"copy".to_vec(),
            loc: 4,
        },
        local: ast::Name {
            name: b"copy".to_vec(),
            loc: 4,
        },
    };
    let diag = native_specs::import(clips_spec, &function, function.type_only).unwrap_err();
    assert_eq!("`copy` is a function", diag.msg);
    // a spec that does not exist yet
    let diag = native_specs::spec_for(&specs, "native/files", 7).unwrap_err();
    assert_eq!(
        "cannot find `native/files`: add native/files.ts (the spec) and native/files.rs",
        diag.msg
    );
    assert_eq!(7, diag.loc);
    // a relative import inside a spec
    let diag = native_specs::spec_for(&specs, "./missing", 3).unwrap_err();
    assert_eq!(
        "cannot find `./missing`: add native/missing.ts (the spec) and native/missing.rs",
        diag.msg
    );
}

/// An imported name of a `.tsx` file (`import { name } from "native/clips"`).
fn imported(name: &str, loc: ast::Loc, type_only: bool) -> ast::ImportName {
    ast::ImportName {
        type_only,
        imported: ast::Name {
            name: name.as_bytes().to_vec(),
            loc,
        },
        local: ast::Name {
            name: name.as_bytes().to_vec(),
            loc,
        },
    }
}

#[test]
fn an_imported_name_resolves_to_its_spec_declaration() {
    let clips = parse(CLIPS);
    let kinds = parse(KINDS);
    let specs = project(&clips, &kinds);
    let clips_spec = native_specs::find(&specs, "clips").expect("found");
    // a declared function ...
    match native_specs::import(clips_spec, &imported("copy", 1, false), false) {
        Ok(Imported::Function(function)) => assert_eq!(b"copy", function.name.as_slice()),
        other => panic!("expected the declared function, got {other:?}"),
    }
    // ... a type imported as a type ...
    match native_specs::import(clips_spec, &imported("ListEntry", 1, true), true) {
        Ok(Imported::Type(decl)) => assert_eq!(b"ListEntry", decl.name.name.as_slice()),
        other => panic!("expected the exported type, got {other:?}"),
    }
    // ... and a spec found by its import source
    assert_eq!(
        "clips",
        native_specs::spec_for(&specs, "native/clips", 0)
            .expect("found")
            .name
    );
}

#[test]
fn module_names_and_trait_names_are_derived_from_the_spec_file() {
    assert_eq!(Some("clips"), native_specs::module_name("native/clips"));
    assert_eq!(None, native_specs::module_name("native/clips.ts"));
    assert_eq!(None, native_specs::module_name("native/engine/clips"));
    assert_eq!(None, native_specs::module_name("./clips"));
    assert_eq!("clips", native_specs::module_ident("clips"));
    assert_eq!("type_", native_specs::module_ident("type"));
    assert_eq!("ClipsSpec", native_specs::trait_name("clips"));
    assert_eq!("FileSystemSpec", native_specs::trait_name("file_system"));
    // the paths generated call sites and the app's impl use
    assert_eq!(
        "crate::native::spec::clips",
        native_specs::app_spec_path("clips")
    );
    assert_eq!(
        "crate::native::spec::clips::ClipsSpec",
        native_specs::app_trait_path("clips")
    );
    assert_eq!(
        "crate::native::clips::Native",
        native_specs::app_impl_path("clips")
    );
    assert_eq!(
        "crate::native::spec::clips::ListEntry",
        native_specs::app_type_path("clips", "ListEntry")
    );
}

/// The test's implementation of the generated `ClipsSpec` (what the app writes
/// in `native/clips.rs`).
struct Native;

impl golden::clips::ClipsSpec for Native {
    fn entries(query: zant::Str) -> zant::tsx::List<golden::clips::ListEntry> {
        zant::tsx::List::from_vec(vec![golden::clips::ListEntry {
            row: 1,
            kind: golden::kinds::Kind::Text,
            preview: query,
            from: None,
            pinned: Some(true),
        }])
    }

    fn copy(row: i64) -> golden::clips::CopyResult {
        if row == 0 {
            golden::clips::CopyResult::Ok
        } else {
            golden::clips::CopyResult::NotFound
        }
    }

    fn clear() {}

    fn count(kind: golden::kinds::Kind, limit: Option<u32>) -> i64 {
        i64::from(limit.unwrap_or(0)) + i64::from(!matches!(kind, golden::kinds::Kind::Files))
    }

    fn line(row: i64) -> Option<zant::Str> {
        (row > 0).then(|| zant::Str::copy(&format!("line {row}")))
    }
}

#[test]
fn a_generated_spec_impl_round_trips_through_the_registry() {
    zant::tsx::native::clear();
    assert!(!zant::tsx::native::registered(golden::clips::NAME));
    assert!(zant::tsx::native::register_named(
        golden::clips::NAME,
        Native
    ));
    assert!(zant::tsx::native::registered("clips"));
    assert!(
        !zant::tsx::native::register_named(golden::clips::NAME, Native),
        "the first registration wins"
    );
    let module = zant::tsx::native::get_named::<Native>(golden::clips::NAME).expect("registered");
    assert_eq!(2, std::rc::Rc::strong_count(&module));
    assert!(zant::tsx::native::get_named::<String>("clips").is_none());
    assert!(zant::tsx::native::get_named::<Native>("missing").is_none());
    // the trait calls the generated signatures check are real calls
    let entries = Native::entries("hi".into());
    assert_eq!(1, entries.len());
    assert_eq!("hi", entries[0].preview.as_str());
    // the generated types derive `Clone`/`PartialEq`/`Default` (no `Debug`)
    assert!(golden::kinds::Kind::Text == entries[0].kind);
    assert!(golden::clips::CopyResult::Ok == Native::copy(0));
    assert!(golden::clips::CopyResult::NotFound == Native::copy(2));
    assert_eq!("ok", Native::copy(0).as_str());
    Native::clear();
    assert_eq!(1, Native::count(golden::kinds::Kind::Text, None));
    assert_eq!(4, Native::count(golden::kinds::Kind::Files, Some(4)));
    assert_eq!("line 3", Native::line(3).expect("some").as_str());
    assert!(Native::line(0).is_none());
    zant::tsx::native::clear();
    assert!(zant::tsx::native::get_named::<Native>(golden::clips::NAME).is_none());
}
