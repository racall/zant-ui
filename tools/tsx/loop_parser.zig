//! Loop and switch syntax, kept separate from the module and expression parsers.
const std = @import("std");
const ast = @import("ast.zig");
const parser = @import("parser.zig");
const expressions = @import("expr_parser.zig");
const Parser = parser.Parser;
const Error = parser.Error;

/// Parse a statement whose leading keyword has already been recognized.
pub fn parse(p: *Parser) Error!*ast.Stmt {
    const loc = p.cur.start;
    if (p.isKw("for")) return parseFor(p);
    if (p.isKw("switch")) return parseSwitch(p);
    if (p.isKw("break") or p.isKw("continue")) {
        const is_break = p.isKw("break");
        p.advance();
        if (!p.cur.nl_before and p.cur.kind == .identifier) {
            return p.fail(p.cur.start, "labeled break and continue are not supported", .{});
        }
        try p.endStatement();
        return p.new(ast.Stmt, .{ .loc = loc, .data = if (is_break) .break_ else .continue_ });
    }
    const do_first = p.isKw("do");
    p.advance();
    const body = if (do_first) try p.parseStmt() else null;
    if (do_first) {
        if (!p.isKw("while")) return p.unexpected("while");
        p.advance();
    }
    try p.expectP("(");
    const cond = try expressions.parseExpr(p);
    try p.expectP(")");
    const result = try p.new(ast.Stmt, .{ .loc = loc, .data = .{ .while_ = .{
        .cond = cond,
        .body = body orelse try p.parseStmt(),
        .do_first = do_first,
    } } });
    if (do_first) try p.endStatement();
    return result;
}

fn parseFor(p: *Parser) Error!*ast.Stmt {
    const loc = p.cur.start;
    p.advance();
    try p.expectP("(");
    var init: ?*ast.Stmt = null;
    if (p.isKw("let") or p.isKw("const")) {
        const decl_loc = p.cur.start;
        const is_const = p.isKw("const");
        p.advance();
        const name = try p.expectIdent("a loop variable name");
        if (p.isKw("of")) {
            p.advance();
            const iterable = try expressions.parseExpr(p);
            try p.expectP(")");
            return p.new(ast.Stmt, .{ .loc = loc, .data = .{ .for_of = .{
                .name = name,
                .is_const = is_const,
                .iterable = iterable,
                .body = try p.parseStmt(),
            } } });
        }
        if (is_const) return p.fail(decl_loc, "C-style for initializer must use let", .{});
        const ty = if (p.eatP(":")) try p.parseType() else null;
        try p.expectP("=");
        const value = try expressions.parseExpr(p);
        init = try p.new(ast.Stmt, .{ .loc = decl_loc, .data = .{ .decl = .{
            .is_const = false,
            .pattern = .{ .ident = name },
            .ty = ty,
            .init = value,
        } } });
    } else if (!p.isP(";")) {
        const value = try expressions.parseExpr(p);
        if (value.data != .assign) {
            return p.fail(value.loc, "for initializer must be a let declaration or assignment", .{});
        }
        init = try p.new(ast.Stmt, .{ .loc = value.loc, .data = .{ .expr = value } });
    }
    try p.expectP(";");
    const cond = if (!p.isP(";")) try expressions.parseExpr(p) else null;
    try p.expectP(";");
    const update = if (!p.isP(")")) try expressions.parseExpr(p) else null;
    if (update) |value| if (value.data != .assign and value.data != .update) {
        return p.fail(value.loc, "for update must be an assignment, ++ or --", .{});
    };
    try p.expectP(")");
    return p.new(ast.Stmt, .{ .loc = loc, .data = .{ .for_ = .{
        .init = init,
        .cond = cond,
        .update = update,
        .body = try p.parseStmt(),
    } } });
}

fn parseSwitch(p: *Parser) Error!*ast.Stmt {
    const loc = p.cur.start;
    p.advance();
    try p.expectP("(");
    const value = try expressions.parseExpr(p);
    try p.expectP(")");
    try p.expectP("{");
    var cases: std.ArrayList(ast.Case) = .empty;
    var has_default = false;
    while (!p.isP("}")) {
        const case_loc = p.cur.start;
        const is_default = p.isKw("default");
        if (!is_default and !p.isKw("case")) return p.unexpected("case or default");
        if (is_default and has_default) return p.fail(case_loc, "switch has more than one default", .{});
        if (is_default) has_default = true;
        p.advance();
        const test_expr = if (is_default) null else try expressions.parseExpr(p);
        try p.expectP(":");
        var body: std.ArrayList(*ast.Stmt) = .empty;
        while (!p.isP("}") and !p.isKw("case") and !p.isKw("default")) {
            if (p.eatP(";")) continue;
            try body.append(p.arena, try p.parseStmt());
        }
        try cases.append(p.arena, .{ .loc = case_loc, .value = test_expr, .body = body.items });
    }
    p.advance();
    return p.new(ast.Stmt, .{ .loc = loc, .data = .{ .switch_ = .{ .value = value, .cases = cases.items } } });
}
