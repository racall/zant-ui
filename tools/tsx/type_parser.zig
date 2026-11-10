//! Type annotations of the TSX subset (`docs/tsx.md`): named types (dotted
//! too), `T[]`, `Array<T>`, unions, string literal types, `null` /
//! `undefined`, object types and function types. `interface` bodies are
//! object types.
//!
//! Like tsc, a `(` starts a function type when the matching `)` is followed
//! by `=>`, and a parenthesized type otherwise.
const std = @import("std");
const ast = @import("ast.zig");
const lexer = @import("lexer.zig");
const parser = @import("parser.zig");
const Parser = parser.Parser;
const Error = parser.Error;

/// `A | B | C` (a leading `|` is allowed, as in TypeScript).
pub fn parseType(p: *Parser) Error!ast.TypeRef {
    const loc = p.cur.start;
    _ = p.eatP("|");
    const first = try parsePostfix(p);
    if (!p.isP("|")) return first;
    var members: std.ArrayList(ast.TypeRef) = .empty;
    try members.append(p.arena, first);
    while (p.eatP("|")) try members.append(p.arena, try parsePostfix(p));
    return detailed(p, loc, .{ .union_ = members.items });
}

/// A primary type followed by any number of `[]`.
fn parsePostfix(p: *Parser) Error!ast.TypeRef {
    const loc = p.cur.start;
    var t = try parsePrimary(p);
    while (p.isP("[")) {
        p.advance();
        if (!p.isP("]")) return p.fail(p.cur.start, "indexed access types are not supported; write `T[]` for an array", .{});
        p.advance();
        t = try detailed(p, loc, .{ .array = t });
    }
    return t;
}

fn parsePrimary(p: *Parser) Error!ast.TypeRef {
    const loc = p.cur.start;
    switch (p.cur.kind) {
        .string => {
            const value = lexer.decodeString(p.arena, p.cur.text(p.src)) catch return p.fail(loc, "invalid escape in string", .{});
            p.advance();
            return detailed(p, loc, .{ .string_lit = value });
        },
        .number => return p.fail(loc, "number literal types are not supported; use a string literal union", .{}),
        .identifier => {},
        else => {
            if (p.isP("{")) return detailed(p, loc, .{ .object = try parseMembers(p) });
            if (p.isP("(")) return parseParenOrFunction(p);
            return p.unexpected("a type");
        },
    }
    const word = p.cur.text(p.src);
    if (std.mem.eql(u8, word, "null") or std.mem.eql(u8, word, "undefined")) {
        p.advance();
        return detailed(p, loc, .null_);
    }
    inline for (.{ "typeof", "keyof", "infer", "unique", "readonly" }) |kw| {
        if (std.mem.eql(u8, word, kw)) return p.fail(loc, "`{s}` types are not supported", .{kw});
    }
    // a name, dotted (`JSX.Element`)
    var end = p.cur.end;
    p.advance();
    while (p.isP(".")) {
        p.advance();
        if (p.cur.kind != .identifier) return p.unexpected("a type name");
        end = p.cur.end;
        p.advance();
    }
    const name = p.src[loc..end];
    if (!p.isP("<")) return .{ .name = name, .loc = loc };
    // generics: only Array<T> / ReadonlyArray<T>
    if (!std.mem.eql(u8, name, "Array") and !std.mem.eql(u8, name, "ReadonlyArray")) {
        return p.fail(loc, "generic type `{s}<…>` is not supported; only `Array<T>` is", .{name});
    }
    p.advance();
    const item = try parseType(p);
    if (p.isP(",")) return p.fail(p.cur.start, "`Array` takes one type argument", .{});
    if (!p.isP(">")) return p.unexpected(">");
    p.advance();
    return detailed(p, loc, .{ .array = item });
}

/// `{ id: number; title?: string }`: members end with `;`, `,` or a line
/// break.
pub fn parseMembers(p: *Parser) Error![]const ast.TypeField {
    try p.expectP("{");
    var fields: std.ArrayList(ast.TypeField) = .empty;
    while (!p.isP("}")) {
        if (p.cur.kind == .eof) return p.unexpected("}");
        if (p.isP("[")) return p.fail(p.cur.start, "index signatures are not supported; use an array of objects", .{});
        if (p.isKw("readonly")) {
            const s = p.save();
            p.advance();
            if (p.cur.kind != .identifier) p.restore(s);
        }
        const name: ast.Name = switch (p.cur.kind) {
            .identifier => .{ .name = p.cur.text(p.src), .loc = p.cur.start },
            .string => .{
                .name = lexer.decodeString(p.arena, p.cur.text(p.src)) catch return p.fail(p.cur.start, "invalid escape in string", .{}),
                .loc = p.cur.start,
            },
            else => return p.unexpected("a field name"),
        };
        p.advance();
        for (fields.items) |f| {
            if (std.mem.eql(u8, f.name.name, name.name)) return p.fail(name.loc, "duplicate field `{s}`", .{name.name});
        }
        const optional = p.eatP("?");
        if (p.isP("(")) return p.fail(p.cur.start, "methods are not supported in object types; write `{s}: (...) => T`", .{name.name});
        try p.expectP(":");
        const ty = try parseType(p);
        try fields.append(p.arena, .{ .name = name, .ty = ty, .optional = optional });
        if (p.eatP(";") or p.eatP(",")) continue;
        if (p.isP("}") or p.cur.nl_before) continue;
        return p.unexpected(";");
    }
    p.advance();
    return fields.items;
}

/// `(A | B)` or `(id: number) => void`.
fn parseParenOrFunction(p: *Parser) Error!ast.TypeRef {
    const loc = p.cur.start;
    if (!followedByArrow(p)) {
        p.advance();
        const inner = try parseType(p);
        try p.expectP(")");
        return inner;
    }
    const params = try p.parseParams();
    try p.expectP("=>");
    const ret = try parseType(p);
    return detailed(p, loc, .{ .func = .{ .params = params, .ret = ret } });
}

/// Whether the `(` at `cur` is matched by a `)` followed by `=>` (nothing
/// consumed).
pub fn followedByArrow(p: *Parser) bool {
    const s = p.save();
    defer p.restore(s);
    var depth: usize = 0;
    while (p.cur.kind != .eof) {
        if (p.isP("(") or p.isP("[") or p.isP("{")) {
            depth += 1;
        } else if (p.isP(")") or p.isP("]") or p.isP("}")) {
            depth -= 1;
            if (depth == 0) {
                p.advance();
                return p.isP("=>");
            }
        } else if (p.cur.kind == .template) {
            return false; // never inside parameter lists or types
        }
        p.advance();
    }
    return false;
}

fn detailed(p: *Parser, loc: u32, detail: ast.TypeDetail) Error!ast.TypeRef {
    const d = try p.arena.create(ast.TypeDetail);
    d.* = detail;
    return .{ .name = "", .loc = loc, .detail = d };
}
