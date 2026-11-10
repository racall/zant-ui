//! JSX elements, attributes and children. Tags are read with the lexer's
//! JSX tag mode (so `>` never merges into `>=`), text with its raw mode, and
//! `{expressions}` with the normal expression parser.
const std = @import("std");
const ast = @import("ast.zig");
const parser = @import("parser.zig");
const expr_parser = @import("expr_parser.zig");
const jsx_text = @import("jsx_text.zig");
const Parser = parser.Parser;
const Error = parser.Error;

/// Parses an element whose `<` was just read. Leaves the lexer right
/// after the element's final `>`; `cur` is stale.
pub fn parseJsxAfterLt(self: *Parser, loc: u32) Error!*ast.Jsx {
    var t = self.lx.jsxTagToken();
    var name: []const u8 = "";
    var attrs: std.ArrayList(ast.Attr) = .empty;
    if (t.is(self.src, ">")) {
        return parseJsxChildren(self, loc, name, attrs.items); // fragment
    }
    if (t.kind != .identifier) return self.fail(t.start, "expected a tag name after `<`", .{});
    name = t.text(self.src);
    while (true) {
        t = self.lx.jsxTagToken();
        if (t.is(self.src, ".")) return self.fail(t.start, "member tag names (`<a.b>`) are not supported", .{});
        if (t.kind == .identifier) {
            const attr_name: ast.Name = .{ .name = t.text(self.src), .loc = t.start };
            for (attrs.items) |a| {
                if (std.mem.eql(u8, a.name.name, attr_name.name)) return self.fail(t.start, "duplicate attribute `{s}`", .{attr_name.name});
            }
            const before = self.lx.pos;
            const eq = self.lx.jsxTagToken();
            if (!eq.is(self.src, "=")) {
                self.lx.pos = before;
                try attrs.append(self.arena, .{ .name = attr_name, .value = null });
                continue;
            }
            const v = self.lx.jsxTagToken();
            if (v.kind == .string) {
                const raw = v.text(self.src);
                const value = try jsx_text.decodeEntities(self.arena, raw[1 .. raw.len - 1]);
                try attrs.append(self.arena, .{ .name = attr_name, .value = try self.expr(v.start, .{ .string = value }) });
            } else if (v.is(self.src, "{")) {
                self.advance();
                if (self.isP("}")) return self.fail(self.cur.start, "attribute `{s}` has an empty expression", .{attr_name.name});
                const value = try expr_parser.parseExpr(self);
                if (!self.isP("}")) return self.unexpected("}");
                try attrs.append(self.arena, .{ .name = attr_name, .value = value });
            } else if (v.is(self.src, "<")) {
                return self.fail(v.start, "wrap element attribute values in braces: `{s}={{<... />}}`", .{attr_name.name});
            } else {
                return self.fail(v.start, "expected a string or `{{...}}` for attribute `{s}`", .{attr_name.name});
            }
        } else if (t.is(self.src, "{")) {
            return self.fail(t.start, "spread attributes (`{{...props}}`) are not supported", .{});
        } else if (t.is(self.src, "/")) {
            const gt = self.lx.jsxTagToken();
            if (!gt.is(self.src, ">")) return self.fail(gt.start, "expected `>` after `/` in <{s} />", .{name});
            return self.new(ast.Jsx, .{ .loc = loc, .name = name, .attrs = attrs.items, .children = &.{} });
        } else if (t.is(self.src, ">")) {
            return parseJsxChildren(self, loc, name, attrs.items);
        } else if (t.kind == .eof) {
            return self.fail(loc, "unterminated <{s}> tag", .{name});
        } else {
            return self.fail(t.start, "unexpected `{s}` in <{s}> tag", .{ t.text(self.src), name });
        }
    }
}

pub fn parseJsxChildren(self: *Parser, loc: u32, name: []const u8, attrs: []const ast.Attr) Error!*ast.Jsx {
    var children: std.ArrayList(ast.Child) = .empty;
    while (true) {
        const raw = self.lx.jsxText();
        if (std.mem.indexOfAny(u8, raw.text(self.src), ">}")) |i| {
            const c = raw.text(self.src)[i];
            const entity = if (c == '>') "&gt;" else "&rbrace;";
            return self.fail(raw.start + @as(u32, @intCast(i)), "unexpected `{c}` in JSX text; write `{{'{c}'}}` or `{s}`", .{ c, c, entity });
        }
        const decoded = try jsx_text.decodeEntities(self.arena, raw.text(self.src));
        if (try jsx_text.cleanJsxText(self.arena, decoded)) |value| {
            // point errors at the text itself, not the indentation before it
            const lead = std.mem.indexOfNone(u8, raw.text(self.src), " \t\r\n") orelse 0;
            try children.append(self.arena, .{ .text = .{ .value = value, .loc = raw.start + @as(u32, @intCast(lead)) } });
        }
        if (self.lx.pos >= self.src.len) {
            return if (name.len == 0)
                self.fail(loc, "unterminated fragment: missing `</>`", .{})
            else
                self.fail(loc, "unterminated <{s}>: missing `</{s}>`", .{ name, name });
        }
        const child_loc = self.lx.pos;
        if (self.src[self.lx.pos] == '{') {
            self.lx.pos += 1;
            self.advance();
            if (self.isP("}")) {
                try children.append(self.arena, .{ .expr = null });
                continue;
            }
            const e = try expr_parser.parseExpr(self);
            if (!self.isP("}")) return self.unexpected("}");
            try children.append(self.arena, .{ .expr = e });
            continue;
        }
        // `<`: a closing tag or a child element
        self.lx.pos += 1;
        const after_lt = self.lx.pos;
        const t = self.lx.jsxTagToken();
        if (t.is(self.src, "/")) {
            const close = self.lx.jsxTagToken();
            if (name.len == 0) {
                if (!close.is(self.src, ">")) return self.fail(close.start, "expected `</>` to close the fragment", .{});
            } else {
                if (close.kind != .identifier or !std.mem.eql(u8, close.text(self.src), name)) {
                    return self.fail(close.start, "expected `</{s}>` to close <{s}>", .{ name, name });
                }
                const gt = self.lx.jsxTagToken();
                if (!gt.is(self.src, ">")) return self.fail(gt.start, "expected `>` after `</{s}`", .{name});
            }
            return self.new(ast.Jsx, .{ .loc = loc, .name = name, .attrs = attrs, .children = children.items });
        }
        self.lx.pos = after_lt;
        const child = try parseJsxAfterLt(self, child_loc);
        try children.append(self.arena, .{ .element = child });
    }
}
