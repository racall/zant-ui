//! Unit tests.

use std::rc::Rc;

use super::*;
use crate::codegen::app_state::{ModuleFile, ProjectData, RustSource};
use crate::parser::Parser;

fn project(files: &[(&str, &str)]) -> Rc<ProjectData> {
    let modules = files
        .iter()
        .map(|(path, src)| {
            let mut parser = Parser::init(src);
            ModuleFile {
                name: stem(path),
                path: (*path).to_owned(),
                native: false,
                source: (*src).to_owned(),
                module: parser.parse_module().expect("test source parses"),
            }
        })
        .collect();
    Rc::new(ProjectData {
        modules,
        rust: Vec::<RustSource>::new(),
        assets: std::cell::RefCell::new(Vec::new()),
        root: String::new(),
        config: None,
        window_components: Vec::new(),
        i18n: None,
    })
}

fn parse(src: &str) -> ast::Module {
    let mut parser = Parser::init(src);
    parser.parse_module().expect("test source parses")
}

#[test]
fn module_names_are_snake_case() {
    assert_eq!("todo_row", module_name("TodoRow"));
    assert_eq!("app", module_name("App"));
    assert_eq!("my_file", module_name("my_file"));
}

#[test]
fn process_registers_imported_components_types_stores_and_windows() {
    let project = project(&[
        (
            "src/Row.tsx",
            "export interface Todo { id: u32 }\n\
                 export interface RowProps { todo: Todo }\n\
                 export function Row({ todo }: RowProps) { return <View/>; }\n",
        ),
        (
            "src/store.tsx",
            "export interface Prefs { dark: boolean }\nexport const prefs = createStore<Prefs>({ dark: false });\n",
        ),
        (
            "src/window.tsx",
            "function Panel() { return <View/>; }\nexport const panel = createWindow(Panel, {});\n",
        ),
    ]);
    let module = parse(
        "import { Row, type Todo } from \"./Row\";\n\
             import { prefs } from \"./store\";\n\
             import { panel } from \"./window\";\n\
             export default function App() { return <View/>; }\n",
    );
    let _guard = app_state::set_project(Some(project));
    let mut diag = None;
    let mut g = Generator::new(&mut diag);
    crate::codegen::generate::install(&mut g);
    g.react_imports = module.imports.clone();
    if process(&mut g, &module).is_err() {
        panic!("imports resolve: {:?}", g.diag.clone());
    }
    // the imported component's builder is a crate path
    assert_eq!(1, g.components.len());
    assert_eq!("crate::row::Row::build", g.components[0].builder);
    assert_eq!(
        "crate::row::RowProps",
        g.components[0].props.expect("props type").rust(&g)
    );
    // the imported store keeps its declaring accessors
    assert_eq!(1, g.stores.len());
    assert_eq!("prefs", g.stores[0].declared);
    assert_eq!("crate::store::", g.stores[0].module);
    // the imported window is a handle to the declaring accessor
    let windows = app_state::with_module(|state| state.windows.clone());
    assert_eq!(1, windows.len());
    assert_eq!("crate::window::panel_with", windows[0].rust);
    // the imported type resolves to the other module's declaration
    let imported = app_state::with_module(|state| state.imported_types.clone());
    assert_eq!("Todo", imported[0].imported);
    assert_eq!(
        "crate::row::Todo",
        scope_alias(imported[0].scope) + "::Todo"
    );
}

#[test]
fn an_unknown_import_is_a_diagnostic() {
    let project = project(&[(
        "src/Row.tsx",
        "function Hidden(p: { a: string }) { return <View/>; }\n",
    )]);
    let module = parse(
        "import { Hidden } from \"./Row\";\nexport default function App() { return <View/>; }\n",
    );
    let _guard = app_state::set_project(Some(project));
    let mut diag = None;
    let mut g = Generator::new(&mut diag);
    crate::codegen::generate::install(&mut g);
    let error = process(&mut g, &module).unwrap_err();
    assert_eq!(crate::codegen::Error::Tsx, error);
    assert_eq!(
        "`Hidden` is not exported by `./Row`",
        diag.expect("diagnostic").msg
    );
}

#[test]
fn zant_api_names_pass_the_import_validation() {
    let mut diag = None;
    let mut g = Generator::new(&mut diag);
    assert_eq!(
        Some("clipboard".to_owned()),
        external_name(&mut g, "clipboard").expect("hook")
    );
    assert_eq!(None, external_name(&mut g, "Todo").expect("hook"));
}
