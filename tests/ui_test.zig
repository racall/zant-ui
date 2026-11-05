const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const events = zant.events;

const State = struct {
    saved: u32 = 0,
    name_buf: [128]u8 = undefined,
    name_len: usize = 0,
    plan: usize = 0,
    runtime: ?*Runtime = null,

    fn name(self: *const State) []const u8 {
        return self.name_buf[0..self.name_len];
    }

    fn save(self: *State) void {
        self.saved += 1;
        if (self.runtime) |runtime| runtime.invalidate();
    }

    fn changePlan(self: *State, index: usize) void {
        self.plan = index;
    }

    fn changeName(self: *State, value: []const u8) void {
        const n = @min(value.len, self.name_buf.len);
        @memcpy(self.name_buf[0..n], value[0..n]);
        self.name_len = n;
    }

    fn render(context: *anyopaque) ui.Node {
        const self: *State = @ptrCast(@alignCast(context));
        return ui.column(.{
            .gap = 12,
            .padding = ui.inset(24),
            .children = &.{
                ui.heading("Settings", .{ .level = 1 }),
                ui.paragraph("Declarative Zant UI runtime", .{}),
                ui.input(.{
                    .value = self.name(),
                    .placeholder = "Project name",
                    .on_change = ui.textAction(self, changeName),
                }),
                ui.button(.{
                    .text = "Save Changes",
                    .variant = .primary,
                    .on_click = ui.action(self, save),
                }),
                ui.checkbox(.{ .label = "Notifications", .checked = true }),
                ui.@"switch"(.{ .label = "Beta updates", .checked = false }),
                ui.select(.{
                    .options = &.{ "Starter", "Pro", "Enterprise" },
                    .selected = self.plan,
                    .width = 220,
                    .on_change = ui.indexAction(self, changePlan),
                }),
            },
        });
    }
};

fn findKind(tree: *zant.tree.Tree, kind: zant.node.NodeKind) ?*zant.node.Node {
    for (tree.nodes.items) |node| {
        if (node.kind() == kind) return node;
    }
    return null;
}

fn click(runtime: *Runtime, node: *const zant.node.Node) void {
    const p = events.PointerEvent{
        .x = node.bounds.x + node.bounds.width * 0.5,
        .y = node.bounds.y + node.bounds.height * 0.5,
    };
    _ = runtime.dispatch(.{ .pointer_move = p });
    _ = runtime.dispatch(.{ .pointer_down = p });
    _ = runtime.dispatch(.{ .pointer_up = p });
}

test "declarative runtime renders, edits input and invokes actions" {
    var state = State{};
    const runtime = try Runtime.init(std.testing.allocator, &state, State.render, .{
        .title = "Test",
        .window = .{ .width = 520, .height = 420 },
    });
    defer runtime.deinit();
    state.runtime = runtime;

    var renderer = try Renderer.createRaster(520, 420);
    defer renderer.deinit();
    {
        const canvas = renderer.begin(1.0).?;
        runtime.renderFrame(canvas, 520, 420, 1.0);
        renderer.end();
    }
    try std.testing.expect(runtime.tree.nodes.items.len >= 6);

    const input_node = findKind(runtime.tree, .input).?;
    click(runtime, input_node);
    try std.testing.expectEqual(input_node.id, runtime.tree.focused.?);
    _ = runtime.dispatch(.{ .text_input = "Zant" });
    try std.testing.expectEqualStrings("Zant", state.name());

    {
        const canvas = renderer.begin(1.0).?;
        runtime.renderFrame(canvas, 520, 420, 1.0);
        renderer.end();
    }
    const input_after = findKind(runtime.tree, .input).?;
    try std.testing.expectEqualStrings("Zant", input_after.data.input.text.items);

    const button_node = findKind(runtime.tree, .button).?;
    click(runtime, button_node);
    try std.testing.expectEqual(@as(u32, 1), state.saved);

    const toggle_node = findKind(runtime.tree, .toggle).?;
    click(runtime, toggle_node);
    try std.testing.expect(toggle_node.data.toggle.checked == false);
    {
        const canvas = renderer.begin(1.0).?;
        runtime.renderFrame(canvas, 520, 420, 1.0);
        renderer.end();
    }

    const select_node = findKind(runtime.tree, .select).?;
    click(runtime, select_node);
    try std.testing.expect(select_node.data.select.open);
    const popup = zant.select.popupRect(select_node, runtime.viewport);
    const p = events.PointerEvent{
        .x = popup.x + 10,
        .y = popup.y + zant.select.popup_padding + zant.select.row_height * 2 + zant.select.row_height * 0.5,
    };
    _ = runtime.dispatch(.{ .pointer_move = p });
    _ = runtime.dispatch(.{ .pointer_down = p });
    _ = runtime.dispatch(.{ .pointer_up = p });
    try std.testing.expectEqual(@as(usize, 2), state.plan);
    {
        const canvas = renderer.begin(1.0).?;
        runtime.renderFrame(canvas, 520, 420, 1.0);
        renderer.end();
    }
    try std.testing.expectEqual(@as(usize, 2), findKind(runtime.tree, .select).?.data.select.selected);
    try std.testing.expect(runtime.tree.nodes.items.len >= 6);

    const png = "zig-out/test-ui-runtime.png";
    try std.testing.expect(renderer.writePng(png));
}

test "input keeps its intrinsic height in a short window" {
    var state = State{};
    const runtime = try Runtime.init(std.testing.allocator, &state, State.render, .{
        .title = "Small",
        .window = .{ .width = 520, .height = 200 },
    });
    defer runtime.deinit();

    var renderer = try Renderer.createRaster(520, 200);
    defer renderer.deinit();
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, 520, 200, 1.0);
    renderer.end();

    const input_node = findKind(runtime.tree, .input).?;
    try std.testing.expectApproxEqAbs(@as(f32, 32), input_node.bounds.height, 0.01);
}

test "password input masks display text" {
    const allocator = std.testing.allocator;
    var data = zant.node.InputData{
        .placeholder = try allocator.dupe(u8, "Password"),
        .password = true,
    };
    defer {
        data.deinit(allocator);
        allocator.free(data.placeholder);
    }
    try data.text.appendSlice(allocator, "secret");
    data.cursor = data.text.items.len;
    data.anchor = data.cursor;
    zant.input.rebuildDisplay(allocator, &data);
    try std.testing.expectEqualStrings("******", data.display_text.items);
}

test "styled overrides common visual properties" {
    ui.beginFrame();
    defer ui.endFrame();
    const node = ui.styled(ui.text("X", .{}), .{
        .background = 0xff112233,
        .border_color = 0xff445566,
        .border_width = 2,
        .radius = 5,
        .padding = ui.inset(3),
    });
    try std.testing.expectEqual(@as(?u32, 0xff112233), node.element.visual.background);
    try std.testing.expectEqual(@as(?u32, 0xff445566), node.element.visual.border_color);
    try std.testing.expectEqual(@as(f32, 2), node.element.visual.border_width);
    try std.testing.expectEqual(@as(f32, 5), node.element.visual.radius);
    try std.testing.expectEqual(@as(f32, 3), node.element.layout.padding.left);
}
