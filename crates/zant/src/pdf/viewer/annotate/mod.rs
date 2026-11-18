//! Annotations of the PDF viewer.
//!
//! The lists, undo / redo history, hit testing and the annotated export live
//! in `pdf::annotations`; this module is the viewer side: turning a
//! text selection or an ink drag into an annotation, selecting / editing /
//! removing one, marshalling the list for `save_annotated`, and drawing the
//! annotation layers (markup boxes, squiggles, ink strokes, note icons, the
//! selection outline) and the annotation tool switch of the toolbar.

use std::rc::Rc;

use crate::Str;
use crate::components::segmented::{SegmentedOption, SegmentedOptions};
use crate::core::node::Icon;
use crate::core::vector::{Command, Drawing, Path};
use crate::flex;
use crate::pdf::annotations::{self, Annotation, Kind, Point};
use crate::pdf::native;
use crate::pdf::text::Rect;
use crate::theme::tokens::Theme;
use crate::ui::actions::{Action, IndexAction};
use crate::ui::builders::{icon_button, row, space};
use crate::ui::element::Node;
use crate::ui::options::{BoxOptions, IconButtonOptions, IconOptions, SpaceOptions};
use crate::ui::{self, Style, styled};

use super::{Inner, SaveError, Tool, ViewOptions, Viewer};

mod drawing;
mod editing;
mod toolbar;

pub(crate) use self::drawing::*;
pub(crate) use self::toolbar::*;

// Named like the `annotations::colors` constants.
const BLUE: u32 = 0xff16_77ff;

fn tb_undo(viewer: &Viewer) {
    let _ = viewer.undo();
}

fn tb_redo(viewer: &Viewer) {
    let _ = viewer.redo();
}
