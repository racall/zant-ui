//! Which elements may contain which (docs/tsx.md, "嵌套规则"). Every tag has
//! a content model, like HTML's:
//!
//! - containers (`View`, `Row`, `Card`, `Clickable`, …): any elements;
//!   text must be inside `<Text>`;
//! - `<Text>`: text, `{values}`, nested `<Text>` spans and inline `<Link>`s
//!   (compiled to `ui.richText`, `rich_text.zig`);
//! - `Heading` / `Title` / `Paragraph`: text and `{values}`;
//! - labels (`Button`, `Link`, `Checkbox`, `Switch`, `Tag`): text and
//!   `{values}`; a `Button` also takes one `<Icon>`;
//! - one-child slots (`Tooltip`, `Modal`): exactly one element;
//! - leaves (`Input`, `Image`, `Divider`, …): no children.
//!
//! Elements that scroll themselves (`List`) can't go inside a scrolling
//! container, sibling keys must differ, overlays (`Modal`, `Message`) render
//! at the window root wherever they are written (`ui.portal`), and a click
//! goes to the innermost clickable element only (no bubbling), so buttons
//! inside a `Clickable` card work.
const std = @import("std");
const ast = @import("../ast.zig");
const tags = @import("../tags.zig");
const core = @import("codegen.zig");
const text = @import("zig_text.zig");
const Generator = core.Generator;
const Error = core.Error;

/// The phrase used in errors for what `tag` holds.
fn holds(tag: *const tags.Tag) []const u8 {
    if (text.eq(tag.name, "Text")) return "text, {values}, <Text> spans and <Link>";
    if (text.eq(tag.name, "Button")) return "its label (text and {values}) and one <Icon>";
    return switch (tag.content) {
        .children => "elements",
        .text_arg => "text and {values}; use <Text> for spans",
        .text_field => "its label as text and {values}, written directly inside it",
        .child => "exactly one element",
        .none => "no children",
    };
}

/// `<child>` written inside `tag` where its content model doesn't allow it.
pub fn misplaced(g: *Generator, tag: *const tags.Tag, child: *const ast.Jsx) Error {
    const name = if (child.name.len == 0) "a fragment" else child.name;
    if (tag.content == .none) return core.fail(g, child.loc, "<{s}> has no children", .{tag.name});
    return core.fail(g, child.loc, "<{s}> can't be inside <{s}>; it holds {s}", .{ name, tag.name, holds(tag) });
}

/// Text written directly inside an element that holds elements.
pub fn looseText(g: *Generator, owner: []const u8, loc: ast.Loc) Error {
    return core.fail(g, loc, "text inside <{s}> must be wrapped in <Text>", .{owner});
}

/// Overlays are moved to the window root (`ui.portal`).
pub fn overlay(name: []const u8) bool {
    return text.eq(name, "Modal") or text.eq(name, "Message");
}

/// `scroll`, `scroll={true}`, `overflow="scroll"` or `style={{ overflow: "scroll" }}`.
pub fn scrolls(j: *const ast.Jsx) bool {
    for (j.attrs) |a| {
        if (text.eq(a.name.name, "scroll")) {
            const v = a.value orelse return true;
            return v.data == .boolean and v.data.boolean;
        }
        if (text.eq(a.name.name, "overflow")) if (a.value) |v| {
            return v.data == .string and text.eq(v.data.string, "scroll");
        };
        if (text.eq(a.name.name, "style")) if (a.value) |v| if (v.data == .object) {
            for (v.data.object) |p| if (!p.spread and text.eq(p.key.name, "overflow")) {
                return p.value.data == .string and text.eq(p.value.data.string, "scroll");
            };
        };
    }
    return false;
}

/// An element that scrolls itself (`<List>`) written inside a scrolling
/// container: it would get unbounded height and never scroll.
pub fn notInScroll(g: *Generator, j: *const ast.Jsx) Error!void {
    if (g.scroll_depth == 0) return;
    return core.fail(
        g,
        j.loc,
        "<{s}> scrolls by itself; put it in a container with a bounded height instead of a scrolling one",
        .{j.name},
    );
}

/// Two literal sibling keys (`key="a"`) that are equal.
pub fn uniqueKeys(g: *Generator, children: []const ast.Child) Error!void {
    var seen: std.ArrayList([]const u8) = .empty;
    for (children) |child| {
        const el = switch (child) {
            .element => |el| el,
            else => continue,
        };
        if (el.name.len == 0) {
            try uniqueKeys(g, el.children);
            continue;
        }
        const key = literalKey(el) orelse continue;
        for (seen.items) |previous| if (text.eq(previous, key.value)) {
            return core.fail(g, key.loc, "two siblings have the key \"{s}\"; keys must differ", .{key.value});
        };
        try seen.append(g.a, key.value);
    }
}

fn literalKey(el: *const ast.Jsx) ?struct { value: []const u8, loc: ast.Loc } {
    for (el.attrs) |a| if (text.eq(a.name.name, "key")) {
        const v = a.value orelse return null;
        return switch (v.data) {
            .string => |s| .{ .value = s, .loc = v.loc },
            .number => |n| .{ .value = n, .loc = v.loc },
            else => null,
        };
    };
    return null;
}

/// `cond && "text"` in text content: renders "" when false (React renders
/// nothing for `false`). Other expressions are returned unchanged.
pub fn textExpression(g: *Generator, e: *ast.Expr) Error!*ast.Expr {
    if (e.data != .binary or !text.eq(e.data.binary.op, "&&")) return e;
    const empty = try g.a.create(ast.Expr);
    empty.* = .{ .loc = e.loc, .data = .{ .string = "" } };
    const cond = try g.a.create(ast.Expr);
    cond.* = .{ .loc = e.loc, .data = .{ .cond = .{
        .cond = e.data.binary.lhs,
        .then = e.data.binary.rhs,
        .else_ = empty,
    } } };
    return cond;
}
