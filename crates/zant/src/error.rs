//! The framework error type (`zant::Error`).
//!
//! A `Result` is only used where a failure is real (I/O, parsing, SQLite);
//! out-of-memory errors disappear (allocation failure aborts). Modules with a
//! localised failure kind (e.g. `math::parser::ParseError`,
//! `db::sqlite::DbError`) keep their own type.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid UTF-8")]
    Utf8,
    #[error("{0}")]
    Message(String),
}

/// `Result` shorthand used across the framework.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_errors_convert_and_display() {
        let error: Error = std::io::Error::new(std::io::ErrorKind::NotFound, "missing").into();
        assert!(matches!(error, Error::Io(_)));
        assert!(error.to_string().starts_with("I/O error:"));
        let message = Error::Message("boom".to_owned());
        assert_eq!(message.to_string(), "boom");
        assert_eq!(Error::Utf8.to_string(), "invalid UTF-8");
    }
}
