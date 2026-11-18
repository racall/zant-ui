//! Unit tests.

use super::*;
use crate::parser::Parser;

fn parse_module(src: &str) -> ast::Module {
    let mut parser = Parser::init(src);
    parser.parse_module().expect("test source parses")
}

#[test]
fn usestate_list_and_stateful_children_make_a_component_stateful() {
    let src = "import { useState as useS } from \"zant\";\n\
             function Plain() { return <Text>hi</Text>; }\n\
             function Counter() { const [n, setN] = useS(0); return <Button onClick={() => setN(n + 1)}>{n}</Button>; }\n\
             function Wrapper() { return <View>{true && <Counter />}</View>; }\n\
             function Feed() { return <List data={[1]} renderItem={(x) => <Text>{x}</Text>} />; }\n\
             export default function App() { return <View><Plain /></View>; }\n";
    let module = parse_module(src);
    let expected = [false, true, true, true, false];
    for (top, want) in module.functions.iter().zip(expected) {
        assert_eq!(
            want,
            stateful(&[], &module, &top.func),
            "{}",
            String::from_utf8_lossy(&top.func.name)
        );
    }
}

#[test]
fn every_kind_of_bound_handler_needs_the_state_slot() {
    // a handler compiles to a method reached through `_state`: a component
    // without hooks still needs the slot, or its events are dropped
    let src = "import { grant } from \"./actions\";\n\
             import { relaunch } from \"native/capture\";\n\
             import type { Mode } from \"./actions\";\n\
             import { useTray, useTheme } from \"zant\";\n\
             function save(): void {}\n\
             const reset = () => {};\n\
             function Imported() { return <Button onClick={grant}>Grant</Button>; }\n\
             function Native() { return <Button onClick={relaunch}>Reopen</Button>; }\n\
             function Module() { return <Switch onChange={save} />; }\n\
             function ModuleConst() { return <Button onClick={reset}>Reset</Button>; }\n\
             function LocalArrow() { const go = () => save(); return <Button onClick={go}>Go</Button>; }\n\
             function Member() { return <Button onClick={store.save}>Save</Button>; }\n\
             function ChildCallback() { return <Row2 commit={save} />; }\n\
             function Tray() { useTray({ icon: \"\", items: [] }); return <Text>t</Text>; }\n\
             function Forwarded({ onSave }: P) { return <Button onClick={onSave}>Save</Button>; }\n\
             function ForwardedProps(props: P) { return <Button onClick={props.onSave}>Save</Button>; }\n\
             function Visual() { const theme = useTheme(); return <Text color={theme.colorText}>{label}</Text>; }\n\
             function Conditional({ busy }: P) { return <Button onClick={busy ? undefined : save}>Save</Button>; }\n\
             function SpreadLiteral() { return <Button {...{ onClick: save }}>Save</Button>; }\n\
             function SpreadLook() { return <Button {...{ width: 120 }}>Save</Button>; }\n";
    let module = parse_module(src);
    let expected = [
        ("save", false),
        ("reset", false),
        ("Imported", true),
        ("Native", true),
        ("Module", true),
        ("ModuleConst", true),
        ("LocalArrow", true),
        ("Member", true),
        ("ChildCallback", true),
        ("Tray", true),
        ("Forwarded", false),
        ("ForwardedProps", false),
        ("Visual", false),
        ("Conditional", true),
        ("SpreadLiteral", true),
        ("SpreadLook", false),
    ];
    for (top, (name, want)) in module.functions.iter().zip(expected) {
        assert_eq!(name.as_bytes(), top.func.name.as_slice());
        assert_eq!(want, stateful(&[], &module, &top.func), "{name}");
    }
}

#[test]
fn widgets_that_take_an_instance_id_are_stateful() {
    assert!(needs_id(b"Markdown"));
    assert!(!needs_id(b"View"));
}
