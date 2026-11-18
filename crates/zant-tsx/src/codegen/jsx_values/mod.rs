//! Node and literal-list props, recursive item schemas, and required-field
//! checks, together with the ordered option-field merging and the `Edges`
//! lowering.
//!
//! Options are concrete structs: [`Fields::emit`] spells
//! `ui::XOptions { field: value, ..Default::default() }`. Children become the
//! `nodes!` macro, and optional fields are wrapped in `Some(..)` by the caller
//! (the tag table does not carry optionality; see `jsx.rs`).

use crate::ast;
use crate::tags::{self, Kind, Prop, Sides, Tag};

use super::expr;
use super::jsx;
use super::jsx_children::ChildList;
use super::model;
use super::scope;
use super::types::{self, Type};
use super::{Generator, Result, fail};

mod edges;
mod fields;
#[cfg(test)]
mod tests;
mod values;

pub use self::edges::*;
pub use self::fields::*;
pub use self::values::*;

/// `ui::inset` for the four equal sides, or a struct for mixed sides.
pub fn inset(code: &str) -> String {
    format!("ui::inset({code})")
}
