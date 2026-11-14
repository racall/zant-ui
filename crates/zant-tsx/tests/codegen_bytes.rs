//! `Uint8Array` → `&'static [u8]`: a TSX `Uint8Array` is the
//! runtime's static byte slice, the type of imported assets
//! (`include_bytes!`), `TrayOptions::{icon, iconLight}` and
//! `tsx::platform::asset_image`. Byte literals become a promoted `&[..]` slice
//! (`const icon: Uint8Array = [137, 80, 78, 71]` → `&[137u8, 80u8, 78u8, 71u8]`).
//!
//! The type is intentionally *not* `tsx::List<u8>`: the tray icon must be a
//! `'static` slice, and `List` is a runtime `Vec`. Dynamic bytes from the
//! clipboard stay `tsx::Bytes` (`Rc<[u8]>`) inside the `"zant"` function
//! signatures; those are runtime values, not literals.
//!
//! `testdata/tsx/desktop.tsx` is the end-to-end case: it is the
//! `desktop` golden in `tests/golden.rs`, compiled against the runtime there.

use zant_tsx::compile::{self, SourceFile};

/// Generate one TSX file; the generated Rust is prettyplease-formatted, so the
/// assertions compare it with all whitespace removed.
fn generated(source: &str) -> String {
    let generated =
        compile::compile(source, "bytes.tsx").unwrap_or_else(|error| panic!("{}", error.msg));
    generated.split_whitespace().collect()
}

fn diagnostic(source: &str) -> String {
    compile::compile(source, "bytes.tsx")
        .expect_err("the source must be rejected")
        .msg
}

/// A component that registers a tray icon from a module constant.
fn tray_source(icon: &str) -> String {
    format!(
        "import {{ useState, useTray, View, Text }} from \"zant\";\n\
         {icon}\
         export default function App() {{\n\
         const [clips, setClips] = useState(0);\n\
         const shown = useTray({{ icon, tooltip: \"Clips\", items: [\n\
         {{ title: \"Open\", onSelect: () => setClips(0) }},\n\
         ] }});\n\
         return <View><Text key=\"t\">{{clips}} {{shown ? \"tray\" : \"no tray\"}}</Text></View>;\n\
         }}\n"
    )
}

#[test]
fn a_uint8array_literal_is_a_promoted_static_slice() {
    let source = generated(&tray_source(
        "const icon: Uint8Array = [137, 80, 78, 71];\n",
    ));
    // the module constant is a `&'static [u8]` accessor (Rust `const` items
    // cannot allocate, so every TSX constant is a `thread_local!` accessor)
    assert!(
        source.contains("fnicon()->&'static[u8]{thread_local!{static_icon_VALUE:&'static[u8]=&[137u8,80u8,78u8,71u8];}_icon_VALUE.with(|value|value.clone())}"),
        "{source}"
    );
    assert!(source.contains("icon:icon(),"), "{source}");
    assert!(
        source.contains("tsx::desktop::update(tsx::desktop::TrayOptions{"),
        "{source}"
    );
}

#[test]
fn a_uint8array_literal_is_accepted_inline() {
    let source = generated(
        "import { useTray, View, Text } from \"zant\";\n\
         export default function App() {\n\
         const shown = useTray({ icon: [1, 2, 3], tooltip: \"Clips\", items: [] });\n\
         return <View><Text key=\"t\">{shown ? \"tray\" : \"no tray\"}</Text></View>;\n\
         }\n",
    );
    assert!(source.contains("icon:&[1u8,2u8,3u8],"), "{source}");
}

#[test]
fn an_empty_uint8array_literal_is_an_empty_slice() {
    let source = generated(&tray_source("const icon: Uint8Array = [];\n"));
    assert!(
        source.contains("static_icon_VALUE:&'static[u8]=&[];"),
        "{source}"
    );
}

#[test]
fn an_imported_asset_is_a_uint8array() {
    // an asset import already emits `const logo: &'static [u8] =
    // include_bytes!(..)`; annotating it as `Uint8Array` must be the same type
    let files = [SourceFile::tsx(
        "src/App.tsx",
        "import trayIcon from \"./assets/tray.png\";\n\
         import { useTray, View, Text } from \"zant\";\n\
         const icon: Uint8Array = trayIcon;\n\
         export default function App() {\n\
         const shown = useTray({ icon, tooltip: \"Clips\", items: [] });\n\
         return <View><Text key=\"t\">{shown ? \"tray\" : \"no tray\"}</Text></View>;\n\
         }\n",
    )];
    let project = compile::compile_project_with(&files, None)
        .unwrap_or_else(|error| panic!("{}", error.diag.msg));
    let source: String = project.outputs[0].split_whitespace().collect();
    assert!(
        source.contains("consttrayIcon:&'static[u8]=include_bytes!(\"_assets/"),
        "{source}"
    );
    assert!(source.contains("fnicon()->&'static[u8]"), "{source}");
}

#[test]
fn an_untyped_byte_literal_points_at_the_annotation() {
    let message = diagnostic(&tray_source("const icon = [137, 80];\n"));
    assert!(
        message.contains("annotate byte literals with `const name: Uint8Array = [..]`"),
        "{message}"
    );
}

#[test]
fn an_out_of_range_byte_literal_is_rejected() {
    let message = diagnostic(&tray_source("const icon: Uint8Array = [256];\n"));
    assert!(
        message.contains("integer literal does not fit u8"),
        "{message}"
    );
}

#[test]
fn a_uint8array_literal_cannot_hold_other_values() {
    let message = diagnostic(&tray_source(
        "const icon: Uint8Array = [137, 80, 78, 71, Math.max(1, 2)];\n",
    ));
    assert!(
        message.contains("a Uint8Array literal contains only integer byte literals"),
        "{message}"
    );
}

#[test]
fn a_uint8array_literal_cannot_hold_a_spread() {
    let message = diagnostic(&tray_source(
        "const other: Uint8Array = [1];\nconst icon: Uint8Array = [...other];\n",
    ));
    assert!(
        message.contains("a Uint8Array literal contains only integer byte literals"),
        "{message}"
    );
}

#[test]
fn a_rust_module_byte_function_is_a_uint8array() {
    // bytes from a hand-written `native/`-style `.rs` module (`&'static [u8]`,
    // the same spelling asset imports use) are a `Uint8Array` too
    let files = [
        SourceFile::tsx(
            "src/App.tsx",
            "import { useTray, View, Text } from \"zant\";\n\
             import { trayPng } from \"./bytes.rs\";\n\
             const icon: Uint8Array = trayPng();\n\
             export default function App() {\n\
             const shown = useTray({ icon, tooltip: \"Clips\", items: [] });\n\
             return <View><Text key=\"t\">{shown ? \"tray\" : \"no tray\"}</Text></View>;\n\
             }\n",
        ),
        SourceFile::rust(
            "src/bytes.rs",
            "pub fn trayPng() -> &'static [u8] {\n    &[137, 80, 78, 71]\n}\n",
        ),
    ];
    let project = compile::compile_project_with(&files, None)
        .unwrap_or_else(|error| panic!("{}", error.diag.msg));
    let source: String = project.outputs[0].split_whitespace().collect();
    assert!(
        source.contains("static_icon_VALUE:&'static[u8]=crate::bytes::trayPng();"),
        "{source}"
    );
    assert!(source.contains("icon:icon(),"), "{source}");
}
