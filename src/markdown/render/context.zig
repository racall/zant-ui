//! Per-render state passed down the block tree.
const std = @import("std");
const actions = @import("../../core/actions.zig");
const text_mod = @import("../../renderer/text.zig");
const Options = @import("options.zig").Options;
const link_actions = @import("actions.zig");

pub const Ctx = struct {
    /// Allocator for callback contexts and inline embeds; must outlive the
    /// rendered UI (frame arena, or a cache entry's arena when memoised).
    arena: std.mem.Allocator,
    options: *const Options,
    /// Current text colour (quotes and callouts mute it).
    fg: u32,
    /// Current body size (footnotes shrink it).
    size: f32,
    /// Append the streaming caret to the last text run.
    caret: bool = false,
    link_action: ?actions.TextAction = null,
    /// Unique key prefix of the block being rendered.
    key: []const u8 = "md",

    pub fn init(arena: std.mem.Allocator, options: *const Options) !Ctx {
        return .{
            .arena = arena,
            .options = options,
            .fg = options.theme.foreground,
            .size = options.font_size,
            .link_action = try link_actions.Link.action(arena, options.on_link),
        };
    }

    pub fn with(self: Ctx, fg: u32, size: f32) Ctx {
        var c = self;
        c.fg = fg;
        c.size = size;
        return c;
    }

    pub fn subKey(self: Ctx, comptime fmt: []const u8, args: anytype) []const u8 {
        return std.fmt.allocPrint(self.arena, "{s}-" ++ fmt, .{self.key} ++ args) catch self.key;
    }

    pub fn family(self: Ctx) text_mod.Family {
        return self.options.theme.body_family;
    }
};
