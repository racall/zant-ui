const std = @import("std");

extern fn zant_open_file_dialog() ?[*:0]u8;
extern fn zant_save_file_dialog(default_name: ?[*:0]const u8) ?[*:0]u8;
extern fn zant_dialog_free(value: ?[*:0]u8) void;

pub fn openFile(allocator: std.mem.Allocator) !?[]u8 {
    const raw = zant_open_file_dialog() orelse return null;
    defer zant_dialog_free(raw);
    return try allocator.dupe(u8, std.mem.span(raw));
}

pub fn saveFile(allocator: std.mem.Allocator, default_name: []const u8) !?[]u8 {
    const name = try allocator.dupeZ(u8, default_name);
    defer allocator.free(name);
    const raw = zant_save_file_dialog(name.ptr) orelse return null;
    defer zant_dialog_free(raw);
    return try allocator.dupe(u8, std.mem.span(raw));
}
