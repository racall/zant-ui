//! TSX compiler output tests (`zig build test-tsx`).
test {
    _ = @import("tsx_test.zig");
    _ = @import("tsx_todos_test.zig");
    _ = @import("tsx_children_test.zig");
    _ = @import("tsx_list_test.zig");
    _ = @import("tsx_project_test.zig");
    _ = @import("tsx_local_state_test.zig");
    _ = @import("tsx_hooks_test.zig");
    _ = @import("tsx_builtins_test.zig");
    _ = @import("tsx_loops_test.zig");
    _ = @import("tsx_store_test.zig");
    _ = @import("tsx_nesting_test.zig");
    _ = @import("tsx_functions_test.zig");
    _ = @import("tsx_narrowing_test.zig");
    _ = @import("tsx_defaults_test.zig");
    _ = @import("tsx_timers_test.zig");
    _ = @import("tsx_crossfile_test.zig");
    _ = @import("tsx_list_scroll_test.zig");
    _ = @import("tsx_named_handlers_test.zig");
    _ = @import("tsx_zig_types_test.zig");
    _ = @import("tsx_spread_test.zig");
    _ = @import("tsx_app_test.zig");
    _ = @import("tsx_app_hooks_test.zig");
    _ = @import("tsx_wake_test.zig");
    _ = @import("tsx_media_test.zig");
    _ = @import("tsx_strings_test.zig");
    _ = @import("tsx_golden_test.zig");
    _ = @import("tsx_widgets_test.zig");
}
