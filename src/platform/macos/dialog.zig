//! Native modal dialogs and small platform services (AppKit).
const std = @import("std");

extern fn zant_open_file_dialog() ?[*:0]u8;
extern fn zant_save_file_dialog(default_name: ?[*:0]const u8) ?[*:0]u8;
extern fn zant_save_file_dialog_ext(default_name: ?[*:0]const u8, extensions: ?[*:0]const u8) ?[*:0]u8;
extern fn zant_dialog_free(value: ?[*:0]u8) void;
extern fn zant_confirm_dialog(title: ?[*:0]const u8, message: ?[*:0]const u8, first: ?[*:0]const u8, second: ?[*:0]const u8, third: ?[*:0]const u8) c_int;
extern fn zant_open_url(url: [*:0]const u8) c_int;

/// Shows an open panel for Markdown/text files. Caller owns the path.
pub fn openFile(allocator: std.mem.Allocator) !?[]u8 {
    const raw = zant_open_file_dialog() orelse return null;
    defer zant_dialog_free(raw);
    return try allocator.dupe(u8, std.mem.span(raw));
}

/// Shows a save panel for Markdown/text files. Caller owns the path.
pub fn saveFile(allocator: std.mem.Allocator, default_name: []const u8) !?[]u8 {
    const name = try allocator.dupeSentinel(u8, default_name, 0);
    defer allocator.free(name);
    const raw = zant_save_file_dialog(name.ptr) orelse return null;
    defer zant_dialog_free(raw);
    return try allocator.dupe(u8, std.mem.span(raw));
}

/// Save panel restricted to `extensions` ("csv,md"); empty allows any type.
pub fn saveFileAs(allocator: std.mem.Allocator, default_name: []const u8, extensions: []const u8) !?[]u8 {
    const name = try allocator.dupeSentinel(u8, default_name, 0);
    defer allocator.free(name);
    const exts = try allocator.dupeSentinel(u8, extensions, 0);
    defer allocator.free(exts);
    const raw = zant_save_file_dialog_ext(name.ptr, exts.ptr) orelse return null;
    defer zant_dialog_free(raw);
    return try allocator.dupe(u8, std.mem.span(raw));
}

pub const Choice = enum { first, second, third };

/// Modal alert with up to three buttons (e.g. 保存 / 不保存 / 取消).
pub fn confirm(title: []const u8, message: []const u8, buttons: []const []const u8) Choice {
    var buf: [5][256]u8 = undefined;
    const t = zString(&buf[0], title);
    const m = zString(&buf[1], message);
    var labels: [3]?[*:0]const u8 = .{ null, null, null };
    for (buttons, 0..) |label, i| {
        if (i >= 3) break;
        labels[i] = zString(&buf[2 + i], label);
    }
    return switch (zant_confirm_dialog(t, m, labels[0], labels[1], labels[2])) {
        0 => .first,
        1 => .second,
        else => .third,
    };
}

/// Opens a URL with the default handler (browser, mail client, ...).
pub fn openUrl(url: []const u8) bool {
    var buf: [4096]u8 = undefined;
    if (url.len >= buf.len) return false;
    return zant_open_url(zStringAny(&buf, url)) != 0;
}

/// Writes `data` to an absolute path (no std.Io needed; safe from callbacks).
pub fn writeFile(path: []const u8, data: []const u8) !void {
    var buf: [4096]u8 = undefined;
    if (path.len >= buf.len) return error.NameTooLong;
    const file = std.c.fopen(zStringAny(&buf, path), "wb") orelse return error.OpenFailed;
    defer _ = std.c.fclose(file);
    if (data.len > 0 and std.c.fwrite(data.ptr, 1, data.len, file) != data.len) return error.WriteFailed;
}

fn zString(buf: *[256]u8, value: []const u8) [*:0]const u8 {
    return zStringAny(buf, value);
}

fn zStringAny(buf: anytype, value: []const u8) [*:0]const u8 {
    const n = @min(value.len, buf.len - 1);
    @memcpy(buf[0..n], value[0..n]);
    buf[n] = 0;
    return @ptrCast(buf);
}
