//! Code block (Streamdown `code-block/*`): header with the language and
//! copy/download controls, highlighted body with optional line numbers,
//! scrolling past `code_max_height`.
const std = @import("std");
const ui = @import("../../ui.zig");
const node_mod = @import("../../core/node.zig");
const ast = @import("../ast.zig");
const highlight = @import("../highlight.zig");
const theme_mod = @import("../theme.zig");
const Ctx = @import("context.zig").Ctx;
const controls = @import("controls.zig");
const act = @import("actions.zig");

fn tokenColor(c: theme_mod.CodeColors, kind: highlight.Kind) u32 {
    return switch (kind) {
        .plain => c.plain,
        .keyword => c.keyword,
        .string => c.string,
        .comment => c.comment,
        .number => c.number,
        .function => c.function,
        .type => c.type,
        .constant => c.constant,
        .property => c.property,
        .tag => c.tag,
        .attribute => c.attribute,
        .punctuation => c.punctuation,
        .meta => c.meta,
        .inserted => c.inserted,
        .deleted => c.deleted,
    };
}

pub fn render(ctx: Ctx, code: ast.Code, caret: bool) !ui.Node {
    const theme = &ctx.options.theme;
    const size = ctx.options.font_size * 0.8125;
    const line_height: f32 = 1.65;
    const a = ctx.arena;

    // header: language + controls
    const incomplete = !code.closed and ctx.options.is_animating;
    const lang_label = ui.text(if (code.lang.len > 0) code.lang else "text", .{ .size = 12, .family = .mono, .color = theme.muted_foreground });
    var buttons: std.ArrayListUnmanaged(ui.Node) = .empty;
    if (ctx.options.controls.code.copy) {
        try buttons.append(a, controls.button(ctx, ctx.subKey("copy", .{}), ctx.options.labels.copy, try act.Copy.action(a, code.text), !incomplete));
    }
    if (ctx.options.controls.code.download) {
        const ext = highlight.extension(code.lang);
        try buttons.append(a, controls.button(ctx, ctx.subKey("download", .{}), ctx.options.labels.download, try act.Download.action(a, .{
            .filename = try std.fmt.allocPrint(a, "file.{s}", .{ext}),
            .extensions = "",
            .text = code.text,
        }), !incomplete));
    }
    const header = ui.row(.{
        .key = ctx.subKey("header", .{}),
        .width_percent = 100,
        .height = 34,
        .padding = .{ .left = 14, .right = 6 },
        .align_items = .center,
        .justify = .space_between,
        .background = theme.code_header_bg,
        .children = &.{
            lang_label,
            ui.row(.{ .gap = controls.gap, .align_items = .center, .children = buttons.items }),
        },
    });

    // body: highlighted lines
    var state: highlight.State = .{};
    var lines = std.mem.splitScalar(u8, code.text, '\n');
    var line_nodes: std.ArrayListUnmanaged(ui.Node) = .empty;
    var all_runs: std.ArrayListUnmanaged(node_mod.RichSpan) = .empty;
    var count: usize = 0;
    var total_lines: usize = 0;
    {
        var counter = std.mem.splitScalar(u8, code.text, '\n');
        while (counter.next()) |_| total_lines += 1;
    }
    const digits = std.fmt.count("{d}", .{total_lines});
    const gutter_w = @as(f32, @floatFromInt(digits)) * size * 0.62 + 18;
    while (lines.next()) |line| : (count += 1) {
        const tokens = try highlight.line(a, code.lang, line, &state);
        var runs: std.ArrayListUnmanaged(node_mod.RichSpan) = .empty;
        for (tokens) |tok| {
            var run: node_mod.RichSpan = .{ .text = tok.text, .font_size = size, .family = .mono, .color = tokenColor(theme.code, tok.kind) };
            if (tok.kind == .inserted) run.background = theme.code.inserted_bg;
            if (tok.kind == .deleted) run.background = theme.code.deleted_bg;
            try runs.append(a, run);
        }
        const last = count + 1 == total_lines;
        if (caret and last) try runs.append(a, .{ .text = ctx.options.caret.glyph(), .font_size = size, .color = theme.caret });
        if (runs.items.len == 0) try runs.append(a, .{ .text = " ", .font_size = size, .family = .mono, .color = theme.code.plain });
        if (ctx.options.line_numbers) {
            try line_nodes.append(a, ui.row(.{
                .width_percent = 100,
                .align_items = .start,
                .children = &.{
                    ui.row(.{ .width = gutter_w, .justify = .end, .children = &.{ui.text(try std.fmt.allocPrint(a, "{d}", .{count + 1}), .{ .size = size, .family = .mono, .color = theme.line_number, .line_height = line_height })} }),
                    ui.column(.{ .grow = 1, .shrink = 1, .basis = 0, .margin = .{ .left = 12 }, .children = &.{ui.richText(.{ .spans = runs.items, .line_height = line_height })} }),
                },
            }));
        } else {
            if (count > 0) try all_runs.append(a, .{ .text = "\n", .font_size = size, .family = .mono, .color = theme.code.plain });
            try all_runs.appendSlice(a, runs.items);
        }
    }
    const body_children = if (ctx.options.line_numbers) line_nodes.items else &[_]ui.Node{ui.richText(.{ .spans = all_runs.items, .line_height = line_height })};
    const est_height = @as(f32, @floatFromInt(total_lines)) * size * line_height + 28;
    const max_h = ctx.options.code_max_height;
    const scrolls = max_h > 0 and est_height > max_h;
    const body = ui.column(.{
        .key = ctx.subKey("body", .{}),
        .width_percent = 100,
        .padding = .{ .left = if (ctx.options.line_numbers) 2 else 16, .right = 16, .top = 14, .bottom = 14 },
        .height = if (scrolls) max_h else null,
        .scroll = scrolls,
        .background = theme.code_bg,
        .align_items = .stretch,
        .children = body_children,
    });
    return ui.column(.{
        .key = ctx.key,
        .width_percent = 100,
        .border_color = theme.code_border,
        .border_width = 1,
        .radius = 10,
        .clip = true,
        .align_items = .stretch,
        .children = &.{ header, ui.divider(.{ .color = theme.code_border }), body },
    });
}
