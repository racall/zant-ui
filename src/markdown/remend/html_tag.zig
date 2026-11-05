//! Port of remend `html-tag-handler.ts`: strips an HTML tag that has not
//! received its closing `>` yet.
const std = @import("std");
const Context = @import("context.zig").Context;

fn startsTag(text: []const u8, index: usize) bool {
    if (index + 1 >= text.len) return false;
    const c = text[index + 1];
    return std.ascii.isAlphabetic(c) or c == '/';
}

pub fn handleIncompleteHtmlTag(ctx: *Context, text: []const u8) ![]const u8 {
    // /<[a-zA-Z/][^>]*$/ — leftmost "<x" with no ">" after it
    var start: ?usize = null;
    var i: usize = 0;
    while (std.mem.indexOfScalarPos(u8, text, i, '<')) |lt| : (i = lt + 1) {
        if (startsTag(text, lt) and std.mem.indexOfScalarPos(u8, text, lt, '>') == null) {
            start = lt;
            break;
        }
    }
    const match_index = start orelse return text;
    const check_math = std.mem.indexOfScalar(u8, text, '$') != null or
        std.mem.indexOf(u8, text, "\\(") != null or std.mem.indexOf(u8, text, "\\[") != null;
    var index = match_index;
    while (index < text.len) : (index += 1) {
        if (text[index] != '<' or !startsTag(text, index)) continue;
        if (try ctx.isInsideCodeBlock(text, index)) continue;
        if (check_math and (try ctx.scan(text)).inMathAt(index)) continue;
        return std.mem.trimEnd(u8, text[0..index], " \t\r\n");
    }
    return text;
}
