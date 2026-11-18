//! Pointer dispatch.
//!
//! Hover / press state of buttons, toggles and inputs, the container hover look,
//! pointer-area capture (`ui::pointer_area`: down / drag / up / hover / leave),
//! cursor changes, multi-click counting, the click into an input (caret
//! placement), and the press that starts a text selection (see `selection`).
//!
//! While a select popup is open it owns the pointer: hover highlights the
//! option under the cursor and a click selects it (`components::select` only
//! provides the geometry).
//!
//! Toggle activation lives here as well: the stored `index_action` / `action`
//! callbacks need the runtime's `ActionCx`, so [`Runtime::toggle_set`] clones
//! them out of the node and calls them.
#![forbid(unsafe_code)]

use super::{Runtime, SelMode, action_cx};

mod area;
mod click;
mod hover;
mod text;

#[cfg(test)]
mod tests;
