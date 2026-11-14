//! Cheap-clone immutable strings (`zant::Str`), see docs/rust-port/api.md.
//!
//! Option structs and declaration nodes copy strings constantly (every frame
//! builds a fresh tree, `styled()` clones elements); a `Str` is either a
//! `&'static str` (literals and defaults, no allocation) or an `Rc<str>` shared
//! by every clone.

use std::ops::Deref;
use std::rc::Rc;

#[derive(Clone)]
pub enum Str {
    /// A literal / default string: no allocation, no reference counting.
    Static(&'static str),
    /// A shared owned string.
    Shared(Rc<str>),
}

impl Str {
    /// Copies a runtime `&str` into a shared `Str` (Zig: arena copy).
    pub fn copy(value: &str) -> Str {
        Str::Shared(Rc::from(value))
    }

    pub fn as_str(&self) -> &str {
        match self {
            Str::Static(value) => value,
            Str::Shared(value) => value,
        }
    }
}

impl std::borrow::Borrow<str> for Str {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Deref for Str {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl Default for Str {
    fn default() -> Self {
        Str::Static("")
    }
}

impl From<&'static str> for Str {
    fn from(value: &'static str) -> Self {
        Str::Static(value)
    }
}

impl From<String> for Str {
    fn from(value: String) -> Self {
        Str::Shared(Rc::from(value))
    }
}

impl From<&String> for Str {
    fn from(value: &String) -> Self {
        Str::copy(value)
    }
}

impl PartialEq for Str {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for Str {}

impl PartialEq<str> for Str {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Str {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl std::hash::Hash for Str {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl std::fmt::Debug for Str {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_str(), formatter)
    }
}

impl std::fmt::Display for Str {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_and_shared_strings_compare_by_content() {
        let a = Str::from("save");
        let b = Str::from("save".to_owned());
        assert_eq!(a, b);
        assert_eq!(a, "save");
        assert_eq!(a.as_str(), "save");
        assert_eq!(a.len(), 4);
        assert_eq!(&*a, "save");
        assert_ne!(a, Str::from("cancel"));
    }

    #[test]
    fn copy_and_default() {
        let owned = String::from("runtime");
        let copied = Str::copy(&owned);
        assert_eq!(copied, "runtime");
        drop(owned);
        assert_eq!(copied, "runtime");
        assert_eq!(Str::default(), "");
        assert_eq!(format!("{copied}!"), "runtime!");
    }

    #[test]
    fn cloning_a_static_string_does_not_allocate() {
        let value = Str::Static("static");
        let clone = value.clone();
        assert!(matches!(clone, Str::Static("static")));
        assert!(matches!(Str::from("x"), Str::Static("x")));
    }
}
