//! A todo app written by hand in Zig, the way Zant apps are usually
//! written: the baseline `zant-tsx-bench` compares the TSX todo app
//! (`todos.tsx`) against. Same UI, same keys.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;

const Todo = struct {
    id: u32,
    title: []const u8,
    done: bool,
};

const Filter = enum { all, active, done };

pub const Todos = struct {
    gpa: std.mem.Allocator = std.heap.smp_allocator,
    todos: std.ArrayList(Todo) = .empty,
    next_id: u32 = 1,
    text: std.ArrayList(u8) = .empty,
    filter: Filter = .all,

    pub fn deinit(self: *Todos) void {
        for (self.todos.items) |t| self.gpa.free(t.title);
        self.todos.deinit(self.gpa);
        self.text.deinit(self.gpa);
    }

    fn onText(self: *Todos, value: []const u8) void {
        self.text.clearRetainingCapacity();
        self.text.appendSlice(self.gpa, value) catch {};
    }

    fn add(self: *Todos) void {
        const title = std.mem.trim(u8, self.text.items, " ");
        if (title.len == 0) return;
        const copy = self.gpa.dupe(u8, title) catch return;
        self.todos.append(self.gpa, .{ .id = self.next_id, .title = copy, .done = false }) catch return;
        self.next_id += 1;
        self.text.clearRetainingCapacity();
    }

    fn indexOf(self: *Todos, id: u32) ?usize {
        for (self.todos.items, 0..) |t, i| if (t.id == id) return i;
        return null;
    }

    const RowAction = struct { self: *Todos, id: u32 };

    fn toggle(cap: *RowAction, _: bool) void {
        const i = cap.self.indexOf(cap.id) orelse return;
        cap.self.todos.items[i].done = !cap.self.todos.items[i].done;
    }

    fn remove(cap: *RowAction) void {
        const i = cap.self.indexOf(cap.id) orelse return;
        cap.self.gpa.free(cap.self.todos.items[i].title);
        _ = cap.self.todos.orderedRemove(i);
    }

    fn clearDone(self: *Todos) void {
        var i: usize = 0;
        while (i < self.todos.items.len) {
            if (self.todos.items[i].done) {
                self.gpa.free(self.todos.items[i].title);
                _ = self.todos.orderedRemove(i);
            } else i += 1;
        }
    }

    fn setFilter(self: *Todos, index: usize) void {
        self.filter = @fromBackingInt(@intCast(index));
    }

    fn row(self: *Todos, t: Todo) ui.Node {
        const a = ui.frameAllocator();
        const cap = a.create(RowAction) catch unreachable;
        cap.* = .{ .self = self, .id = t.id };
        const key = std.fmt.allocPrint(a, "todo-{d}", .{t.id}) catch "";
        return ui.row(.{
            .key = key,
            .gap = 8,
            .align_items = .center,
            .children = a.dupe(ui.Node, &.{
                ui.checkbox(.{
                    .key = std.fmt.allocPrint(a, "check-{d}", .{t.id}) catch "",
                    .label = t.title,
                    .checked = t.done,
                    .on_change = ui.boolAction(cap, toggle),
                }),
                ui.row(.{ .grow = 1 }),
                ui.iconButton(.{
                    .key = std.fmt.allocPrint(a, "remove-{d}", .{t.id}) catch "",
                    .icon = .trash,
                    .on_click = ui.action(cap, remove),
                }),
            }) catch unreachable,
        });
    }

    pub fn render(self: *Todos) ui.Node {
        const a = ui.frameAllocator();
        var left: usize = 0;
        for (self.todos.items) |t| {
            if (!t.done) left += 1;
        }
        var rows: std.ArrayList(ui.Node) = .empty;
        for (self.todos.items) |t| {
            const show = switch (self.filter) {
                .all => true,
                .active => !t.done,
                .done => t.done,
            };
            if (show) rows.append(a, self.row(t)) catch {};
        }
        return ui.view(.{
            .padding = ui.inset(24),
            .gap = 12,
            .children = a.dupe(ui.Node, &.{
                ui.heading("Todos", .{ .level = 3 }),
                ui.row(.{
                    .gap = 8,
                    .children = a.dupe(ui.Node, &.{
                        ui.view(.{ .grow = 1, .shrink = 1, .basis = 0, .children = a.dupe(ui.Node, &.{
                            ui.input(.{
                                .key = "new",
                                .value = self.text.items,
                                .placeholder = "What needs to be done?",
                                .on_change = ui.textAction(self, onText),
                                .on_submit = ui.action(self, add),
                            }),
                        }) catch unreachable }),
                        ui.button(.{ .key = "add", .text = "Add", .variant = .primary, .on_click = ui.action(self, add) }),
                    }) catch unreachable,
                }),
                ui.segmented(.{
                    .key = "filter",
                    .options = &.{ .{ .label = "All" }, .{ .label = "Active" }, .{ .label = "Done" } },
                    .selected = @backingInt(self.filter),
                    .on_change = ui.indexAction(self, setFilter),
                }),
                if (rows.items.len == 0)
                    ui.text(if (self.todos.items.len == 0) "Nothing to do yet." else "Nothing here.", .{ .key = "empty" })
                else
                    ui.view(.{ .key = "rows", .gap = 4, .children = rows.items }),
                ui.row(.{
                    .gap = 12,
                    .align_items = .center,
                    .children = a.dupe(ui.Node, &.{
                        ui.textFmt("{d} {s} left", .{ left, if (left == 1) "item" else "items" }, .{ .key = "left" }),
                        ui.button(.{ .key = "clear", .text = "Clear completed", .on_click = ui.action(self, clearDone) }),
                    }) catch unreachable,
                }),
            }) catch unreachable,
        });
    }
};
