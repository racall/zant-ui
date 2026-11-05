//! Callback contexts for interactive pieces (copy, download, links).
//!
//! Contexts are allocated from the render arena: the frame arena for plain
//! renders, or the block cache entry's arena for memoised blocks (so they
//! outlive frames while the block's UI is reused).
const std = @import("std");
const ui = @import("../../ui.zig");
const actions = @import("../../core/actions.zig");
const platform = @import("../../platform/platform.zig");
const dialogs = @import("../../platform/macos/dialog.zig");

pub const Copy = struct {
    text: []const u8,

    fn run(self: *Copy) void {
        _ = platform.setClipboard(self.text);
    }

    pub fn action(arena: std.mem.Allocator, text: []const u8) !actions.Action {
        const ctx = try arena.create(Copy);
        ctx.* = .{ .text = try arena.dupe(u8, text) };
        return ui.action(ctx, run);
    }
};

/// Save panel + write. With two formats, the chosen extension decides.
pub const Download = struct {
    filename: []const u8,
    extensions: []const u8,
    text: []const u8,
    /// Optional alternative content for a second extension (table: md).
    alt_extension: []const u8 = "",
    alt_text: []const u8 = "",

    fn run(self: *Download) void {
        const path = dialogs.saveFileAs(std.heap.c_allocator, self.filename, self.extensions) catch return;
        const p = path orelse return;
        defer std.heap.c_allocator.free(p);
        const data = if (self.alt_extension.len > 0 and std.mem.endsWith(u8, p, self.alt_extension)) self.alt_text else self.text;
        dialogs.writeFile(p, data) catch {};
    }

    pub fn action(arena: std.mem.Allocator, value: Download) !actions.Action {
        const ctx = try arena.create(Download);
        ctx.* = .{
            .filename = try arena.dupe(u8, value.filename),
            .extensions = try arena.dupe(u8, value.extensions),
            .text = try arena.dupe(u8, value.text),
            .alt_extension = try arena.dupe(u8, value.alt_extension),
            .alt_text = try arena.dupe(u8, value.alt_text),
        };
        return ui.action(ctx, run);
    }
};

/// Rich-text link activation: forwards to the app (link safety) or opens
/// the URL directly.
pub const Link = struct {
    forward: ?actions.TextAction,

    fn run(self: *Link, url: []const u8) void {
        if (self.forward) |f| return f.func(f.ctx, url);
        _ = dialogs.openUrl(url);
    }

    pub fn action(arena: std.mem.Allocator, forward: ?actions.TextAction) !actions.TextAction {
        const ctx = try arena.create(Link);
        ctx.* = .{ .forward = forward };
        return ui.textAction(ctx, run);
    }
};
