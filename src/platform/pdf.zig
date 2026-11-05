const std = @import("std");

extern fn zant_pdf_page_count(path: [*:0]const u8) c_int;
extern fn zant_pdf_render_page(path: [*:0]const u8, page: c_int, width: c_int, height: c_int, out_path: [*]u8, capacity: usize) c_int;

pub fn pageCount(path: [:0]const u8) usize {
    const count = zant_pdf_page_count(path.ptr);
    return if (count > 0) @intCast(count) else 0;
}

pub fn renderPage(path: [:0]const u8, page: usize, width: u32, height: u32, out: []u8) ?[]const u8 {
    if (zant_pdf_render_page(path.ptr, @intCast(page), @intCast(width), @intCast(height), out.ptr, out.len) != 0) return null;
    return std.mem.sliceTo(out, 0);
}

extern fn zant_pdf_page_text(path: [*:0]const u8, page: c_int, out_text: [*]u8, capacity: usize) c_int;

pub fn pageText(path: [:0]const u8, page: usize, out: []u8) ?[]const u8 {
    if (zant_pdf_page_text(path.ptr, @intCast(page), out.ptr, out.len) != 0) return null;
    return std.mem.sliceTo(out, 0);
}
