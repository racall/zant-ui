use super::*;

#[test]
fn rust_under_src_is_rejected_at_any_depth() {
    let root = temp_dir("rust-in-src");
    copy_tree(&fixture_root(), &root);
    std::fs::write(root.join("src/components/.helper.rs"), "pub fn helper() {}").unwrap();
    let out = temp_dir("rust-in-src-out");
    let error = zant_build::tsx_app_in(&root, &out).unwrap_err().to_string();
    assert!(
        error.contains("helper.rs") && error.contains("move Rust code into native/"),
        "{error}"
    );
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(out).unwrap();
}

#[test]
fn native_helpers_and_specs_are_declared_and_only_specs_are_registered() {
    let root = app_with_native_spec("native-tree");
    std::fs::write(root.join("native/clip.rs"), "pub struct Native;").unwrap();
    std::fs::write(root.join("native/helper.rs"), "pub mod internal;").unwrap();
    std::fs::create_dir_all(root.join("native/internal")).unwrap();
    std::fs::write(
        root.join("native/internal/mod.rs"),
        "pub const VALUE: u32 = 1;",
    )
    .unwrap();
    let out = temp_dir("native-tree-out");
    zant_build::tsx_app_in(&root, &out).unwrap();
    let app = read(&out.join("zant_app.rs"));
    for declaration in [
        "pub mod clip;",
        "pub mod helper;",
        "pub mod spec",
        "pub mod app_config",
    ] {
        assert!(app.contains(declaration), "{app}");
    }
    assert!(app.contains("register(clip::Native)"));
    for absent in [
        "register(helper::Native)",
        "pub mod internal;",
        "pub mod main;",
        "pub mod build;",
    ] {
        assert!(!app.contains(absent), "{app}");
    }
    assert!(!root.join("native/spec.rs").exists());
    let main = read(&out.join("zant_main.rs"));
    let hot: String = read(&out.join("zant_hot.rs")).split_whitespace().collect();
    assert!(
        main.contains("native::register()")
            && main.contains("zant::tsx::app::run::<")
            && main.contains("crate::app::App")
    );
    assert!(
        hot.contains("native::register()")
            && hot.contains("zant::tsx::app::prepare::<")
            && hot.contains("crate::app::App")
    );
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(out).unwrap();
}
