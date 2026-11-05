//! Port of remend `single-tilde-handler.ts`: escapes a single `~` between
//! word characters (`20~25`) so it is not read as strikethrough.
const std = @import("std");
const Context = @import("context.zig").Context;
const p = @import("patterns.zig");

/// `text.replace(/([\p{L}\p{N}_])~(?!~)(?=[\p{L}\p{N}_])/gu, ...)`
pub fn handleSingleTildeEscape(ctx: *Context, text: []const u8) ![]const u8 {
    if (std.mem.indexOfScalar(u8, text, '~') == null) return text;
    var out: std.ArrayListUnmanaged(u8) = .empty;
    var copied: usize = 0; // bytes of `text` already written to `out`
    var regex_pos: usize = 0; // where the global regex resumes
    var from: usize = 0;
    while (std.mem.indexOfScalarPos(u8, text, from, '~')) |t| : (from = t + 1) {
        if (t == 0) continue;
        const prev = p.prevChar(text, t);
        const prev_len = std.unicode.utf8CodepointSequenceLength(prev) catch 1;
        if (t < prev_len or t - prev_len < regex_pos) continue;
        if (!p.isWordChar(prev)) continue;
        const next = p.charAt(text, t + 1);
        if (next == '~' or !p.isWordChar(next)) continue;
        regex_pos = t + 1;
        if (try ctx.isInsideCodeBlock(text, t)) continue;
        try out.appendSlice(ctx.arena, text[copied..t]);
        try out.appendSlice(ctx.arena, "\\~");
        copied = t + 1;
    }
    if (copied == 0) return text;
    try out.appendSlice(ctx.arena, text[copied..]);
    return out.toOwnedSlice(ctx.arena);
}
