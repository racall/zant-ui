//! Port of remend `strikethrough-handler.ts`.
const std = @import("std");
const Context = @import("context.zig").Context;
const p = @import("patterns.zig");

pub fn handleIncompleteStrikethrough(ctx: *Context, text: []const u8) ![]const u8 {
    if (p.matchTrailing(text, p.strikethrough)) |m| {
        if (m.content.len == 0 or p.isWhitespaceOrMarkers(m.content)) return text;
        const marker_index = p.lastIndexOf(text, m.marker) orelse return text;
        if (try ctx.isInsideCodeBlock(text, marker_index) or try ctx.isWithinCompleteInlineCode(text, marker_index)) return text;
        if ((try ctx.scan(text)).countDoublePairs('~') % 2 == 1) return ctx.concat(&.{ text, "~~" });
    } else if (p.matchTrailing(text, p.half_complete_tilde)) |half| {
        const marker_index = p.lastIndexOf(text, half.whole[0..2]) orelse return text;
        if (try ctx.isInsideCodeBlock(text, marker_index) or try ctx.isWithinCompleteInlineCode(text, marker_index)) return text;
        if ((try ctx.scan(text)).countDoublePairs('~') % 2 == 1) return ctx.concat(&.{ text, "~" });
    }
    return text;
}
