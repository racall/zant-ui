//! Port of remend `link-image-handler.ts`: heals `[text`, `[text](partial`
//! and `![alt` at the end of a streamed text.
const std = @import("std");
const Context = @import("context.zig").Context;
const p = @import("patterns.zig");

pub const LinkMode = enum { protocol, text_only };

/// Sentinel URL for links whose target has not streamed in yet.
pub const incomplete_link = "streamdown:incomplete-link";
/// Sentinel source for images whose source has not streamed in yet.
pub const incomplete_image = "streamdown:incomplete-image";

fn handleIncompleteUrl(ctx: *Context, text: []const u8, last_paren: usize, mode: LinkMode) !?[]const u8 {
    const after = text[last_paren + 2 ..];
    if (std.mem.indexOfScalar(u8, after, ')') != null) return null;
    const open = p.findMatchingOpeningBracket(text, last_paren) orelse return null;
    if (try ctx.isInsideCodeBlock(text, open)) return null;
    const is_image = open > 0 and text[open - 1] == '!';
    const start = if (is_image) open - 1 else open;
    const before = text[0..start];
    const label = text[open + 1 .. last_paren];
    if (is_image) return try ctx.concat(&.{ before, "![", label, "](", incomplete_image, ")" });
    if (mode == .text_only) return try ctx.concat(&.{ before, label });
    return try ctx.concat(&.{ before, "[", label, "](", incomplete_link, ")" });
}

fn findFirstIncompleteBracket(ctx: *Context, text: []const u8, max_pos: usize) !usize {
    var j: usize = 0;
    while (j < max_pos) : (j += 1) {
        if (text[j] == '[' and !try ctx.isInsideCodeBlock(text, j)) {
            if (j > 0 and text[j - 1] == '!') continue;
            const closing = p.findMatchingClosingBracket(text, j) orelse return j;
            if (closing + 1 < text.len and text[closing + 1] == '(') {
                if (std.mem.indexOfScalarPos(u8, text, closing + 2, ')')) |url_end| j = url_end;
            }
        }
    }
    return max_pos;
}

fn handleIncompleteText(ctx: *Context, text: []const u8, i: usize, mode: LinkMode) !?[]const u8 {
    const is_image = i > 0 and text[i - 1] == '!';
    const open = if (is_image) i - 1 else i;
    const has_close = std.mem.indexOfScalar(u8, text[i + 1 ..], ']') != null;
    const closing = if (has_close) p.findMatchingClosingBracket(text, i) else null;
    if (closing != null) return null;
    const before = text[0..open];
    if (is_image) return try ctx.concat(&.{ before, "![", text[i + 1 ..], "](", incomplete_image, ")" });
    if (mode == .text_only) {
        const first = try findFirstIncompleteBracket(ctx, text, i);
        return try ctx.concat(&.{ text[0..first], text[first + 1 ..] });
    }
    return try ctx.concat(&.{ text, "](", incomplete_link, ")" });
}

fn healTrailingLinkOrImage(ctx: *Context, text: []const u8, mode: LinkMode) ![]const u8 {
    if (std.mem.lastIndexOf(u8, text, "](")) |last_paren| {
        if (!try ctx.isInsideCodeBlock(text, last_paren)) {
            if (try handleIncompleteUrl(ctx, text, last_paren, mode)) |result| return result;
        }
    }
    var search_end = text.len;
    while (std.mem.lastIndexOfScalar(u8, text[0..search_end], '[')) |i| {
        if (!try ctx.isInsideCodeBlock(text, i)) {
            if (try handleIncompleteText(ctx, text, i, mode)) |result| return result;
        }
        if (i == 0) break;
        search_end = i;
    }
    return text;
}

const max_heal_passes = 32;

pub fn handleIncompleteLinksAndImages(ctx: *Context, text: []const u8, mode: LinkMode) ![]const u8 {
    var current = text;
    var pass: usize = 0;
    while (pass < max_heal_passes) : (pass += 1) {
        const next = try healTrailingLinkOrImage(ctx, current, mode);
        if (next.len >= current.len) return next;
        current = next;
    }
    return current;
}
