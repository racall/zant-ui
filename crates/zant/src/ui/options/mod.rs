//! Option structs of the `ui::*` builders.
//!
//! Every struct derives `Clone` + `zant_macros::TsxProps` (the TSX compiler
//! reflects on them). Many defaults are non-zero, so most `Default` impls are
//! hand-written.
//!
//! The option structs of every builder are declared here; the builder bodies
//! live in `ui::builders` and `components/`.
#![forbid(unsafe_code)]

use std::rc::Rc;

use crate::Str;
use crate::core::geometry::Edges;
use crate::core::node::{Icon, ImageFit, RichSpan};
use crate::core::vector::{Drawing, DrawingRef};
use crate::flex;
use crate::image::Image;
use crate::motion::motion;
use crate::platform::platform::Cursor;
use crate::renderer::text;

use super::actions::{Action, BoolAction, IndexAction, KeyAction, PointerAction, TextAction};
use super::element::Node;

mod controls;
mod display;
mod layout;
mod media;
#[cfg(test)]
mod tests;
mod typography;

pub use self::controls::*;
pub use self::display::*;
pub use self::layout::*;
pub use self::media::*;
pub use self::typography::*;

/// Every `#[derive(TsxProps)]` option struct of this module, in declaration
/// order, for `zant-tsx-tags`.
///
/// Keep it in sync when adding an option struct: the generator reports a
/// builder whose options type is missing here. `ui::Styled`'s `Style` lives in
/// `ui/styled.rs` and is not a builder option, so it is not registered.
pub static TSX_REGISTRY: &[&tsx_props::TsxStruct] = &[
    &<BoxOptions as tsx_props::TsxProps>::TSX,
    &<PointerAreaOptions as tsx_props::TsxProps>::TSX,
    &<ClickableOptions as tsx_props::TsxProps>::TSX,
    &<SpaceOptions as tsx_props::TsxProps>::TSX,
    &<TextOptions as tsx_props::TsxProps>::TSX,
    &<HeadingOptions as tsx_props::TsxProps>::TSX,
    &<ParagraphOptions as tsx_props::TsxProps>::TSX,
    &<ButtonOptions as tsx_props::TsxProps>::TSX,
    &<LinkOptions as tsx_props::TsxProps>::TSX,
    &<InputOptions as tsx_props::TsxProps>::TSX,
    &<ToggleOptions as tsx_props::TsxProps>::TSX,
    &<RadioOption as tsx_props::TsxProps>::TSX,
    &<RadioGroupOptions as tsx_props::TsxProps>::TSX,
    &<TabItem as tsx_props::TsxProps>::TSX,
    &<TabsOptions as tsx_props::TsxProps>::TSX,
    &<TooltipOptions as tsx_props::TsxProps>::TSX,
    &<ImageOptions as tsx_props::TsxProps>::TSX,
    &<SelectOptions as tsx_props::TsxProps>::TSX,
    &<IconOptions as tsx_props::TsxProps>::TSX,
    &<IconButtonOptions as tsx_props::TsxProps>::TSX,
    &<DismissLayerOptions as tsx_props::TsxProps>::TSX,
    &<VectorOptions as tsx_props::TsxProps>::TSX,
    &<MemoOptions as tsx_props::TsxProps>::TSX,
    &<RichTextOptions as tsx_props::TsxProps>::TSX,
    &<PdfOptions as tsx_props::TsxProps>::TSX,
    &<DividerOptions as tsx_props::TsxProps>::TSX,
    &<TagOptions as tsx_props::TsxProps>::TSX,
    &<CardOptions as tsx_props::TsxProps>::TSX,
    &<AlertOptions as tsx_props::TsxProps>::TSX,
    &<AvatarOptions as tsx_props::TsxProps>::TSX,
    &<ProgressOptions as tsx_props::TsxProps>::TSX,
];
