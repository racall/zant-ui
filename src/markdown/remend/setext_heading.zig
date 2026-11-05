//! Port of remend `setext-heading-handler.ts`: a trailing `-`/`--`/`=`/`==`
//! under a text line would turn that line into a heading mid-stream; append a
//! zero-width space to break the pattern.
const std = @import("std");
const Context = @import("context.zig").Context;

fn onlyRun(line: []const u8, c: u8) bool {
    const t = std.mem.trim(u8, line, " \t\r\n");
    if (t.len < 1 or t.len > 2) return false;
    for (t) |x| if (x != c) return false;
    return true;
}

/// `/^[\s]*-{1,2}[\s]+$/` (a dash run followed by whitespace)
fn runWithTrailingSpace(line: []const u8, c: u8) bool {
    var i: usize = 0;
    while (i < line.len and std.ascii.isWhitespace(line[i])) i += 1;
    const start = i;
    while (i < line.len and line[i] == c) i += 1;
    const run = i - start;
    if (run < 1 or run > 2) return false;
    if (i >= line.len) return false;
    while (i < line.len) : (i += 1) if (!std.ascii.isWhitespace(line[i])) return false;
    return true;
}

pub fn handleIncompleteSetextHeading(ctx: *Context, text: []const u8) ![]const u8 {
    const nl = std.mem.lastIndexOfScalar(u8, text, '\n') orelse return text;
    const last_line = text[nl + 1 ..];
    const previous = text[0..nl];
    const prev_line = if (std.mem.lastIndexOfScalar(u8, previous, '\n')) |j| previous[j + 1 ..] else previous;
    const prev_has_text = std.mem.trim(u8, prev_line, " \t\r\n").len > 0;
    inline for (.{ '-', '=' }) |c| {
        if (onlyRun(last_line, c) and !runWithTrailingSpace(last_line, c) and prev_has_text) {
            return ctx.concat(&.{ text, "\u{200B}" });
        }
    }
    return text;
}
