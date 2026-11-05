//! Table (Streamdown `table/*`): bordered grid with a header row and
//! copy (Markdown / CSV) and download (CSV or Markdown) controls.
const std = @import("std");
const ui = @import("../../ui.zig");
const flex = @import("../../layout/flex.zig");
const ast = @import("../ast.zig");
const table_utils = @import("../table_utils.zig");
const spans = @import("spans.zig");
const Ctx = @import("context.zig").Ctx;
const controls = @import("controls.zig");
const act = @import("actions.zig");

fn cellAlign(a: ast.Align) flex.Align {
    return switch (a) {
        .center => .center,
        .right => .end,
        else => .start,
    };
}

fn rowNode(ctx: Ctx, cells: []const []const ast.Span, aligns: []const ast.Align, header: bool, key: []const u8) !ui.Node {
    const theme = &ctx.options.theme;
    var children: std.ArrayListUnmanaged(ui.Node) = .empty;
    for (cells, 0..) |cell, c| {
        const content = try spans.richNode(ctx, cell, .{
            .size = ctx.size * 0.875,
            .color = ctx.fg,
            .weight = if (header) .semibold else .regular,
            .family = ctx.family(),
        }, false);
        try children.append(ctx.arena, ui.column(.{
            .grow = 1,
            .shrink = 1,
            .basis = 0,
            .padding = .{ .left = 12, .right = 12, .top = 8, .bottom = 8 },
            .align_items = cellAlign(if (c < aligns.len) aligns[c] else .none),
            .children = &.{content},
        }));
    }
    return ui.row(.{
        .key = key,
        .width_percent = 100,
        .align_items = .stretch,
        .background = if (header) theme.table_header_bg else null,
        .children = children.items,
    });
}

pub fn render(ctx: Ctx, table: ast.Table) !ui.Node {
    const theme = &ctx.options.theme;
    const a = ctx.arena;
    var rows: std.ArrayListUnmanaged(ui.Node) = .empty;
    try rows.append(a, try rowNode(ctx, table.headers, table.aligns, true, ctx.subKey("h", .{})));
    for (table.rows, 0..) |row, r| {
        try rows.append(a, ui.divider(.{ .color = theme.table_border }));
        try rows.append(a, try rowNode(ctx, row, table.aligns, false, ctx.subKey("r{d}", .{r})));
    }
    const grid = ui.column(.{
        .key = ctx.subKey("grid", .{}),
        .width_percent = 100,
        .border_color = theme.table_border,
        .border_width = 1,
        .radius = 8,
        .clip = true,
        .align_items = .stretch,
        .children = rows.items,
    });
    const tc = ctx.options.controls.table;
    if (!tc.copy and !tc.download) return grid;

    const data: table_utils.TableData = .{ .headers = table.raw_headers, .rows = table.raw_rows };
    const md = try table_utils.toMarkdown(a, data);
    const csv = try table_utils.toCsv(a, data, .comma);
    const busy = ctx.options.is_animating and ctx.caret;
    var buttons: std.ArrayListUnmanaged(ui.Node) = .empty;
    if (tc.copy) {
        try buttons.append(a, controls.button(ctx, ctx.subKey("copy-md", .{}), ctx.options.labels.copy_markdown, try act.Copy.action(a, md), !busy));
        try buttons.append(a, controls.button(ctx, ctx.subKey("copy-csv", .{}), ctx.options.labels.copy_csv, try act.Copy.action(a, csv), !busy));
    }
    if (tc.download) {
        try buttons.append(a, controls.button(ctx, ctx.subKey("download", .{}), ctx.options.labels.download, try act.Download.action(a, .{
            .filename = "table.csv",
            .extensions = "csv,md",
            .text = csv,
            .alt_extension = ".md",
            .alt_text = md,
        }), !busy));
    }
    return ui.column(.{
        .key = ctx.key,
        .width_percent = 100,
        .gap = 4,
        .align_items = .stretch,
        .children = &.{
            ui.row(.{ .width_percent = 100, .justify = .end, .gap = controls.gap, .children = buttons.items }),
            grid,
        },
    });
}
