//! `local_bench.tsx` written by hand in Zig with `ui.localState`: the
//! baseline for the child component state benchmark.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;

const Counter = struct {
    id: u32,
    state: *u32,

    fn inc(self: *Counter) void {
        self.state.* += 1;
    }

    fn build(id: u32, odd: bool) ui.Node {
        const a = ui.frameAllocator();
        const theme = ui.currentTheme();
        const self = a.create(Counter) catch unreachable;
        self.* = .{ .id = id, .state = ui.localState(u32, ui.childId(0, id), 0) };
        return ui.row(.{
            .key = std.fmt.allocPrint(a, "{d}", .{id}) catch "",
            .gap = 8,
            .align_items = .center,
            .height = 36,
            .background = if (odd) theme.color_fill_quaternary else theme.color_bg_container,
            .children = a.dupe(ui.Node, &.{
                ui.textFmt("Row {d}: {d}", .{ id, self.state.* }, .{}),
                ui.button(.{
                    .key = std.fmt.allocPrint(a, "inc-{d}", .{id}) catch "",
                    .text = "+1",
                    .on_click = ui.action(self, inc),
                }),
            }) catch unreachable,
        });
    }
};

pub const LocalBench = struct {
    ids: std.ArrayList(u32) = .empty,
    next: u32 = 1,

    fn add(self: *LocalBench) void {
        self.ids.append(std.heap.smp_allocator, self.next) catch unreachable;
        self.next += 1;
    }

    pub fn deinit(self: *LocalBench) void {
        self.ids.deinit(std.heap.smp_allocator);
    }

    pub fn render(self: *LocalBench) ui.Node {
        const a = ui.frameAllocator();
        const children = a.alloc(ui.Node, self.ids.items.len + 1) catch unreachable;
        children[0] = ui.button(.{ .key = "add", .text = "Add row", .on_click = ui.action(self, add) });
        for (self.ids.items, children[1..]) |id, *child| child.* = Counter.build(id, id % 2 == 1);
        return ui.view(.{ .padding = ui.inset(12), .gap = 4, .children = children });
    }
};
