//! Mermaid diagram block (Streamdown `mermaid/*`) with copy/download of the
//! source. Unsupported diagram types fall back to a code block.
const std = @import("std");
const ui = @import("../../ui.zig");
const ast = @import("../ast.zig");
const mermaid = @import("../mermaid.zig");
const Ctx = @import("context.zig").Ctx;
const controls = @import("controls.zig");
const act = @import("actions.zig");
const code_block = @import("code_block.zig");

pub fn render(ctx: Ctx, code: ast.Code, caret: bool) !ui.Node {
    const theme = &ctx.options.theme;
    const a = ctx.arena;
    // Streamdown shows the source until the fence closes
    if (!code.closed) return code_block.render(ctx, code, caret);
    const drawing_value = (try mermaid.render(a, code.text, .{
        .font_size = ctx.options.font_size * 0.875,
        .max_width = ctx.options.content_width - 32,
        .colors = .{
            .text = theme.foreground,
            .node_fill = if (theme.dark) 0xff1f2937 else 0xfff4f4f5,
            .node_border = if (theme.dark) 0xff4b5563 else 0xffa1a1aa,
            .edge = theme.muted_foreground,
            .label_bg = theme.background,
            .muted = theme.muted_foreground,
        },
    })) orelse return code_block.render(ctx, code, caret);
    const drawing = try a.create(@TypeOf(drawing_value));
    drawing.* = drawing_value;

    var buttons: std.ArrayListUnmanaged(ui.Node) = .empty;
    const mc = ctx.options.controls.mermaid;
    if (mc.copy) try buttons.append(a, controls.button(ctx, ctx.subKey("copy", .{}), ctx.options.labels.copy, try act.Copy.action(a, code.text), true));
    if (mc.download) try buttons.append(a, controls.button(ctx, ctx.subKey("download", .{}), ctx.options.labels.download, try act.Download.action(a, .{
        .filename = "diagram.mmd",
        .extensions = "mmd",
        .text = code.text,
    }), true));
    return ui.column(.{
        .key = ctx.key,
        .width_percent = 100,
        .border_color = theme.border,
        .border_width = 1,
        .radius = 10,
        .clip = true,
        .padding = .{ .bottom = 16 },
        .align_items = .stretch,
        .children = &.{
            ui.row(.{ .width_percent = 100, .justify = .end, .padding = .{ .top = 4, .right = 6 }, .gap = controls.gap, .children = buttons.items }),
            ui.row(.{ .width_percent = 100, .justify = .center, .children = &.{ui.vector(.{ .drawing = drawing })} }),
        },
    });
}
