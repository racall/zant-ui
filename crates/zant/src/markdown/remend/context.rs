//! Shared state for one `remend` call and the single-entry scan memo of
//! `scan.ts` (`getScan`).
//!
//! The memo owns the scan (and its text) and compares content, which yields
//! exactly the same scans as keying on string identity.

use super::scan::Scan;

pub struct Context {
    cached: Option<Scan>,
}

impl Context {
    pub fn new() -> Context {
        Context { cached: None }
    }

    /// Memoized scan of `text`. Handlers pass each other's output along, and
    /// most return their input unchanged, so the cache usually survives a step.
    pub fn scan(&mut self, text: &str) -> &mut Scan {
        let hit = matches!(&self.cached, Some(scan) if scan.text == text);
        if !hit {
            self.cached = Some(Scan::new(text));
        }
        self.cached.as_mut().expect("scan cache was just filled")
    }

    pub fn is_inside_code_block(&mut self, text: &str, position: usize) -> bool {
        self.scan(text).is_code_at(position as isize)
    }

    pub fn is_within_complete_inline_code(&mut self, text: &str, position: usize) -> bool {
        self.scan(text).is_complete_span_at(position as isize)
    }

    pub fn concat(&self, parts: &[&str]) -> String {
        parts.concat()
    }
}

impl Default for Context {
    fn default() -> Self {
        Context::new()
    }
}
