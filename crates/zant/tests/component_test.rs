//! Port of `tests/component_test.zig` — input/caret internals and paint-only
//! text updates. The gallery-app case needs `zant::app::App` (WP-2.9) and is
//! listed as not ported yet in the WP-2.10 report.

use zant::components::input;
use zant::core::geometry::Size;
use zant::core::node::{InputData, NodeData, NodeId};
use zant::core::tree::Tree;
use zant::core::tree::input_ops::Move;
use zant::renderer::text;
use zant::theme::tokens::ant_light;

fn input_data(tree: &Tree, id: NodeId) -> &InputData {
    match &tree.get(id).data {
        NodeData::Input(data) => data,
        _ => panic!("expected an input node"),
    }
}

fn input_data_mut(tree: &mut Tree, id: NodeId) -> &mut InputData {
    match &mut tree.get_mut(id).data {
        NodeData::Input(data) => data,
        _ => panic!("expected an input node"),
    }
}

#[test]
fn paint_only_text_update_does_not_mark_layout_dirty() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    let label = tree.add_text(
        Some(root),
        "before",
        text::Style {
            wrap: false,
            ..Default::default()
        },
    );
    tree.layout(Size {
        width: 200.0,
        height: 100.0,
    });
    tree.clear_paint();
    tree.set_text_no_layout(label, "after");
    assert!(!tree.needs_layout());
    assert!(tree.needs_paint());
}

#[test]
fn caret_keeps_the_text_height_with_a_tall_line_height() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    let id = tree.add_input(Some(root), "**写作应该是轻盈的。**\nsecond line", "", None);
    {
        let node = tree.get_mut(id);
        node.style.width = Some(400.0);
        node.style.height = Some(200.0);
        let data = match &mut node.data {
            NodeData::Input(data) => data,
            _ => panic!("expected an input node"),
        };
        data.multiline = true;
        data.font_size = 16.0;
        data.line_height = 2.0;
        data.available_width = 380.0;
        data.invalidate_layout();
        data.cursor = 0;
        data.anchor = 0;
    }
    tree.layout(Size {
        width: 500.0,
        height: 300.0,
    });
    let bounds = tree.get(id).bounds;
    let caret = input::caret_rect(input_data_mut(&mut tree, id), bounds).expect("caret");
    assert!(caret.height <= 16.0 * 1.3 + 0.01);
    assert!(caret.height >= 16.0);
    // still inside the first line box (8px top inset, 32px line)
    assert!(caret.y >= bounds.y + 8.0 - 0.5);
    assert!(caret.y + caret.height <= bounds.y + 8.0 + 32.0 + 0.5);
}

fn multiline_input(tree: &mut Tree, value: &str, height: f32) -> NodeId {
    let root = tree.root;
    let id = tree.add_input(Some(root), value, "", None);
    {
        let node = tree.get_mut(id);
        node.style.width = Some(300.0);
        node.style.height = Some(height);
        let data = match &mut node.data {
            NodeData::Input(data) => data,
            _ => panic!("expected an input node"),
        };
        data.multiline = true;
        data.font_size = 16.0;
        data.available_width = 278.0;
        data.invalidate_layout();
    }
    tree.layout(Size {
        width: 400.0,
        height: 400.0,
    });
    id
}

#[test]
fn multiline_caret_moves_between_lines_and_to_line_edges() {
    let mut tree = Tree::init(ant_light());
    let id = multiline_input(&mut tree, "first line\nsecond line\nthird line", 200.0);
    {
        let data = input_data_mut(&mut tree, id);
        // caret in the middle of "second line"
        data.cursor = 11 + 3;
        data.anchor = data.cursor;
    }
    tree.input_move(id, Move::Up, false);
    assert!(input_data(&tree, id).cursor < 11);
    tree.input_move(id, Move::Down, false);
    {
        let data = input_data(&tree, id);
        assert!(data.cursor >= 11 && data.cursor <= 22);
        assert!((data.cursor as i64 - 14).abs() <= 1);
    }
    tree.input_move(id, Move::LineEnd, false);
    assert_eq!(input_data(&tree, id).cursor, 22);
    tree.input_move(id, Move::LineStart, false);
    assert_eq!(input_data(&tree, id).cursor, 11);
    tree.input_move(id, Move::DocEnd, true);
    {
        let data = input_data(&tree, id);
        assert_eq!(data.cursor, data.text.len());
    }
    assert_eq!(input_data(&tree, id).anchor, 11);
}

#[test]
fn caret_moves_reveal_the_caret_plain_scrolling_does_not_snap_back() {
    let mut tree = Tree::init(ant_light());
    let mut long = String::with_capacity(400);
    for i in 0..400 {
        long.push(if i % 10 == 9 { '\n' } else { 'a' });
    }
    let id = multiline_input(&mut tree, &long, 80.0);
    tree.input_move(id, Move::DocEnd, false);
    {
        let data = input_data_mut(&mut tree, id);
        let width = data.available_width;
        input::ensure_caret_visible(data, width, 64.0);
        assert!(data.scroll_y > 0.0);
        // the user scrolls back to the top: no caret move, so it must stay there
        data.scroll_y = 0.0;
        input::ensure_caret_visible(data, width, 64.0);
        assert_eq!(data.scroll_y, 0.0);
    }
}
