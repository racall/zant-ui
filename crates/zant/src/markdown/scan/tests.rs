//! Unit tests.

use super::*;

#[test]
fn list_markers() {
    let m = list_marker("  12) item").expect("ordered marker");
    assert!(m.ordered);
    assert_eq!(m.number, 12);
    assert_eq!(m.content_indent, 6);
    assert_eq!(m.rest, "item");
    assert!(list_marker("-not").is_none());
    assert_eq!(list_marker("* ").unwrap().rest.len(), 0);
}

#[test]
fn fences_and_headings() {
    let f = fence_open("~~~~ zig title=a").expect("fence");
    assert_eq!(f.len, 4);
    assert!(is_fence_close("~~~~~", &f));
    assert!(!is_fence_close("~~~", &f));
    assert_eq!(atx_heading("## Title ##").unwrap().text, "Title");
    assert!(atx_heading("#Title").is_none());
    assert_eq!(atx_heading("# C#").unwrap().text, "C#");
}
