//! Unit tests.

use super::*;

#[test]
fn box_options_have_the_expected_defaults() {
    let options = BoxOptions::default();
    assert_eq!(options.gap, 0.0);
    assert_eq!(options.wrap, flex::Wrap::Nowrap);
    assert_eq!(options.align_content, flex::AlignContent::Start);
    assert_eq!(options.align_self, flex::AlignSelf::Auto);
    assert_eq!(options.justify, flex::Justify::Start);
    assert!(options.justify_content.is_none());
    assert_eq!(options.align_items, flex::Align::Start);
    assert!(options.flex_direction.is_none());
    assert!(!options.absolute);
    assert!(!options.clip);
    assert!(!options.scroll);
    assert!(!options.block_pointer);
    assert_eq!(options.grow, 0.0);
    assert_eq!(options.shrink, 0.0, "a box does not shrink by default");
    assert_eq!(options.border_width, 0.0);
    assert_eq!(options.radius, 0.0);
    assert!(options.children.is_empty());
}

#[test]
fn text_and_button_options_have_the_expected_defaults() {
    let text = TextOptions::default();
    assert_eq!(text.size, 14.0);
    assert_eq!(text.weight, text::Weight::Regular);
    assert!(!text.wrap);
    assert_eq!(text.line_height, 0.0);
    assert_eq!(text.max_lines, 0);
    assert_eq!(text.family, text::Family::Sans);

    let button = ButtonOptions::default();
    assert_eq!(button.variant, crate::core::node::ButtonType::Default);
    assert_eq!(button.size, crate::core::node::ButtonSize::Middle);
    assert_eq!(button.text_align, text::Align::Center);
    assert_eq!(button.icon_size, 18.0);
    assert!(button.focusable);
    assert_eq!(button.text, "");

    let input = InputOptions::default();
    assert_eq!(input.height, 32.0);
    assert_eq!(input.font_size, 14.0);
    assert!(input.focus_ring);
}

#[test]
fn the_other_options_keep_their_non_zero_defaults() {
    assert_eq!(SpaceOptions::default().gap, 8.0);
    assert_eq!(HeadingOptions::default().level, 1);
    assert_eq!(HeadingOptions::default().weight, text::Weight::Semibold);
    assert_eq!(ParagraphOptions::default().line_height, 1.5);
    assert_eq!(ToggleOptions::default().font_size, 14.0);
    assert_eq!(
        RadioGroupOptions::default().direction,
        flex::Direction::Column
    );
    assert_eq!(RadioGroupOptions::default().gap, 8.0);
    assert_eq!(SelectOptions::default().height, 32.0);
    assert_eq!(ImageOptions::default().radius, 8.0);
    assert_eq!(RichTextOptions::default().line_height, 2.0);
    assert_eq!(
        PdfOptions::default().width,
        320.0,
        "PdfOptions width / height / toolbar defaults"
    );
    assert_eq!(PdfOptions::default().height, 420.0);
    assert!(PdfOptions::default().show_toolbar);
    assert_eq!(CardOptions::default().gap, 12.0);
    assert_eq!(CardOptions::default().padding, Edges::all(20.0));
    assert!(CardOptions::default().bordered);
    assert_eq!(AvatarOptions::default().text, "?");
    assert_eq!(AvatarOptions::default().size, 32.0);
    assert_eq!(ProgressOptions::default().width, 200.0);
    assert_eq!(ProgressOptions::default().height, 8.0);
    assert_eq!(AlertOptions::default().r#type, AlertType::Info);
    assert_eq!(
        DismissLayerOptions::default().key.as_deref(),
        Some("dismiss-layer")
    );
    assert_eq!(ClickableOptions::default().cursor, Cursor::PointingHand);
    assert!(PointerAreaOptions::default().clicks);
}
