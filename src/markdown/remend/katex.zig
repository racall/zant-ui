//! Port of remend `katex-handler.ts`: closes `$$` blocks (and, opt-in,
//! single-dollar inline math).
const std = @import("std");
const Context = @import("context.zig").Context;

fn countSingleDollars(ctx: *Context, text: []const u8) !usize {
    const scan = try ctx.scan(text);
    var count: usize = 0;
    var i: usize = 0;
    while (i < text.len) : (i += 1) {
        if (text[i] == '\\') {
            i += 1;
            continue;
        }
        if (scan.regions[i] != .prose) continue;
        if (text[i] == '$') {
            if (i + 1 < text.len and text[i + 1] == '$') {
                i += 1;
            } else {
                count += 1;
            }
        }
    }
    return count;
}

fn addClosingKatex(ctx: *Context, text: []const u8) ![]const u8 {
    if (std.mem.endsWith(u8, text, "$") and !std.mem.endsWith(u8, text, "$$")) return ctx.concat(&.{ text, "$" });
    const first = std.mem.indexOf(u8, text, "$$");
    const newline_after = first != null and std.mem.indexOfScalarPos(u8, text, first.?, '\n') != null;
    if (newline_after and !std.mem.endsWith(u8, text, "\n")) return ctx.concat(&.{ text, "\n$$" });
    return ctx.concat(&.{ text, "$$" });
}

pub fn handleIncompleteBlockKatex(ctx: *Context, text: []const u8) ![]const u8 {
    if ((try ctx.scan(text)).countDoublePairs('$') % 2 == 0) return text;
    return addClosingKatex(ctx, text);
}

pub fn handleIncompleteInlineKatex(ctx: *Context, text: []const u8) ![]const u8 {
    if ((try countSingleDollars(ctx, text)) % 2 == 1) return ctx.concat(&.{ text, "$" });
    return text;
}
