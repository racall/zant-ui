#[test]
fn generated_hot_hooks_cover_root_and_child_state() {
    let source = r#"
        import { useState, useRef } from "react";
        import { View, Text } from "zant";
        interface Record { name: string; active: boolean }
        type Mode = "ready" | "busy";
        function Child() {
            const [open, setOpen] = useState(false);
            return <Text>{open ? "yes" : "no"}</Text>;
        }
        export default function App() {
            const [count, setCount] = useState(1);
            const [title, setTitle] = useState("hello");
            const ref = useRef(3);
            return <View><Child /><Text>{count}{title}</Text></View>;
        }
    "#;
    let generated = zant_tsx::compile::compile(source, "hot.tsx").unwrap();
    for text in [
        "fn hot_save(&self)",
        "fn hot_load(&mut self",
        "self.count",
        "self.title",
        "ui::local_state::local_state_hot::<",
        "impl tsx::hot::Hot for Child",
        "impl tsx::hot::Hot for Record",
        "impl tsx::hot::Hot for Mode",
    ] {
        assert!(generated.contains(text), "missing {text}: {generated}");
    }
    assert!(generated.contains("zant::hot_put!(snapshot, \"count\", self.count)"));
    assert!(generated.contains("zant::hot_get!(restore, \"title\", self.title)"));
    assert!(generated.contains("zant::hot_put!(snapshot, \"ref_\", self.ref_)"));
    assert!(generated.contains("if !zant::hot_get!(restore, \"name\", instance.name)"));
}
