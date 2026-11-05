//! Port of remend `inline-code-handler.ts`.
const std = @import("std");
const Context = @import("context.zig").Context;
const p = @import("patterns.zig");

pub fn handleIncompleteInlineCode(ctx: *Context, text: []const u8) ![]const u8 {
    const scan = try ctx.scan(text);
    if (scan.open_fence != null) return text;
    const span = scan.open_span orelse return text;
    const content = text[span.start + span.run_length ..];
    if (content.len == 0 or p.isWhitespaceOrMarkers(content)) return text;
    var trailing: usize = 0;
    var i = text.len;
    while (i > 0 and text[i - 1] == '`') : (i -= 1) trailing += 1;
    if (trailing >= span.run_length) return text;
    const closer = try ctx.arena.alloc(u8, span.run_length - trailing);
    @memset(closer, '`');
    return ctx.concat(&.{ text, closer });
}
