//! Unit tests.

use super::*;

#[test]
fn module_paths_strip_src_and_the_mod_file() {
    let root = Path::new("/repo/crates/zant/src");
    assert_eq!(
        module_path_for(&root.join("ui/builders/mod.rs")),
        ["crate", "ui", "builders"]
    );
    assert_eq!(
        module_path_for(&root.join("core/layout/mod.rs")),
        ["crate", "core", "layout"]
    );
    assert_eq!(module_path_for(&root.join("flex.rs")), ["crate", "flex"]);
}

#[test]
fn type_helpers_read_generic_types() {
    let ty: syn::Type = syn::parse_quote!(Option<Rc<Image>>);
    assert_eq!(last_type_ident(&ty).as_deref(), Some("Option"));
    assert_eq!(type_arguments(&ty).len(), 1);
    assert_eq!(
        last_type_ident(type_arguments(&ty)[0]).as_deref(),
        Some("Rc")
    );
    let path: syn::Type = syn::parse_quote!(flex::Wrap);
    assert_eq!(type_segments(&path).unwrap(), ["flex", "Wrap"]);
}

#[test]
fn split_modules_resolve_paths_through_glob_imports() {
    // `ui/options/` split into files: the child only has `use super::*;` and
    // names the enum through the parent's `use crate::renderer::text;` import.
    let dir = std::env::temp_dir().join(format!("zant-tags-glob-{}", std::process::id()));
    let src = dir.join("src");
    std::fs::create_dir_all(src.join("ui/options")).unwrap();
    std::fs::create_dir_all(src.join("renderer")).unwrap();
    std::fs::write(src.join("lib.rs"), "pub mod renderer;\npub mod ui;\n").unwrap();
    std::fs::write(src.join("renderer/mod.rs"), "pub mod text;\n").unwrap();
    std::fs::write(
        src.join("renderer/text.rs"),
        "pub enum Align { Left, Right }\n",
    )
    .unwrap();
    std::fs::write(src.join("ui/mod.rs"), "pub mod options;\n").unwrap();
    std::fs::write(
        src.join("ui/options/mod.rs"),
        "use crate::renderer::text;\nmod typography;\npub use self::typography::*;\n",
    )
    .unwrap();
    std::fs::write(
        src.join("ui/options/typography.rs"),
        "use super::*;\npub struct TextOptions { pub align: text::Align }\n",
    )
    .unwrap();
    let index = Index::load(&src);
    std::fs::remove_dir_all(&dir).unwrap();
    let module = ["crate", "ui", "options", "typography"].map(String::from);
    let path = ["text", "Align"].map(String::from);
    match index.resolve(&module, &path) {
        Some(Resolved::Enum(variants)) => assert_eq!(variants, ["Left", "Right"]),
        _ => panic!("text::Align did not resolve through `use super::*`"),
    }
}
