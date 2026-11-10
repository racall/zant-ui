//! TSX compiler output tests (`zig build test-tsx`).
test {
    _ = @import("tsx_test.zig");
    _ = @import("tsx_todos_test.zig");
    _ = @import("tsx_children_test.zig");
    _ = @import("tsx_list_test.zig");
    _ = @import("tsx_project_test.zig");
    _ = @import("tsx_local_state_test.zig");
    _ = @import("tsx_hooks_test.zig");
    _ = @import("tsx_store_test.zig");
    _ = @import("tsx_golden_test.zig");
}
