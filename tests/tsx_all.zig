//! TSX compiler output tests (`zig build test-tsx`).
test {
    _ = @import("tsx_test.zig");
    _ = @import("tsx_golden_test.zig");
}
