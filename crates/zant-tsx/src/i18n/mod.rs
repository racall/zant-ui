//! Locale catalog discovery and located JSON translation entries.
//!
//! Catalogs accept flat or nested objects and preserve byte offsets for later
//! message parsing and diagnostics. MessageFormat interpretation is separate.

pub mod catalog;
pub mod contract;
pub mod json;
pub mod message;
pub mod types;

pub use catalog::{Catalog, CatalogError, Entry, canonical_tag, discover, parse};
pub use contract::{Contract, Param, ParamKind, check_translation, contract};
pub use message::{Category, Message, MessageError, Part, parse as parse_message};
