//! Locale catalog discovery and located JSON translation entries.
//!
//! Catalogs accept flat or nested objects and preserve byte offsets for later
//! message parsing and diagnostics. MessageFormat interpretation is separate.

pub mod catalog;
pub mod json;

pub use catalog::{Catalog, CatalogError, Entry, canonical_tag, discover, parse};
