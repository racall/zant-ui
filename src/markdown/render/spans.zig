//! Inline spans → rich text runs (links, inline code, inline math embeds,
//! highlights, footnote references, the streaming caret).
const std = @import("std");
const ui = @import("../../ui.zig");
const node_mod = @import("../../core/node.zig");
const text_mod = @import("../../renderer/text.zig");
const ast = @import("../ast.zig");
const safety = @import("../safety.zig");
const remend = @import("../remend/mod.zig");
const math = @import("../../math/mod.zig");
const Ctx = @import("context.zig").Ctx;

pub const Base = struct {
    size: f32,
    color: u32,
    weight: text_mod.Weight = .regular,
    family: text_mod.Family = .sans,
    italic: bool = false,
};

pub fn toRich(ctx: Ctx, spans: []const ast.Span, base: Base, with_caret: bool) ![]node_mod.RichSpan {
    const theme = &ctx.options.theme;
    var out: std.ArrayListUnmanaged(node_mod.RichSpan) = .empty;
    for (spans) |span| {
        const st = span.style;
        var run: node_mod.RichSpan = .{
            .text = span.text,
            .font_size = base.size,
            .weight = if (st.bold) (if (@backingInt(base.weight) >= @backingInt(text_mod.Weight.semibold)) .bold else .semibold) else base.weight,
            .color = base.color,
            .italic = base.italic or st.italic,
            .family = base.family,
            .strike = st.strike,
            .underline = st.underline,
        };
        if (st.superscript or st.subscript) run.font_size = base.size * 0.75;
        if (st.highlight) run.background = theme.highlight_bg;
        if (st.code) {
            run.family = .mono;
            run.font_size = base.size * 0.875;
            run.background = theme.inline_code_bg;
            run.color = theme.inline_code_fg;
            // pad the code chip with thin spaces
            run.text = try std.mem.concat(ctx.arena, u8, &.{ "\u{2009}", span.text, "\u{2009}" });
        }
        if (st.math) {
            const drawing = try math.inlineDrawing(ctx.arena, span.text, .{
                .size = base.size,
                .color = if (base.color == theme.foreground) theme.math else base.color,
                .error_color = theme.math_error,
            });
            run.embed = drawing;
            run.text = "\u{FFFC}";
        }
        if (st.footnote_ref) {
            run.color = theme.link;
            run.text = try std.mem.concat(ctx.arena, u8, &.{ "[", span.text, "]" });
        }
        if (st.image) {
            run.color = theme.muted_foreground;
            run.text = try std.mem.concat(ctx.arena, u8, &.{ "🖼 ", span.text });
        }
        if (st.link and !st.image) {
            const incomplete = std.mem.eql(u8, span.url, "#incomplete") or std.mem.eql(u8, span.url, remend.incomplete_link);
            if (!incomplete and safety.check(ctx.options.policy, .link, span.url)) {
                run.color = theme.link;
                run.underline = true;
                run.link = span.url;
            } else {
                run.color = if (incomplete) theme.link else theme.link_blocked;
            }
        }
        try out.append(ctx.arena, run);
    }
    if (with_caret) {
        try out.append(ctx.arena, .{
            .text = ctx.options.caret.glyph(),
            .font_size = base.size * 0.9,
            .color = theme.caret,
            .family = .sans,
        });
    }
    return out.toOwnedSlice(ctx.arena);
}

pub fn richNode(ctx: Ctx, spans: []const ast.Span, base: Base, with_caret: bool) !ui.Node {
    if (spans.len == 0 and !with_caret) return ui.space(.{ .height = 1 });
    const runs = try toRich(ctx, spans, base, with_caret);
    return ui.richText(.{
        .spans = runs,
        .line_height = ctx.options.theme.line_height,
        .on_link = ctx.link_action,
    });
}

/// Body text with the context's colour and size.
pub fn body(ctx: Ctx, spans: []const ast.Span, with_caret: bool) !ui.Node {
    return richNode(ctx, spans, .{ .size = ctx.size, .color = ctx.fg, .family = ctx.family() }, with_caret);
}
