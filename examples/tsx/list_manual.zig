//! The 10 000-row list of `list_bench.tsx` written by hand in Zig: the
//! baseline for `zant-tsx-bench list`.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;

pub const Row = struct {
    id: u32,
    title: []const u8,
};

pub const ListBench = struct {
    rows: []const Row = &.{},
    picked: u32 = 0,
    list: ui.ListState = .{ .allocator = std.heap.smp_allocator },

    const Pick = struct { self: *ListBench, id: u32 };

    fn pick(cap: *Pick) void {
        cap.self.picked = cap.id;
    }

    fn item(self: *ListBench, index: usize) ui.Node {
        const a = ui.frameAllocator();
        const theme = ui.currentTheme();
        const row = self.rows[index];
        const cap = a.create(Pick) catch unreachable;
        cap.* = .{ .self = self, .id = row.id };
        return ui.row(.{
            .key = std.fmt.allocPrint(a, "row-{d}", .{row.id}) catch "",
            .gap = 8,
            .align_items = .center,
            .height = 40,
            .background = if (index % 2 == 0) theme.color_fill_quaternary else theme.color_bg_container,
            .children = a.dupe(ui.Node, &.{
                ui.text(row.title, .{}),
                ui.button(.{
                    .key = std.fmt.allocPrint(a, "pick-{d}", .{row.id}) catch "",
                    .text = "Pick",
                    .on_click = ui.action(cap, pick),
                }),
            }) catch unreachable,
        });
    }

    fn id(self: *ListBench, index: usize) u64 {
        return self.rows[index].id;
    }

    pub fn render(self: *ListBench) ui.Node {
        const a = ui.frameAllocator();
        return ui.view(.{
            .grow = 1,
            .padding = ui.inset(12),
            .gap = 8,
            .children = a.dupe(ui.Node, &.{
                ui.textFmt("Picked {d}", .{self.picked}, .{ .key = "picked" }),
                ui.list(&self.list, .{
                    .key = "rows",
                    .count = self.rows.len,
                    .item_height = 40,
                    .item = ui.listItem(self, item),
                    .item_id = ui.listItemId(self, id),
                }),
            }) catch unreachable,
        });
    }
};
