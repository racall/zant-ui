//! Virtual list test modules and shared assertions.

mod layout;
mod runtime;
mod scroll;

fn approx(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} !~= {expected} (tolerance {tolerance})"
    );
}
