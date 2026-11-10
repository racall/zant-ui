//! Recursive-descent parser for the TSX subset (`docs/tsx.md`): the parser
//! state, the module, functions, types and statements. Expressions live in
//! `expr_parser.zig`, JSX in `jsx_parser.zig` (text rules in `jsx_text.zig`).
//!
//! Shape follows esbuild's parser: one token of lookahead (`cur`, with the
//! lexer positioned right after it) and explicit mode switches for JSX tags,
//! JSX text and template literals.
const std = @import("std");
const ast = @import("ast.zig");
const lexer = @import("lexer.zig");
const Lexer = lexer.Lexer;
const Token = lexer.Token;
const expr_parser = @import("expr_parser.zig");
const type_parser = @import("type_parser.zig");
const jsx_parser = @import("jsx_parser.zig");

pub const Error = error{ Syntax, OutOfMemory };

pub const Diag = struct {
    loc: u32,
    msg: []const u8,
};

pub const Parser = struct {
    arena: std.mem.Allocator,
    src: []const u8,
    lx: Lexer,
    cur: Token,
    diag: ?Diag = null,

    pub fn init(arena: std.mem.Allocator, src: []const u8) Parser {
        var lx = Lexer.init(src);
        const first = lx.next();
        return .{ .arena = arena, .src = src, .lx = lx, .cur = first };
    }

    // ── helpers ─────────────────────────────────────────────────────────

    pub fn fail(self: *Parser, loc: u32, comptime fmt: []const u8, args: anytype) Error {
        if (self.diag == null) {
            self.diag = .{ .loc = loc, .msg = try std.fmt.allocPrint(self.arena, fmt, args) };
        }
        return error.Syntax;
    }

    pub fn text(self: *const Parser) []const u8 {
        return self.cur.text(self.src);
    }

    pub fn advance(self: *Parser) void {
        self.cur = self.lx.next();
    }

    pub fn isP(self: *const Parser, p: []const u8) bool {
        return self.cur.kind == .punct and std.mem.eql(u8, self.text(), p);
    }

    pub fn isKw(self: *const Parser, kw: []const u8) bool {
        return self.cur.kind == .identifier and std.mem.eql(u8, self.text(), kw);
    }

    pub fn eatP(self: *Parser, p: []const u8) bool {
        if (!self.isP(p)) return false;
        self.advance();
        return true;
    }

    pub fn expectP(self: *Parser, p: []const u8) Error!void {
        if (!self.eatP(p)) return self.unexpected(p);
    }

    pub fn unexpected(self: *Parser, wanted: []const u8) Error {
        return switch (self.cur.kind) {
            .eof => self.fail(self.cur.start, "expected `{s}` but the file ended", .{wanted}),
            .invalid => self.fail(self.cur.start, "unexpected character `{s}`", .{self.text()}),
            else => self.fail(self.cur.start, "expected `{s}`, found `{s}`", .{ wanted, self.text() }),
        };
    }

    pub fn expectIdent(self: *Parser, what: []const u8) Error!ast.Name {
        if (self.cur.kind != .identifier) return self.unexpected(what);
        const name: ast.Name = .{ .name = self.text(), .loc = self.cur.start };
        self.advance();
        return name;
    }

    /// tsc's rule for the end of a statement: a `;`, or (automatic
    /// semicolon insertion) a `}`, the end of the file or a line break.
    pub fn endStatement(self: *Parser) Error!void {
        if (self.eatP(";")) return;
        if (self.isP("}") or self.cur.kind == .eof or self.cur.nl_before) return;
        return self.fail(self.cur.start, "expected `;` or a line break before `{s}`", .{self.text()});
    }

    /// Joins a `>` token with a following `=` into `>=` (binary-operator
    /// position only; see the lexer).
    pub fn rescanGreater(self: *Parser) void {
        if (!self.isP(">")) return;
        if (self.cur.end < self.src.len and self.src[self.cur.end] == '=') {
            self.cur.end += 1;
            self.lx.pos = self.cur.end;
        }
    }

    pub fn new(self: *Parser, comptime T: type, value: T) Error!*T {
        const p = try self.arena.create(T);
        p.* = value;
        return p;
    }

    pub fn expr(self: *Parser, loc: u32, data: ast.Expr.Data) Error!*ast.Expr {
        return self.new(ast.Expr, .{ .loc = loc, .data = data });
    }

    pub const State = struct { pos: u32, cur: Token };

    pub fn save(self: *const Parser) State {
        return .{ .pos = self.lx.pos, .cur = self.cur };
    }

    pub fn restore(self: *Parser, s: State) void {
        self.lx.pos = s.pos;
        self.cur = s.cur;
    }

    // ── module ──────────────────────────────────────────────────────────

    pub fn parseModule(self: *Parser) Error!ast.Module {
        var m: ModuleBuilder = .{};
        while (self.cur.kind != .eof) {
            if (self.eatP(";")) continue;
            if (self.isKw("import")) {
                try m.imports.append(self.arena, try self.parseImport());
                continue;
            }
            var exported = false;
            var default = false;
            if (self.isKw("export")) {
                self.advance();
                exported = true;
                default = self.isKw("default");
                if (default) self.advance();
                if (default and self.cur.kind == .identifier and !self.isKw("function")) {
                    try self.markDefault(&m);
                    continue;
                }
            }
            if (self.isKw("function")) {
                const func = try self.parseFunction();
                try m.functions.append(self.arena, .{ .func = func, .exported = exported, .default = default });
            } else if (!default and self.isKw("const")) {
                try self.parseTopConst(&m, exported);
            } else if (!default and (self.isKw("interface") or self.isKw("type"))) {
                try m.types.append(self.arena, try self.parseTypeDecl(exported));
            } else if (exported) {
                return self.fail(self.cur.start, "only functions, constants and types can be exported", .{});
            } else {
                return self.fail(self.cur.start, "only imports, functions, constants and types are supported at the top level", .{});
            }
        }
        const module: ast.Module = .{
            .imports = m.imports.items,
            .functions = m.functions.items,
            .types = m.types.items,
            .consts = m.consts.items,
        };
        @import("react_compat.zig").normalize(module);
        return module;
    }

    const ModuleBuilder = struct {
        imports: std.ArrayList(ast.Import) = .empty,
        functions: std.ArrayList(ast.TopFunction) = .empty,
        types: std.ArrayList(ast.TypeDecl) = .empty,
        consts: std.ArrayList(ast.TopConst) = .empty,
    };

    /// `export default App;` marks an earlier function component.
    fn markDefault(self: *Parser, m: *ModuleBuilder) Error!void {
        const name = try self.expectIdent("a component name");
        try self.endStatement();
        for (m.functions.items) |*f| {
            if (std.mem.eql(u8, f.func.name, name.name)) {
                f.exported = true;
                f.default = true;
                return;
            }
        }
        return self.fail(name.loc, "`{s}` is not a function declared above", .{name.name});
    }

    /// `interface Todo { ... }` / `type Filter = "all" | "done";`
    fn parseTypeDecl(self: *Parser, exported: bool) Error!ast.TypeDecl {
        const is_interface = self.isKw("interface");
        self.advance();
        const name = try self.expectIdent("a type name");
        if (self.isP("<")) return self.fail(self.cur.start, "generic type declarations are not supported", .{});
        if (is_interface) {
            if (self.isKw("extends")) return self.fail(self.cur.start, "`extends` is not supported; repeat the fields", .{});
            const loc = self.cur.start;
            const detail = try self.arena.create(ast.TypeDetail);
            detail.* = .{ .object = try type_parser.parseMembers(self) };
            return .{ .name = name, .ty = .{ .name = "", .loc = loc, .detail = detail }, .exported = exported };
        }
        try self.expectP("=");
        const ty = try self.parseType();
        try self.endStatement();
        return .{ .name = name, .ty = ty, .exported = exported };
    }

    /// A top-level `const`: an arrow function is a component, anything else
    /// a constant.
    fn parseTopConst(self: *Parser, m: *ModuleBuilder, exported: bool) Error!void {
        self.advance(); // const
        const name = try self.expectIdent("a name");
        var ty: ?ast.TypeRef = null;
        if (self.eatP(":")) ty = try self.parseType();
        try self.expectP("=");
        const value = try expr_parser.parseExpr(self);
        if (self.isP(",")) return self.fail(self.cur.start, "declare one constant per statement", .{});
        try self.endStatement();
        if (value.data != .arrow) {
            try m.consts.append(self.arena, .{ .name = name, .ty = ty, .value = value, .exported = exported });
            return;
        }
        const func = value.data.arrow;
        func.name = name.name;
        if (func.body == .expr) {
            // a concise body becomes `{ return body; }`
            const ret = try self.new(ast.Stmt, .{ .loc = func.body.expr.loc, .data = .{ .ret = func.body.expr } });
            const block = try self.arena.alloc(*ast.Stmt, 1);
            block[0] = ret;
            func.body = .{ .block = block };
        }
        try m.functions.append(self.arena, .{ .func = func, .exported = exported, .default = false });
    }

    /// Parse a named import or React's default / namespace import.
    pub fn parseImport(self: *Parser) Error!ast.Import {
        return @import("import_parser.zig").parse(self);
    }

    // ── functions and types ─────────────────────────────────────────────

    pub fn parseFunction(self: *Parser) Error!*ast.Function {
        const loc = self.cur.start;
        self.advance(); // function
        if (self.isP("*")) return self.fail(self.cur.start, "generator functions are not supported", .{});
        const name = try self.expectIdent("a function name");
        if (self.isP("<")) return self.fail(self.cur.start, "generic functions are not supported", .{});
        const params = try self.parseParams();
        const ret = try self.parseReturnType();
        if (!self.isP("{")) return self.unexpected("{");
        const body = try self.parseBlock();
        return self.new(ast.Function, .{ .loc = loc, .name = name.name, .params = params, .ret = ret, .body = .{ .block = body } });
    }

    pub fn parseParams(self: *Parser) Error![]const ast.Param {
        try self.expectP("(");
        var params: std.ArrayList(ast.Param) = .empty;
        while (!self.isP(")")) {
            if (self.isP("[")) return self.fail(self.cur.start, "array destructuring in parameters is not supported", .{});
            if (self.isP("...")) return self.fail(self.cur.start, "rest parameters are not supported", .{});
            var param: ast.Param = undefined;
            if (self.isP("{")) {
                // `{ todo, onToggle }: Props` (React props)
                const loc = self.cur.start;
                param = .{ .name = .{ .name = "props", .loc = loc }, .ty = null, .destructure = try self.parseDestructure() };
            } else {
                param = .{ .name = try self.expectIdent("a parameter name"), .ty = null };
                param.optional = self.eatP("?");
            }
            if (self.eatP(":")) param.ty = try self.parseType();
            if (self.isP("=")) return self.fail(self.cur.start, "default parameter values are not supported yet", .{});
            try params.append(self.arena, param);
            if (!self.eatP(",")) break;
        }
        try self.expectP(")");
        return params.items;
    }

    /// `{ a, b }` in a parameter: the names (renames and defaults are not
    /// supported).
    fn parseDestructure(self: *Parser) Error![]const ast.Name {
        self.advance(); // {
        var names: std.ArrayList(ast.Name) = .empty;
        while (!self.isP("}")) {
            if (self.isP("...")) return self.fail(self.cur.start, "rest properties are not supported", .{});
            const name = try self.expectIdent("a property name");
            if (self.isP(":")) return self.fail(self.cur.start, "renaming destructured props is not supported; read `props.{s}` instead", .{name.name});
            if (self.isP("=")) return self.fail(self.cur.start, "default values for destructured props are not supported yet", .{});
            try names.append(self.arena, name);
            if (!self.eatP(",")) break;
        }
        try self.expectP("}");
        return names.items;
    }

    pub fn parseReturnType(self: *Parser) Error!?ast.TypeRef {
        if (!self.eatP(":")) return null;
        return try self.parseType();
    }

    /// A named type, optionally dotted (`JSX.Element`).
    /// A type annotation (`type_parser.zig`).
    pub fn parseType(self: *Parser) Error!ast.TypeRef {
        return type_parser.parseType(self);
    }

    // ── statements ──────────────────────────────────────────────────────

    pub fn parseBlock(self: *Parser) Error![]const *ast.Stmt {
        try self.expectP("{");
        var stmts: std.ArrayList(*ast.Stmt) = .empty;
        while (!self.isP("}")) {
            if (self.cur.kind == .eof) return self.unexpected("}");
            if (self.eatP(";")) continue;
            try stmts.append(self.arena, try self.parseStmt());
        }
        self.advance();
        return stmts.items;
    }

    pub fn parseStmt(self: *Parser) Error!*ast.Stmt {
        const loc = self.cur.start;
        if (self.isKw("const") or self.isKw("let") or self.isKw("var")) {
            const is_const = self.isKw("const");
            self.advance();
            const pattern: ast.Pattern = if (self.isP("[")) blk: {
                self.advance();
                var names: std.ArrayList(?ast.Name) = .empty;
                while (!self.isP("]")) {
                    if (self.isP(",")) {
                        self.advance();
                        try names.append(self.arena, null);
                        continue;
                    }
                    try names.append(self.arena, try self.expectIdent("a name"));
                    if (!self.eatP(",")) break;
                }
                try self.expectP("]");
                break :blk .{ .array = names.items };
            } else if (self.isP("{"))
                return self.fail(self.cur.start, "object destructuring is not supported yet", .{})
            else
                .{ .ident = try self.expectIdent("a variable name") };
            var ty: ?ast.TypeRef = null;
            if (self.eatP(":")) ty = try self.parseType();
            var init_expr: ?*ast.Expr = null;
            if (self.eatP("=")) init_expr = try expr_parser.parseExpr(self);
            if (self.isP(",")) return self.fail(self.cur.start, "declare one variable per statement", .{});
            try self.endStatement();
            return self.new(ast.Stmt, .{ .loc = loc, .data = .{ .decl = .{ .is_const = is_const, .pattern = pattern, .ty = ty, .init = init_expr } } });
        }
        if (self.isKw("function")) {
            const func = try self.parseFunction();
            return self.new(ast.Stmt, .{ .loc = loc, .data = .{ .func = func } });
        }
        if (self.isKw("return")) {
            self.advance();
            var value: ?*ast.Expr = null;
            if (!self.isP(";") and !self.isP("}") and self.cur.kind != .eof) {
                if (self.cur.nl_before) {
                    // JS inserts `;` here and returns nothing
                    return self.fail(self.cur.start, "a line break after `return` ends the statement; start the value on the same line (`return (`)", .{});
                }
                value = try expr_parser.parseExpr(self);
            }
            try self.endStatement();
            return self.new(ast.Stmt, .{ .loc = loc, .data = .{ .ret = value } });
        }
        if (self.isKw("if")) {
            self.advance();
            try self.expectP("(");
            const cond = try expr_parser.parseExpr(self);
            try self.expectP(")");
            const then = try self.parseStmt();
            var else_: ?*ast.Stmt = null;
            if (self.isKw("else")) {
                self.advance();
                else_ = try self.parseStmt();
            }
            return self.new(ast.Stmt, .{ .loc = loc, .data = .{ .if_ = .{ .cond = cond, .then = then, .else_ = else_ } } });
        }
        if (self.isP("{")) {
            const body = try self.parseBlock();
            return self.new(ast.Stmt, .{ .loc = loc, .data = .{ .block = body } });
        }
        inline for (.{ "for", "while", "do", "switch", "break", "continue" }) |kw| {
            if (self.isKw(kw)) return @import("loop_parser.zig").parse(self);
        }
        inline for (.{ "try", "throw", "class", "async", "await" }) |kw| {
            if (self.isKw(kw)) return self.fail(loc, "`{s}` is not supported yet", .{kw});
        }
        const e = try expr_parser.parseExpr(self);
        try self.endStatement();
        return self.new(ast.Stmt, .{ .loc = loc, .data = .{ .expr = e } });
    }
};
