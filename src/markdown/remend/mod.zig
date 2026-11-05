//! Zig port of Streamdown's `remend`: heals incomplete Markdown at the end of
//! a streamed text so partial syntax renders as its finished form
//! (`**bo` → `**bo**`, `[link](htt` → `[link](streamdown:incomplete-link)`).
//!
//! Handlers run in Streamdown's priority order; each is a separate module
//! mirroring `packages/remend/src`.
const std = @import("std");
pub const Context = @import("context.zig").Context;
pub const scan = @import("scan.zig");
pub const patterns = @import("patterns.zig");
const emphasis = @import("emphasis.zig");
const inline_code = @import("inline_code.zig");
const strikethrough = @import("strikethrough.zig");
const katex = @import("katex.zig");
const single_tilde = @import("single_tilde.zig");
const setext_heading = @import("setext_heading.zig");
const comparison_operator = @import("comparison_operator.zig");
const html_tag = @import("html_tag.zig");
const link_image = @import("link_image.zig");

pub const LinkMode = link_image.LinkMode;
pub const incomplete_link = link_image.incomplete_link;
pub const incomplete_image = link_image.incomplete_image;

/// Custom handler (Streamdown's `RemendHandler`). Runs after the built-ins
/// with the same priority unless `priority` says otherwise.
pub const Handler = struct {
    name: []const u8,
    priority: u8 = 100,
    handle: *const fn (ctx: *Context, text: []const u8) anyerror![]const u8,
};

/// Every option defaults to enabled, except `inline_katex` (opt-in).
pub const Options = struct {
    bold: bool = true,
    bold_italic: bool = true,
    comparison_operators: bool = true,
    html_tags: bool = true,
    images: bool = true,
    inline_code: bool = true,
    inline_katex: bool = false,
    italic: bool = true,
    katex: bool = true,
    link_mode: LinkMode = .protocol,
    links: bool = true,
    setext_headings: bool = true,
    single_tilde: bool = true,
    strikethrough: bool = true,
    handlers: []const Handler = &.{},
};

const Step = enum {
    single_tilde,
    comparison_operators,
    html_tags,
    setext_headings,
    links,
    bold_italic,
    bold,
    italic_double_underscore,
    italic_single_asterisk,
    italic_single_underscore,
    inline_code,
    strikethrough,
    katex,
    inline_katex,

    fn priority(self: Step) u8 {
        return switch (self) {
            .single_tilde => 0,
            .comparison_operators => 5,
            .html_tags => 10,
            .setext_headings => 15,
            .links => 20,
            .bold_italic => 30,
            .bold => 35,
            .italic_double_underscore => 40,
            .italic_single_asterisk => 41,
            .italic_single_underscore => 42,
            .inline_code => 50,
            .strikethrough => 60,
            .katex => 70,
            .inline_katex => 75,
        };
    }

    fn enabled(self: Step, o: Options) bool {
        return switch (self) {
            .single_tilde => o.single_tilde,
            .comparison_operators => o.comparison_operators,
            .html_tags => o.html_tags,
            .setext_headings => o.setext_headings,
            .links => o.links or o.images,
            .bold_italic => o.bold_italic,
            .bold => o.bold,
            .italic_double_underscore, .italic_single_asterisk, .italic_single_underscore => o.italic,
            .inline_code => o.inline_code,
            .strikethrough => o.strikethrough,
            .katex => o.katex,
            .inline_katex => o.inline_katex,
        };
    }

    fn run(self: Step, ctx: *Context, text: []const u8, o: Options) ![]const u8 {
        return switch (self) {
            .single_tilde => single_tilde.handleSingleTildeEscape(ctx, text),
            .comparison_operators => comparison_operator.handleComparisonOperators(ctx, text),
            .html_tags => html_tag.handleIncompleteHtmlTag(ctx, text),
            .setext_headings => setext_heading.handleIncompleteSetextHeading(ctx, text),
            .links => link_image.handleIncompleteLinksAndImages(ctx, text, o.link_mode),
            .bold_italic => emphasis.handleIncompleteBoldItalic(ctx, text),
            .bold => emphasis.handleIncompleteBold(ctx, text),
            .italic_double_underscore => emphasis.handleIncompleteDoubleUnderscoreItalic(ctx, text),
            .italic_single_asterisk => emphasis.handleIncompleteSingleAsteriskItalic(ctx, text),
            .italic_single_underscore => emphasis.handleIncompleteSingleUnderscoreItalic(ctx, text),
            .inline_code => inline_code.handleIncompleteInlineCode(ctx, text),
            .strikethrough => strikethrough.handleIncompleteStrikethrough(ctx, text),
            .katex => katex.handleIncompleteBlockKatex(ctx, text),
            .inline_katex => katex.handleIncompleteInlineKatex(ctx, text),
        };
    }
};

fn stripSingleTrailingSpace(text: []const u8) []const u8 {
    if (std.mem.endsWith(u8, text, " ") and !std.mem.endsWith(u8, text, "  ")) return text[0 .. text.len - 1];
    return text;
}

/// Heals `text`. The result is allocated in `arena` (or is a slice of
/// `text` when nothing changed).
pub fn remend(arena: std.mem.Allocator, text: []const u8, options: Options) ![]const u8 {
    if (text.len == 0) return text;
    var ctx: Context = .{ .arena = arena };
    var result = stripSingleTrailingSpace(text);

    const builtin_count = std.enums.values(Step).len;
    var custom_index: usize = 0;
    var step_index: usize = 0;
    // Merge built-ins (already priority-sorted) with custom handlers (stable).
    while (step_index < builtin_count or custom_index < options.handlers.len) {
        const step: ?Step = if (step_index < builtin_count) @fromBackingInt(@intCast(step_index)) else null;
        const custom: ?Handler = if (custom_index < options.handlers.len) options.handlers[custom_index] else null;
        const take_custom = custom != null and (step == null or custom.?.priority < step.?.priority());
        if (take_custom) {
            result = try custom.?.handle(&ctx, result);
            custom_index += 1;
            continue;
        }
        step_index += 1;
        const s = step.?;
        if (!s.enabled(options)) continue;
        result = try s.run(&ctx, result, options);
        if (s == .links and options.link_mode == .protocol) {
            if (std.mem.endsWith(u8, result, "](" ++ incomplete_link ++ ")") or
                std.mem.endsWith(u8, result, "](" ++ incomplete_image ++ ")")) return result;
        }
    }
    return stripSingleTrailingSpace(result);
}

/// Convenience wrapper that copies the healed text into `allocator`.
pub fn remendAlloc(allocator: std.mem.Allocator, text: []const u8, options: Options) ![]u8 {
    var arena = std.heap.ArenaAllocator.init(allocator);
    defer arena.deinit();
    return allocator.dupe(u8, try remend(arena.allocator(), text, options));
}

test {
    _ = scan;
    _ = patterns;
}

test "basic completions" {
    const a = std.testing.allocator;
    const cases = [_][2][]const u8{
        .{ "Text with **bold", "Text with **bold**" },
        .{ "**xxx*", "**xxx**" },
        .{ "Use `code", "Use `code`" },
        .{ "~~gone", "~~gone~~" },
        .{ "see [docs](https://exa", "see [docs](streamdown:incomplete-link)" },
        .{ "*it", "*it*" },
        .{ "$$x^2", "$$x^2$$" },
        .{ "snake_case_name", "snake_case_name" },
        .{ "Range 20~25", "Range 20\\~25" },
    };
    for (cases) |case| {
        const out = try remendAlloc(a, case[0], .{});
        defer a.free(out);
        try std.testing.expectEqualStrings(case[1], out);
    }
}
