//! Port of remend `emphasis-handlers.ts`: completes `***`, `**`, `__`, `*`
//! and `_` that are still open at the end of a streamed text.
const std = @import("std");
const Context = @import("context.zig").Context;
const Scan = @import("scan.zig").Scan;
const p = @import("patterns.zig");

fn isWs(cp: u21) bool {
    return cp == ' ' or cp == '\t' or cp == '\n';
}

fn byteAt(text: []const u8, i: usize) u8 {
    return if (i < text.len) text[i] else 0;
}

fn shouldSkipAsterisk(scan: *Scan, index: usize, prev: u21, next: u21) bool {
    if (prev == '\\') return true;
    if (scan.inMathAt(index)) return true;
    if (prev != '*' and next == '*') {
        // first * of *** counts as a single asterisk; first of ** is skipped
        return byteAt(scan.text, index + 2) != '*';
    }
    if (prev == '*') return true;
    const prev_ws = prev == 0 or isWs(prev);
    const next_ws = next == 0 or isWs(next);
    return prev_ws and next_ws;
}

fn isWordInternalAsterisk(prev: u21, next: u21) bool {
    return prev != 0 and next != 0 and p.isWordChar(prev) and p.isWordChar(next);
}

pub fn countSingleAsterisks(ctx: *Context, text: []const u8) !usize {
    const scan = try ctx.scan(text);
    var count: usize = 0;
    var in_word_chain = false;
    var index: usize = 0;
    while (index < text.len) : (index += 1) {
        const c = text[index];
        if (c != '*' or scan.regions[index] != .prose) {
            if ((c & 0xC0) == 0x80) continue; // continuation byte of the same char
            if (!p.isWordChar(p.charAt(text, index))) in_word_chain = false;
            continue;
        }
        const prev = p.prevChar(text, index);
        const next = p.charAt(text, index + 1);
        if (shouldSkipAsterisk(scan, index, prev, next)) continue;
        const word_internal = isWordInternalAsterisk(prev, next);
        const can_open = next != 0 and !isWs(next);
        const can_close = prev != 0 and !isWs(prev);
        if (word_internal and count % 2 == 0 and !in_word_chain) continue;
        if ((can_close and count % 2 == 1) or can_open) {
            count += 1;
            in_word_chain = word_internal;
        }
    }
    return count;
}

fn shouldSkipUnderscore(scan: *Scan, index: usize, prev: u21, next: u21) bool {
    if (prev == '\\') return true;
    if (scan.inMathAt(index)) return true;
    if (scan.inLinkUrlAt(index)) return true;
    if (scan.inHtmlTagAt(index)) return true;
    if (prev == '_' or next == '_') return true;
    if (prev != 0 and next != 0 and p.isWordChar(prev) and p.isWordChar(next)) return true;
    return false;
}

pub fn countSingleUnderscores(ctx: *Context, text: []const u8) !usize {
    const scan = try ctx.scan(text);
    var count: usize = 0;
    for (text, 0..) |c, index| {
        if (c != '_' or scan.regions[index] != .prose) continue;
        if (!shouldSkipUnderscore(scan, index, p.prevChar(text, index), p.charAt(text, index + 1))) count += 1;
    }
    return count;
}

pub fn countTripleAsterisks(ctx: *Context, text: []const u8) !usize {
    const scan = try ctx.scan(text);
    var count: usize = 0;
    var run: usize = 0;
    for (text, 0..) |c, i| {
        if (c == '*' and scan.regions[i] == .prose) {
            run += 1;
        } else {
            if (run >= 3) count += run / 3;
            run = 0;
        }
    }
    if (run >= 3) count += run / 3;
    return count;
}

fn countDoubleAsterisks(ctx: *Context, text: []const u8) !usize {
    return (try ctx.scan(text)).countDoublePairs('*');
}

fn isLineBoundaryChar(cp: u21) bool {
    return cp == 0 or cp == ' ' or cp == '\t' or cp == '\n';
}

const ThematicBreakMemo = struct { line_end: isize = -1, result: bool = false };

fn isThematicBreakRun(text: []const u8, run_start: usize, prev: u21, next: u21, memo: *ThematicBreakMemo) bool {
    if (!(isLineBoundaryChar(prev) and isLineBoundaryChar(next))) return false;
    if (@as(isize, @intCast(run_start)) > memo.line_end) {
        const line_end = std.mem.indexOfScalarPos(u8, text, run_start, '\n') orelse text.len;
        memo.line_end = @intCast(line_end);
        memo.result = p.isHorizontalRule(text, run_start, '_');
    }
    return memo.result;
}

fn doubleUnderscoreRunFlips(scan: *Scan, initial_start: usize, run_end: usize, memo: *ThematicBreakMemo) bool {
    const text = scan.text;
    var run_start = initial_start;
    var escaped = false;
    if (run_start > 0 and text[run_start - 1] == '\\') {
        run_start += 1;
        escaped = true;
    }
    if (run_end < run_start or run_end - run_start < 2) return false;
    const run_length = run_end - run_start;
    const prev: u21 = if (escaped) '\\' else p.prevChar(text, run_start);
    const next = p.charAt(text, run_end);
    if (p.isWordChar(prev) and p.isWordChar(next)) return false;
    if (isThematicBreakRun(text, run_start, prev, next, memo)) return false;
    if (scan.inMathAt(run_start) or scan.inLinkUrlAt(run_start) or scan.inHtmlTagAt(run_start)) return false;
    return (run_length / 2) % 2 == 1;
}

fn hasUnmatchedDoubleUnderscore(ctx: *Context, text: []const u8) !bool {
    const scan = try ctx.scan(text);
    var memo: ThematicBreakMemo = .{};
    var unmatched = false;
    var i: usize = 0;
    while (i < text.len) {
        if (text[i] != '_' or scan.regions[i] != .prose) {
            i += 1;
            continue;
        }
        const run_start = i;
        var run_end = i + 1;
        while (run_end < text.len and text[run_end] == '_' and scan.regions[run_end] == .prose) run_end += 1;
        i = run_end;
        if (doubleUnderscoreRunFlips(scan, run_start, run_end, &memo)) unmatched = !unmatched;
    }
    return unmatched;
}

fn lineBefore(text: []const u8, marker_index: usize) []const u8 {
    const before = text[0..marker_index];
    const line_start = if (std.mem.lastIndexOfScalar(u8, before, '\n')) |nl| nl + 1 else 0;
    return text[line_start..marker_index];
}

fn shouldSkipCompletion(text: []const u8, content: []const u8, marker_index: usize, marker: u8) bool {
    if (content.len == 0 or p.isWhitespaceOrMarkers(content)) return true;
    if (p.isListItemPrefix(lineBefore(text, marker_index))) {
        if (std.mem.indexOfScalar(u8, content, '\n') != null) return true;
    }
    return p.isHorizontalRule(text, marker_index, marker);
}

/// Completes incomplete bold formatting (**)
pub fn handleIncompleteBold(ctx: *Context, text: []const u8) ![]const u8 {
    const m = p.matchTrailing(text, p.bold) orelse return text;
    const marker_index = p.lastIndexOf(text, m.marker) orelse return text;
    if (try ctx.isInsideCodeBlock(text, marker_index) or try ctx.isWithinCompleteInlineCode(text, marker_index)) return text;
    if (shouldSkipCompletion(text, m.content, marker_index, '*')) return text;
    if ((try countDoubleAsterisks(ctx, text)) % 2 == 1) {
        if (m.content.len > 0 and m.content[m.content.len - 1] == '*') return ctx.concat(&.{ text, "*" });
        return ctx.concat(&.{ text, "**" });
    }
    return text;
}

/// Completes incomplete italic formatting with double underscores (__)
pub fn handleIncompleteDoubleUnderscoreItalic(ctx: *Context, text: []const u8) ![]const u8 {
    const m = p.matchTrailing(text, p.italic) orelse {
        if (p.matchTrailing(text, p.half_complete_underscore)) |half| {
            const marker_index = p.lastIndexOf(text, half.marker) orelse return text;
            if (!(try ctx.isInsideCodeBlock(text, marker_index) or try ctx.isWithinCompleteInlineCode(text, marker_index)) and
                try hasUnmatchedDoubleUnderscore(ctx, text))
            {
                return ctx.concat(&.{ text, "_" });
            }
        }
        return text;
    };
    const marker_index = p.lastIndexOf(text, m.marker) orelse return text;
    if (try ctx.isInsideCodeBlock(text, marker_index) or try ctx.isWithinCompleteInlineCode(text, marker_index)) return text;
    if (shouldSkipCompletion(text, m.content, marker_index, '_')) return text;
    if (try hasUnmatchedDoubleUnderscore(ctx, text)) return ctx.concat(&.{ text, "__" });
    return text;
}

fn isLoneProseAsterisk(scan: *Scan, i: usize) bool {
    const text = scan.text;
    return text[i] == '*' and scan.regions[i] == .prose and
        (i == 0 or text[i - 1] != '*') and byteAt(text, i + 1) != '*' and
        (i == 0 or text[i - 1] != '\\') and !scan.inMathAt(i);
}

fn findFirstSingleAsteriskIndex(ctx: *Context, text: []const u8) !?usize {
    const scan = try ctx.scan(text);
    var from: usize = 0;
    while (std.mem.indexOfScalarPos(u8, text, from, '*')) |i| : (from = i + 1) {
        if (!isLoneProseAsterisk(scan, i)) continue;
        const prev = p.prevChar(text, i);
        const next = p.charAt(text, i + 1);
        const prev_ws = prev == 0 or isWs(prev);
        const next_ws = next == 0 or isWs(next);
        if (prev_ws and next_ws) continue;
        if (prev != 0 and next != 0 and p.isWordChar(prev) and p.isWordChar(next)) continue;
        if (next_ws) continue;
        return i;
    }
    return null;
}

/// Completes incomplete italic formatting with single asterisks (*)
pub fn handleIncompleteSingleAsteriskItalic(ctx: *Context, text: []const u8) ![]const u8 {
    _ = p.matchTrailing(text, p.single_asterisk) orelse return text;
    const first = (try findFirstSingleAsteriskIndex(ctx, text)) orelse return text;
    if (try ctx.isInsideCodeBlock(text, first) or try ctx.isWithinCompleteInlineCode(text, first)) return text;
    const after = text[first + 1 ..];
    if (after.len == 0 or p.isWhitespaceOrMarkers(after)) return text;
    if ((try countSingleAsterisks(ctx, text)) % 2 == 1) return ctx.concat(&.{ text, "*" });
    return text;
}

fn findFirstSingleUnderscoreIndex(ctx: *Context, text: []const u8) !?usize {
    const scan = try ctx.scan(text);
    var from: usize = 0;
    while (std.mem.indexOfScalarPos(u8, text, from, '_')) |i| : (from = i + 1) {
        if (scan.regions[i] == .prose and
            (i == 0 or text[i - 1] != '_') and byteAt(text, i + 1) != '_' and
            (i == 0 or text[i - 1] != '\\') and
            !scan.inMathAt(i) and !scan.inLinkUrlAt(i))
        {
            const prev = p.prevChar(text, i);
            const next = p.charAt(text, i + 1);
            if (prev != 0 and next != 0 and p.isWordChar(prev) and p.isWordChar(next)) continue;
            return i;
        }
    }
    return null;
}

fn insertClosingUnderscore(ctx: *Context, text: []const u8) ![]const u8 {
    var end = text.len;
    while (end > 0 and text[end - 1] == '\n') end -= 1;
    if (end < text.len) return ctx.concat(&.{ text[0..end], "_", text[end..] });
    return ctx.concat(&.{ text, "_" });
}

fn handleTrailingAsterisksForUnderscore(ctx: *Context, text: []const u8) !?[]const u8 {
    if (!std.mem.endsWith(u8, text, "**")) return null;
    const without = text[0 .. text.len - 2];
    if ((try countDoubleAsterisks(ctx, without)) % 2 != 1) return null;
    const first_double = std.mem.indexOf(u8, without, "**");
    const underscore = try findFirstSingleUnderscoreIndex(ctx, without);
    if (first_double != null and underscore != null and first_double.? < underscore.?) {
        return try ctx.concat(&.{ without, "_**" });
    }
    return null;
}

/// Completes incomplete italic formatting with single underscores (_)
pub fn handleIncompleteSingleUnderscoreItalic(ctx: *Context, text: []const u8) ![]const u8 {
    _ = p.matchTrailing(text, p.single_underscore) orelse return text;
    const first = (try findFirstSingleUnderscoreIndex(ctx, text)) orelse return text;
    const after = text[first + 1 ..];
    if (after.len == 0 or p.isWhitespaceOrMarkers(after)) return text;
    if (try ctx.isInsideCodeBlock(text, first) or try ctx.isWithinCompleteInlineCode(text, first)) return text;
    if ((try countSingleUnderscores(ctx, text)) % 2 == 1) {
        if (try handleTrailingAsterisksForUnderscore(ctx, text)) |result| return result;
        return insertClosingUnderscore(ctx, text);
    }
    return text;
}

fn areBoldItalicMarkersBalanced(ctx: *Context, text: []const u8) !bool {
    return (try countDoubleAsterisks(ctx, text)) % 2 == 0 and (try countSingleAsterisks(ctx, text)) % 2 == 0;
}

/// Completes incomplete bold-italic formatting (***)
pub fn handleIncompleteBoldItalic(ctx: *Context, text: []const u8) ![]const u8 {
    if (p.isFourOrMoreAsterisks(text)) return text;
    const m = p.matchTrailing(text, p.bold_italic) orelse return text;
    const marker_index = p.lastIndexOf(text, m.marker) orelse return text;
    if (m.content.len == 0 or p.isWhitespaceOrMarkers(m.content)) return text;
    if (try ctx.isInsideCodeBlock(text, marker_index) or try ctx.isWithinCompleteInlineCode(text, marker_index)) return text;
    if (p.isHorizontalRule(text, marker_index, '*')) return text;
    if ((try countTripleAsterisks(ctx, text)) % 2 == 1) {
        if (try areBoldItalicMarkersBalanced(ctx, text)) return text;
        return ctx.concat(&.{ text, "***" });
    }
    return text;
}
