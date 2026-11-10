//! Runtime of nested TSX text (`<Text>a <Text weight="bold">b</Text> <Link>c</Link></Text>`):
//! the compiler flattens the spans into `ui.richText`, filling the styles a
//! span doesn't set from the theme like `ui.text` does.
const std = @import("std");
const ui = @import("ui.zig");
const node = @import("core/node.zig");
const text = @import("renderer/text.zig");
const actions = @import("core/actions.zig");

/// A span's style; null fields fall back to `ui.text`'s defaults.
pub const SpanStyle = struct {
    size: ?f32 = null,
    weight: ?text.Weight = null,
    color: ?u32 = null,
    family: ?text.Family = null,
    italic: bool = false,
    underline: bool = false,
    strike: bool = false,
    background: u32 = 0,
    /// Index into the text's link actions (`links`); null for plain text.
    link: ?usize = null,
};

const link_names = [_][]const u8{ "0", "1", "2", "3", "4", "5", "6", "7", "8", "9" };

/// One span of `value` with `style`.
pub fn span(value: []const u8, style: SpanStyle) node.RichSpan {
    const theme = ui.currentTheme();
    return .{
        .text = value,
        .font_size = style.size orelse 14,
        .weight = style.weight orelse .regular,
        .color = style.color orelse if (style.link != null) theme.color_primary else theme.color_text,
        .family = style.family orelse .sans,
        .italic = style.italic,
        .underline = style.underline,
        .strike = style.strike,
        .background = style.background,
        .link = if (style.link) |i| linkName(i) else "",
    };
}

fn linkName(i: usize) []const u8 {
    if (i < link_names.len) return link_names[i];
    return std.fmt.allocPrint(ui.frameAllocator(), "{d}", .{i}) catch @panic("Zant UI: out of memory");
}

/// The `on_link` of a text with inline `<Link>`s: a click on link span `i`
/// runs `targets[i]`.
pub fn links(targets: []const ?actions.Action) actions.TextAction {
    const Links = struct {
        targets: []const ?actions.Action,

        fn open(self: *@This(), name: []const u8) void {
            const i = std.fmt.parseInt(usize, name, 10) catch return;
            if (i >= self.targets.len) return;
            if (self.targets[i]) |target| target.func(target.ctx);
        }
    };
    const a = ui.frameAllocator();
    const self = a.create(Links) catch @panic("Zant UI: out of memory");
    self.* = .{ .targets = a.dupe(?actions.Action, targets) catch @panic("Zant UI: out of memory") };
    return ui.textAction(self, Links.open);
}

test "spans take theme defaults and links dispatch by index" {
    ui.beginFrame();
    defer ui.endFrame();
    const plain = span("a", .{});
    try std.testing.expectEqual(@as(f32, 14), plain.font_size);
    try std.testing.expectEqualStrings("", plain.link);
    const bold = span("b", .{ .weight = .bold, .size = 20, .link = 1 });
    try std.testing.expectEqual(text.Weight.bold, bold.weight);
    try std.testing.expectEqualStrings("1", bold.link);
    try std.testing.expectEqualStrings("12", linkName(12));

    const Probe = struct {
        var hits: u32 = 0;
        fn hit(_: *u8) void {
            hits += 1;
        }
    };
    var ctx: u8 = 0;
    const on_link = links(&.{ null, ui.action(&ctx, Probe.hit) });
    on_link.func(on_link.ctx, "1");
    on_link.func(on_link.ctx, "0");
    on_link.func(on_link.ctx, "7");
    try std.testing.expectEqual(@as(u32, 1), Probe.hits);
}
