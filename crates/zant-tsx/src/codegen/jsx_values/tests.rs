//! Unit tests.

use super::*;

#[test]
fn fields_merge_later_writes_and_keep_order() {
    let mut fields = Fields::default();
    fields.put("gap", "8.0f32".to_owned());
    fields.put("key", "\"a\".into()".to_owned());
    fields.put("gap", "4.0f32".to_owned());
    let emit = fields.emit("ui::BoxOptions");
    assert!(emit.starts_with("ui::BoxOptions { "));
    assert!(emit.contains("gap: 4.0f32, "), "{emit}");
    assert!(emit.contains("key: \"a\".into(), "), "{emit}");
    assert!(emit.ends_with("..Default::default() }"), "{emit}");
    assert!(!emit.contains("8.0f32"));
}

#[test]
fn content_first_follows_key() {
    let mut fields = Fields::default();
    fields.put("disabled", "true".to_owned());
    fields.put("key", "\"b\".into()".to_owned());
    fields.put("text", "\"Save\".into()".to_owned());
    fields.content_first("text");
    let emit = fields.emit("ui::ButtonOptions");
    let key = emit.find("key:").expect("key");
    let text = emit.find("text:").expect("text");
    let disabled = emit.find("disabled:").expect("disabled");
    assert!(key < text && text < disabled, "{emit}");
}

#[test]
fn equal_edges_become_one_inset() {
    let mut fields = Fields::default();
    let entry = fields.entry("padding");
    for side in 0..4 {
        entry.edges[side] = Some(Edge {
            code: "12.0f32".to_owned(),
            source: 7,
        });
    }
    assert!(
        fields
            .emit("ui::BoxOptions")
            .contains("padding: ui::inset(12.0f32)")
    );
}

#[test]
fn mixed_edges_emit_the_geometry_struct() {
    let mut fields = Fields::default();
    let entry = fields.entry("margin");
    entry.edges[0] = Some(Edge {
        code: "1.0f32".to_owned(),
        source: 1,
    });
    entry.edges[1] = Some(Edge {
        code: "2.0f32".to_owned(),
        source: 2,
    });
    let emit = fields.emit("ui::BoxOptions");
    assert!(
            emit.contains(
                "margin: zant::core::geometry::Edges { top: 1.0f32, right: 2.0f32, bottom: 0.0, left: 0.0 }"
            ),
            "{emit}"
        );
}

#[test]
fn nested_fields_group_under_the_parent_field() {
    let mut fields = Fields::default();
    fields.put(
        "on_click",
        "Some(ui::action(|s: &mut C| s._hit()))".to_owned(),
    );
    fields.group("box").put("gap", "6.0f32".to_owned());
    let emit = fields.emit("ui::ClickableOptions");
    assert!(
        emit.contains("box: ui::BoxOptions { gap: 6.0f32, ..Default::default() }"),
        "{emit}"
    );
    assert!(emit.contains("on_click: Some("), "{emit}");
}
