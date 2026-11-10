//! Virtual list benchmark: 10 000 rows in TSX (`list_bench.tsx`) against the
//! hand-written Zig list (`list_manual.zig`): 300 wheel scrolls and 100 row
//! clicks.
const std = @import("std");
const zant = @import("zant");
const harness = @import("bench_harness.zig");
const Stats = harness.Stats;
const Harness = harness.Harness;
const TsxList = @import("list_bench_tsx").ListBench;
const TsxRow = @import("list_bench_tsx").Row;
const Manual = @import("list_manual.zig");

const rows = 10_000;

pub fn run(gpa: std.mem.Allocator, renderer: *zant.renderer.Renderer) !void {
    // the TSX list owns its rows through tsx.State; give both the same data
    const tsx_rows = try gpa.alloc(TsxRow, rows);
    defer gpa.free(tsx_rows);
    const manual_rows = try gpa.alloc(Manual.Row, rows);
    defer gpa.free(manual_rows);
    for (tsx_rows, manual_rows, 0..) |*t, *m, i| {
        t.* = .{ .id = @intCast(i + 1), .title = "Row title" };
        m.* = .{ .id = @intCast(i + 1), .title = "Row title" };
    }
    var tsx_app = TsxList{};
    tsx_app.rows.set(tsx_rows);
    try scenario(gpa, renderer, &tsx_app, "tsx");
    var manual_app = Manual.ListBench{ .rows = manual_rows };
    try scenario(gpa, renderer, &manual_app, "manual");
}

fn scenario(gpa: std.mem.Allocator, renderer: *zant.renderer.Renderer, app: anytype, label: []const u8) !void {
    var h = try Harness.init(gpa, renderer, app);
    defer h.deinit();
    var scroll: Stats = .{};
    defer scroll.samples.deinit(gpa);
    for (0..300) |i| {
        const dy: f32 = if (i < 200) -120 else 120;
        _ = h.rt.dispatch(.{ .scroll = .{ .x = 200, .y = 400, .dx = 0, .dy = dy, .modifiers = .{} } });
        scroll.add(gpa, h.frame());
    }
    var click: Stats = .{};
    defer click.samples.deinit(gpa);
    var key: [32]u8 = undefined;
    var clicked: usize = 0;
    var id: usize = 1;
    while (clicked < 100) : (id += 1) {
        // click the rows currently on screen (after scrolling back up)
        const k = try std.fmt.bufPrint(&key, "pick-{d}", .{id});
        if (h.rt.tree.boundsOfKey(k) == null) {
            if (id > rows) return error.NoVisibleRows;
            continue;
        }
        click.add(gpa, try h.click(k));
        clicked += 1;
        if (id > 15) id = 0;
    }
    var name: [32]u8 = undefined;
    scroll.report(try std.fmt.bufPrint(&name, "list-scroll-{s}", .{label}));
    click.report(try std.fmt.bufPrint(&name, "list-click-{s}", .{label}));
}
