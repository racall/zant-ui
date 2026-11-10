//! A thumbnail cache stand-in for tests/tsx/media/App.tsx.
const zant = @import("zant");
pub fn thumbnail(id: u64) ?zant.image.Image {
    _ = id;
    return null;
}
