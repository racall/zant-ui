//! Expressions of the TSX subset: precedence climbing for binary operators,
//! arrow functions and generic calls found by backtracking (`Parser.save`),
//! template literals, object literals and the JSX entry point.
const std = @import("std");
const ast = @import("ast.zig");
const lexer = @import("lexer.zig");
const parser = @import("parser.zig");
const jsx_parser = @import("jsx_parser.zig");
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

/// `x => ...`, `() => ...`, `(a: T, b) => ...`; null when the tokens
/// are not an arrow function (nothing consumed).
pub fn tryArrow(self: *Parser) Error!?*ast.Expr {
    const loc = self.cur.start;
    const s = self.save();
    var params: std.ArrayList(ast.Param) = .empty;
    if (self.cur.kind == .identifier) {
        const name: ast.Name = .{ .name = self.text(), .loc = self.cur.start };
        self.advance();
        if (!self.isP("=>")) {
            self.restore(s);
            return null;
        }
        try params.append(self.arena, .{ .name = name, .ty = null });
    } else if (self.isP("(")) {
        if (definitelyArrow(self)) {
            // parse it as an arrow with real errors (`(v: string[]) =>`)
            const typed_params = try self.parseParams();
            const typed_ret = try self.parseReturnType();
            if (!self.isP("=>")) return self.unexpected("=>");
            const typed = try arrowBody(self, loc, typed_params, typed_ret);
            return try self.expr(loc, .{ .arrow = typed });
        }
        self.advance();
        while (!self.isP(")")) {
            if (self.cur.kind != .identifier) {
                self.restore(s);
                return null;
            }
            const name: ast.Name = .{ .name = self.text(), .loc = self.cur.start };
            self.advance();
            var ty: ?ast.TypeRef = null;
            if (self.isP(":")) {
                self.advance();
                if (self.cur.kind != .identifier) {
                    self.restore(s);
                    return null;
                }
                ty = .{ .name = self.text(), .loc = self.cur.start };
                self.advance();
            }
            try params.append(self.arena, .{ .name = name, .ty = ty });
            if (!self.isP(",")) break;
            self.advance();
        }
        if (!self.isP(")")) {
            self.restore(s);
            return null;
        }
        self.advance();
        var ret: ?ast.TypeRef = null;
        if (self.isP(":")) {
            self.advance();
            if (self.cur.kind != .identifier) {
                self.restore(s);
                return null;
            }
            // a dotted name (`JSX.Element`) spans from the first part to the last
            const start = self.cur.start;
            var end = self.cur.end;
            self.advance();
            while (self.isP(".")) {
                self.advance();
                if (self.cur.kind != .identifier) {
                    self.restore(s);
                    return null;
                }
                end = self.cur.end;
                self.advance();
            }
            ret = .{ .name = self.src[start..end], .loc = start };
        }
        if (!self.isP("=>")) {
            self.restore(s);
            return null;
        }
        const func = try arrowBody(self, loc, params.items, ret);
        return try self.expr(loc, .{ .arrow = func });
    } else return null;
    const func = try arrowBody(self, loc, params.items, null);
    return try self.expr(loc, .{ .arrow = func });
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
            return self.fail(self.cur.start, "optional chaining is not supported yet", .{});
        } else if (self.isP("[")) {
            return self.fail(self.cur.start, "indexing is not supported yet", .{});
        } else if ((self.isP("++") or self.isP("--")) and !self.cur.nl_before) {
            const op = self.text();
            self.advance();
            e = try self.expr(e.loc, .{ .update = .{ .op = op, .target = e } });
        } else if (self.isKw("as")) {
            return self.fail(self.cur.start, "`as` casts are not supported yet", .{});
        } else break;
    }
    return e;
}

/// `<u32>(` after a callee: a type argument list, or null (nothing
/// consumed) when `<` is a comparison.
pub fn tryTypeArgs(self: *Parser) ?[]const ast.TypeRef {
    const s = self.save();
    self.advance(); // <
    var list: std.ArrayList(ast.TypeRef) = .empty;
    while (self.cur.kind == .identifier) {
        list.append(self.arena, .{ .name = self.text(), .loc = self.cur.start }) catch {
            self.restore(s);
            return null;
        };
        self.advance();
        if (!self.isP(",")) break;
        self.advance();
    }
    if (list.items.len == 0 or !self.isP(">")) {
        self.restore(s);
        return null;
    }
    self.advance();
    if (!self.isP("(")) {
        self.restore(s);
        return null;
    }
    return list.items;
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
            if (self.isP("[")) return self.fail(loc, "array literals are not supported yet", .{});
            return self.fail(loc, "unexpected `{s}`", .{self.text()});
        },
        .eof => return self.fail(loc, "expected an expression but the file ended", .{}),
        .invalid => return self.fail(loc, "unexpected character `{s}`", .{self.text()}),
    }
}

pub fn parseObject(self: *Parser, loc: u32) Error!*ast.Expr {
    self.advance(); // {
    var props: std.ArrayList(ast.Property) = .empty;
    while (!self.isP("}")) {
        if (self.isP("...")) return self.fail(self.cur.start, "object spread is not supported", .{});
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
