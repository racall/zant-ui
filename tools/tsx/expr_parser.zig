//! Expressions of the TSX subset: precedence climbing for binary operators,
//! arrow functions and generic calls found by backtracking (`Parser.save`),
//! template literals, object literals and the JSX entry point.
const std = @import("std");
const ast = @import("ast.zig");
const lexer = @import("lexer.zig");
const parser = @import("parser.zig");
const jsx_parser = @import("jsx_parser.zig");
const type_parser = @import("type_parser.zig");
const Parser = parser.Parser;
const Error = parser.Error;

pub fn parseExpr(self: *Parser) Error!*ast.Expr {
    return parseAssign(self);
}

pub fn parseAssign(self: *Parser) Error!*ast.Expr {
    if (try tryArrow(self)) |arrow| return arrow;
    const lhs = try parseCond(self);
    inline for (.{ "=", "+=", "-=", "*=", "/=", "%=" }) |op| {
        if (self.isP(op)) {
            self.advance();
            const value = try parseAssign(self);
            return self.expr(lhs.loc, .{ .assign = .{ .op = op, .target = lhs, .value = value } });
        }
    }
    return lhs;
}

/// `x => ...`, `() => ...`, `(a: T, b) => ...`, `(t: Todo): string => ...`;
/// null when the tokens are not an arrow function (nothing consumed).
/// Arrows are recognised before parsing them (tsc's approach), so errors
/// inside their parameters are reported as such.
pub fn tryArrow(self: *Parser) Error!?*ast.Expr {
    const loc = self.cur.start;
    if (self.cur.kind == .identifier) {
        const s = self.save();
        const name: ast.Name = .{ .name = self.text(), .loc = self.cur.start };
        self.advance();
        if (!self.isP("=>") or self.cur.nl_before) {
            self.restore(s);
            return null;
        }
        const params = try self.arena.alloc(ast.Param, 1);
        params[0] = .{ .name = name, .ty = null };
        const func = try arrowBody(self, loc, params, null);
        return try self.expr(loc, .{ .arrow = func });
    }
    if (!self.isP("(")) return null;
    if (!definitelyArrow(self) and !type_parser.followedByArrow(self) and !returnTypedArrow(self)) return null;
    const params = try self.parseParams();
    const ret = try self.parseReturnType();
    if (!self.isP("=>")) return self.unexpected("=>");
    const func = try arrowBody(self, loc, params, ret);
    return try self.expr(loc, .{ .arrow = func });
}

/// `(...): Type =>`: the matching `)` is followed by a return type and `=>`
/// (tried speculatively; nothing consumed, no error kept).
fn returnTypedArrow(self: *Parser) bool {
    const s = self.save();
    defer self.restore(s);
    const saved_diag = self.diag;
    defer self.diag = saved_diag;
    var depth: usize = 0;
    while (self.cur.kind != .eof) {
        if (self.isP("(") or self.isP("[") or self.isP("{")) {
            depth += 1;
        } else if (self.isP(")") or self.isP("]") or self.isP("}")) {
            depth -= 1;
            if (depth == 0) break;
        }
        self.advance();
    }
    if (!self.isP(")")) return false;
    self.advance();
    if (!self.isP(":")) return false;
    self.advance();
    _ = self.parseType() catch return false;
    return self.isP("=>");
}

/// tsc's quick check (`isParenthesizedArrowFunctionExpression`): `() =>`,
/// `():` and `(a:` can only start an arrow function. Other shapes (`(a)`,
/// `(a,`) are tried speculatively by `tryArrow`.
fn definitelyArrow(self: *Parser) bool {
    const s = self.save();
    defer self.restore(s);
    self.advance(); // (
    if (self.isP(")")) {
        self.advance();
        return self.isP("=>") or self.isP(":");
    }
    if (self.cur.kind != .identifier) return false;
    self.advance();
    return self.isP(":");
}

pub fn arrowBody(self: *Parser, loc: u32, params: []const ast.Param, ret: ?ast.TypeRef) Error!*ast.Function {
    self.advance(); // =>
    const body: ast.Body = if (self.isP("{"))
        .{ .block = try self.parseBlock() }
    else
        .{ .expr = try parseAssign(self) };
    return self.new(ast.Function, .{ .loc = loc, .name = "", .params = params, .ret = ret, .body = body });
}

pub fn parseCond(self: *Parser) Error!*ast.Expr {
    const c = try parseBinary(self, 0);
    if (!self.isP("?")) return c;
    self.advance();
    const then = try parseAssign(self);
    try self.expectP(":");
    const else_ = try parseAssign(self);
    return self.expr(c.loc, .{ .cond = .{ .cond = c, .then = then, .else_ = else_ } });
}

pub fn binaryPrec(self: *Parser) ?u8 {
    self.rescanGreater();
    if (self.cur.kind != .punct) return null;
    const t = self.text();
    const table = .{
        .{ "??", 1 }, .{ "||", 2 },  .{ "&&", 3 },  .{ "|", 4 },   .{ "^", 5 },  .{ "&", 6 },
        .{ "==", 7 }, .{ "!=", 7 },  .{ "===", 7 }, .{ "!==", 7 }, .{ "<", 8 },  .{ ">", 8 },
        .{ "<=", 8 }, .{ ">=", 8 },  .{ "+", 9 },   .{ "-", 9 },   .{ "*", 10 }, .{ "/", 10 },
        .{ "%", 10 }, .{ "**", 11 },
    };
    inline for (table) |entry| {
        if (std.mem.eql(u8, t, entry[0])) return entry[1];
    }
    return null;
}

pub fn parseBinary(self: *Parser, min_prec: u8) Error!*ast.Expr {
    var lhs = try parseUnary(self);
    while (binaryPrec(self)) |prec| {
        if (prec < min_prec) break;
        const op = self.text();
        self.advance();
        // `**` is right-associative, the rest left-associative
        const rhs = try parseBinary(self, if (std.mem.eql(u8, op, "**")) prec else prec + 1);
        lhs = try self.expr(lhs.loc, .{ .binary = .{ .op = op, .lhs = lhs, .rhs = rhs } });
    }
    return lhs;
}

pub fn parseUnary(self: *Parser) Error!*ast.Expr {
    const loc = self.cur.start;
    inline for (.{ "!", "-", "+" }) |op| {
        if (self.isP(op)) {
            self.advance();
            const operand = try parseUnary(self);
            return self.expr(loc, .{ .unary = .{ .op = op, .operand = operand } });
        }
    }
    inline for (.{ "++", "--" }) |op| {
        if (self.isP(op)) {
            self.advance();
            const target = try parseUnary(self);
            return self.expr(loc, .{ .update = .{ .op = op, .target = target } });
        }
    }
    inline for (.{ "typeof", "void", "delete", "await", "new" }) |kw| {
        if (self.isKw(kw)) return self.fail(loc, "`{s}` is not supported", .{kw});
    }
    return parsePostfix(self);
}

pub fn parsePostfix(self: *Parser) Error!*ast.Expr {
    var e = try parsePrimary(self);
    while (true) {
        if (self.isP(".")) {
            self.advance();
            const name = try self.expectIdent("a property name");
            e = try self.expr(e.loc, .{ .member = .{ .object = e, .name = name } });
        } else if (self.isP("(")) {
            const args = try parseArgs(self);
            e = try self.expr(e.loc, .{ .call = .{ .callee = e, .type_args = &.{}, .args = args } });
        } else if (self.isP("<") and e.data == .ident) {
            const type_args = tryTypeArgs(self) orelse break;
            const args = try parseArgs(self);
            e = try self.expr(e.loc, .{ .call = .{ .callee = e, .type_args = type_args, .args = args } });
        } else if (self.isP("?.")) {
            self.advance();
            if (self.isP("[")) {
                e = try parseIndex(self, e, true);
            } else if (self.isP("(")) {
                return self.fail(self.cur.start, "optional calls (`f?.()`) are not supported", .{});
            } else {
                const name = try self.expectIdent("a property name");
                e = try self.expr(e.loc, .{ .member = .{ .object = e, .name = name, .optional = true } });
            }
        } else if (self.isP("[")) {
            e = try parseIndex(self, e, false);
        } else if (self.isP("!") and !self.cur.nl_before) {
            // `x!`: TypeScript's non-null assertion
            self.advance();
            e = try self.expr(e.loc, .{ .non_null = e });
        } else if ((self.isP("++") or self.isP("--")) and !self.cur.nl_before) {
            const op = self.text();
            self.advance();
            e = try self.expr(e.loc, .{ .update = .{ .op = op, .target = e } });
        } else if (self.isKw("as") and !self.cur.nl_before) {
            self.advance();
            if (self.isKw("const")) return self.fail(self.cur.start, "`as const` is not supported", .{});
            const ty = try self.parseType();
            e = try self.expr(e.loc, .{ .as = .{ .expr = e, .ty = ty } });
        } else break;
    }
    return e;
}

/// `<Todo[]>(` after a callee: a type argument list, or null (nothing
/// consumed, no error kept) when `<` is a comparison.
pub fn tryTypeArgs(self: *Parser) ?[]const ast.TypeRef {
    const s = self.save();
    const saved_diag = self.diag;
    self.advance(); // <
    var list: std.ArrayList(ast.TypeRef) = .empty;
    while (true) {
        const ty = self.parseType() catch break;
        list.append(self.arena, ty) catch break;
        if (!self.eatP(",")) break;
    }
    if (list.items.len > 0 and self.isP(">")) {
        self.advance();
        if (self.isP("(")) return list.items;
    }
    self.restore(s);
    self.diag = saved_diag;
    return null;
}

/// `a[i]` (`a?.[i]` when `optional`); `cur` is the `[`.
fn parseIndex(self: *Parser, object: *ast.Expr, optional: bool) Error!*ast.Expr {
    self.advance(); // [
    const index = try parseExpr(self);
    try self.expectP("]");
    return self.expr(object.loc, .{ .index = .{ .object = object, .index = index, .optional = optional } });
}

pub fn parseArgs(self: *Parser) Error![]const *ast.Expr {
    try self.expectP("(");
    var args: std.ArrayList(*ast.Expr) = .empty;
    while (!self.isP(")")) {
        if (self.isP("...")) return self.fail(self.cur.start, "spread arguments are not supported", .{});
        try args.append(self.arena, try parseAssign(self));
        if (!self.eatP(",")) break;
    }
    try self.expectP(")");
    return args.items;
}

pub fn parsePrimary(self: *Parser) Error!*ast.Expr {
    const loc = self.cur.start;
    switch (self.cur.kind) {
        .number => {
            const raw = self.text();
            self.advance();
            return self.expr(loc, .{ .number = raw });
        },
        .string => {
            const value = lexer.decodeString(self.arena, self.text()) catch return self.fail(loc, "invalid escape in string", .{});
            self.advance();
            return self.expr(loc, .{ .string = value });
        },
        .template => return parseTemplate(self, loc),
        .identifier => {
            const name = self.text();
            if (std.mem.eql(u8, name, "true") or std.mem.eql(u8, name, "false")) {
                self.advance();
                return self.expr(loc, .{ .boolean = name[0] == 't' });
            }
            if (std.mem.eql(u8, name, "null") or std.mem.eql(u8, name, "undefined")) {
                self.advance();
                return self.expr(loc, .null_lit);
            }
            if (std.mem.eql(u8, name, "function")) return self.fail(loc, "use an arrow function (`() => ...`) inside expressions", .{});
            if (std.mem.eql(u8, name, "this")) return self.fail(loc, "`this` is not supported; components are functions", .{});
            self.advance();
            return self.expr(loc, .{ .ident = name });
        },
        .punct => {
            if (self.isP("(")) {
                self.advance();
                const inner = try parseExpr(self);
                try self.expectP(")");
                return inner;
            }
            if (self.isP("<")) {
                const el = try jsx_parser.parseJsxAfterLt(self, loc);
                self.advance();
                return self.expr(loc, .{ .jsx = el });
            }
            if (self.isP("{")) return parseObject(self, loc);
            if (self.isP("[")) return parseArray(self, loc);
            return self.fail(loc, "unexpected `{s}`", .{self.text()});
        },
        .eof => return self.fail(loc, "expected an expression but the file ended", .{}),
        .invalid => return self.fail(loc, "unexpected character `{s}`", .{self.text()}),
    }
}

/// `[a, b, ...rest]` (trailing comma allowed, holes are not).
fn parseArray(self: *Parser, loc: u32) Error!*ast.Expr {
    self.advance(); // [
    var items: std.ArrayList(*ast.Expr) = .empty;
    while (!self.isP("]")) {
        if (self.isP(",")) return self.fail(self.cur.start, "array holes are not supported", .{});
        try items.append(self.arena, try spreadOrExpr(self));
        if (!self.eatP(",")) break;
    }
    try self.expectP("]");
    return self.expr(loc, .{ .array = items.items });
}

/// `...x` or an expression (array literal items).
fn spreadOrExpr(self: *Parser) Error!*ast.Expr {
    if (!self.isP("...")) return parseAssign(self);
    const loc = self.cur.start;
    self.advance();
    const inner = try parseAssign(self);
    return self.expr(loc, .{ .spread = inner });
}

pub fn parseObject(self: *Parser, loc: u32) Error!*ast.Expr {
    self.advance(); // {
    var props: std.ArrayList(ast.Property) = .empty;
    while (!self.isP("}")) {
        if (self.isP("...")) {
            const loc_spread = self.cur.start;
            self.advance();
            const inner = try parseAssign(self);
            try props.append(self.arena, .{ .key = .{ .name = "", .loc = loc_spread }, .value = inner, .spread = true });
            if (!self.eatP(",")) break;
            continue;
        }
        if (self.isP("[")) return self.fail(self.cur.start, "computed keys are not supported", .{});
        const key: ast.Name = switch (self.cur.kind) {
            .identifier => .{ .name = self.text(), .loc = self.cur.start },
            .string => .{
                .name = lexer.decodeString(self.arena, self.text()) catch return self.fail(self.cur.start, "invalid escape in string", .{}),
                .loc = self.cur.start,
            },
            else => return self.unexpected("a property name"),
        };
        self.advance();
        const value = if (self.eatP(":"))
            try parseAssign(self)
        else if (self.isP(",") or self.isP("}"))
            try self.expr(key.loc, .{ .ident = key.name }) // shorthand `{ gap }`
        else
            return self.unexpected(":");
        try props.append(self.arena, .{ .key = key, .value = value });
        if (!self.eatP(",")) break;
    }
    try self.expectP("}");
    return self.expr(loc, .{ .object = props.items });
}

pub fn parseTemplate(self: *Parser, loc: u32) Error!*ast.Expr {
    var quasis: std.ArrayList([]const u8) = .empty;
    var exprs: std.ArrayList(*ast.Expr) = .empty;
    while (true) {
        const part = self.lx.templatePart();
        const cooked = lexer.cook(self.arena, part.raw) catch return self.fail(part.start, "invalid escape in template literal", .{});
        try quasis.append(self.arena, cooked);
        switch (part.end) {
            .eof => return self.fail(loc, "unterminated template literal", .{}),
            .backtick => break,
            .substitution => {
                self.advance();
                try exprs.append(self.arena, try parseExpr(self));
                if (!self.isP("}")) return self.unexpected("}");
                // the lexer sits right after `}`: continue the template there
            },
        }
    }
    self.advance();
    return self.expr(loc, .{ .template = .{ .quasis = quasis.items, .exprs = exprs.items } });
}
