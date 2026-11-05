//! Port of remend `comparison-operator-handler.ts`: `- >5` in a list item
//! would start a block quote; escape the `>`.
const std = @import("std");
const Context = @import("context.zig").Context;

/// Matches `^(\s*(?:[-*+]|\d+[.)]) +)>(=?\s*[$]?\d)` at `line_start`; returns
/// the index of the `>` when it matches.
fn matchAt(text: []const u8, line_start: usize) ?usize {
    var i = line_start;
    while (i < text.len and (text[i] == ' ' or text[i] == '\t')) i += 1;
    if (i >= text.len) return null;
    if (text[i] == '-' or text[i] == '*' or text[i] == '+') {
        i += 1;
    } else if (std.ascii.isDigit(text[i])) {
        while (i < text.len and std.ascii.isDigit(text[i])) i += 1;
        if (i >= text.len or (text[i] != '.' and text[i] != ')')) return null;
        i += 1;
    } else return null;
    if (i >= text.len or text[i] != ' ') return null;
    while (i < text.len and text[i] == ' ') i += 1;
    if (i >= text.len or text[i] != '>') return null;
    const gt = i;
    i += 1;
    if (i < text.len and text[i] == '=') i += 1;
    while (i < text.len and (text[i] == ' ' or text[i] == '\t')) i += 1;
    if (i < text.len and text[i] == '$') i += 1;
    if (i >= text.len or !std.ascii.isDigit(text[i])) return null;
    return gt;
}

pub fn handleComparisonOperators(ctx: *Context, text: []const u8) ![]const u8 {
    if (std.mem.indexOfScalar(u8, text, '>') == null) return text;
    var out: std.ArrayListUnmanaged(u8) = .empty;
    var last: usize = 0;
    var changed = false;
    var line_start: usize = 0;
    while (line_start <= text.len) {
        if (matchAt(text, line_start)) |gt| {
            if (!try ctx.isInsideCodeBlock(text, line_start)) {
                try out.appendSlice(ctx.arena, text[last..gt]);
                try out.appendSlice(ctx.arena, "\\>");
                last = gt + 1;
                changed = true;
            }
        }
        const nl = std.mem.indexOfScalarPos(u8, text, line_start, '\n') orelse break;
        line_start = nl + 1;
    }
    if (!changed) return text;
    try out.appendSlice(ctx.arena, text[last..]);
    return out.toOwnedSlice(ctx.arena);
}
