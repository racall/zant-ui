//! `<Text>` with nested `<Text>` spans and inline `<Link>`s, React Native
//! style, compiled to `ui.richText`:
//!
//!     <Text size={16}>Read <Text weight="bold">this</Text> or <Link onClick={open}>the docs</Link></Text>
//!
//! Each run of text becomes a `tsx.text.span(...)` whose style is its own
//! props over its parents'; a `<Link>` span gets an index into the link
//! actions passed as `on_link` (`tsx.text.links`).
const std = @import("std");
const ast = @import("../ast.zig");
const tags = @import("../tags.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const events = @import("events.zig");
const jsx = @import("jsx.zig");
const nesting = @import("nesting.zig");
const text = @import("zig_text.zig");
const Generator = core.Generator;
const Context = scope.Context;
const Value = types.Value;
const Error = core.Error;

/// Whether a `<Text>` has element children (and so is rich text).
pub fn hasElements(j: *const ast.Jsx) bool {
    for (j.children) |child| if (child == .element) return true;
    return false;
}

/// Span style fields (`tsx.text.SpanStyle`) as Zig field initialisers;
/// a child's entries replace its parent's.
const Style = struct {
    names: []const []const u8 = &.{},
    codes: []const []const u8 = &.{},

    fn with(style: Style, g: *Generator, name: []const u8, code: []const u8) Error!Style {
        var names: std.ArrayList([]const u8) = .empty;
        var codes: std.ArrayList([]const u8) = .empty;
        for (style.names, style.codes) |n, c| if (!text.eq(n, name)) {
            try names.append(g.a, n);
            try codes.append(g.a, c);
        };
        try names.append(g.a, name);
        try codes.append(g.a, code);
        return .{ .names = names.items, .codes = codes.items };
    }

    fn emit(style: Style, g: *Generator) Error![]const u8 {
        var out: std.ArrayList(u8) = .empty;
        for (style.names, style.codes) |n, c| try out.appendSlice(g.a, try text.fmt(g, ".{s} = {s}, ", .{ n, c }));
        return text.fmt(g, ".{{ {s}}}", .{out.items});
    }
};

const Builder = struct {
    g: *Generator,
    c: *Context,
    spans: std.ArrayList([]const u8) = .empty,
    links: std.ArrayList([]const u8) = .empty,
    run: text.Format,

    fn flush(b: *Builder, style: Style) Error!void {
        if (b.run.literal.items.len == 0 and b.run.args.items.len == 0) return;
        try b.spans.append(b.g.a, try text.fmt(b.g, "tsx.text.span({s}, {s})", .{
            try b.run.finish(false), try style.emit(b.g),
        }));
        b.run = .{ .g = b.g };
    }

    fn children(b: *Builder, owner: *const tags.Tag, cs: []const ast.Child, style: Style) Error!void {
        for (cs) |child| switch (child) {
            .text => |t| try b.run.text(t.value),
            .expr => |x| if (x) |e| try b.run.expression(b.c, try nesting.textExpression(b.g, e)),
            .element => |el| {
                try b.flush(style);
                if (text.eq(el.name, "Text")) {
                    try b.children(tags.find("Text").?, el.children, try spanStyle(b, el, style, false));
                } else if (text.eq(el.name, "Link")) {
                    try b.children(tags.find("Link").?, el.children, try b.link(el, style));
                } else return nesting.misplaced(b.g, owner, el);
                try b.flush(style);
            },
        };
        try b.flush(style);
    }

    /// An inline `<Link onClick={…}>`: its index in the link actions.
    fn link(b: *Builder, el: *const ast.Jsx, style: Style) Error!Style {
        var action: []const u8 = "null";
        for (el.attrs) |a| {
            if (text.eq(a.name.name, "onClick") or text.eq(a.name.name, "onPress")) {
                const v = a.value orelse return core.fail(b.g, a.name.loc, "onClick needs a function", .{});
                action = try events.eventValue(b.g, b.c, v, .action);
            } else if (!text.eq(a.name.name, "key")) {
                const message = "an inline <Link> takes onClick only; style it with <Text> around it";
                return core.fail(b.g, a.name.loc, message, .{});
            }
        }
        const index = b.links.items.len;
        try b.links.append(b.g.a, action);
        return style.with(b.g, "link", try text.fmt(b.g, "{d}", .{index}));
    }
};

/// The style of a nested (or the root) `<Text>`: its props over `parent`.
fn spanStyle(b: *Builder, el: *const ast.Jsx, parent: Style, root: bool) Error!Style {
    const tag = tags.find("Text").?;
    var style = parent;
    for (el.attrs) |a| {
        if (a.spread) return core.fail(b.g, a.name.loc, "text with spans takes its props one by one (no spread)", .{});
        const name = a.name.name;
        if (text.eq(name, "key") or (root and text.eq(name, "lineHeight"))) continue;
        if (try extra(b, a)) |entry| {
            style = try style.with(b.g, entry[0], entry[1]);
            continue;
        }
        const prop = tags.findProp(tag, name) orelse return core.fail(
            b.g,
            a.name.loc,
            "unknown prop `{s}` on a <Text> span; spans take size, weight, color, family, italic, underline, " ++
                "strike and background",
            .{name},
        );
        const v = a.value orelse return core.fail(b.g, a.name.loc, "`{s}` needs a value", .{name});
        const field = prop.field;
        const code = if (text.eq(field, "size"))
            (try expr.emit(b.g, b.c, v, .f32, false)).code
        else if (text.eq(field, "color"))
            try text.color(b.g, b.c, v)
        else if (text.eq(field, "weight") or text.eq(field, "family"))
            try jsx.enumValue(b.g, b.c, prop, v)
        else
            return core.fail(b.g, a.name.loc, "`{s}` can't be used on text with spans", .{name});
        style = try style.with(b.g, field, code);
    }
    return style;
}

/// Span-only props: `italic`, `underline`, `strike` (booleans), `background`.
fn extra(b: *Builder, a: ast.Attr) Error!?[2][]const u8 {
    const name = a.name.name;
    if (text.eq(name, "background")) {
        const v = a.value orelse return core.fail(b.g, a.name.loc, "background needs a color", .{});
        return .{ "background", try text.color(b.g, b.c, v) };
    }
    if (!(text.eq(name, "italic") or text.eq(name, "underline") or text.eq(name, "strike"))) return null;
    const code = if (a.value) |v| (try expr.emit(b.g, b.c, v, .boolean, false)).code else "true";
    return .{ name, code };
}

/// The rich text node of a `<Text>` with element children.
pub fn emit(g: *Generator, c: *Context, j: *ast.Jsx) Error!Value {
    var b = Builder{ .g = g, .c = c, .run = .{ .g = g } };
    const base = try spanStyle(&b, j, .{}, true);
    try b.children(tags.find("Text").?, j.children, base);
    var fields: std.ArrayList(u8) = .empty;
    // natural line height, like `ui.text` (richText's own default is for Markdown)
    var line_height: []const u8 = "0";
    for (j.attrs) |a| {
        const v = a.value orelse continue;
        if (text.eq(a.name.name, "key")) {
            const key = try expr.emit(g, c, v, .string, false);
            try fields.appendSlice(g.a, try text.fmt(g, ".key = {s},\n", .{key.code}));
        } else if (text.eq(a.name.name, "lineHeight")) {
            line_height = (try expr.emit(g, c, v, .f32, false)).code;
        }
    }
    try fields.appendSlice(g.a, try text.fmt(g, ".line_height = {s},\n", .{line_height}));
    try fields.appendSlice(g.a, try text.fmt(g, ".spans = &[_]zant.node.RichSpan{{\n{s}\n}},\n", .{
        try std.mem.join(g.a, ",\n", b.spans.items),
    }));
    if (b.links.items.len > 0) {
        try fields.appendSlice(g.a, try text.fmt(g, ".on_link = tsx.text.links(&.{{ {s} }}),\n", .{
            try std.mem.join(g.a, ", ", b.links.items),
        }));
    }
    return .{ .ty = .node, .code = try text.fmt(g, "ui.richText(.{{\n{s}}})", .{fields.items}) };
}
