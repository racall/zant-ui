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
        var imports: std.ArrayList(ast.Import) = .empty;
        var functions: std.ArrayList(ast.TopFunction) = .empty;
        while (self.cur.kind != .eof) {
            if (self.isKw("import")) {
                try imports.append(self.arena, try self.parseImport());
            } else if (self.isKw("export")) {
                self.advance();
                const default = self.isKw("default");
                if (default) self.advance();
                if (!default and self.isKw("const")) {
                    const func = try self.parseArrowComponent();
                    try functions.append(self.arena, .{ .func = func, .exported = true, .default = false });
                    continue;
                }
                if (!self.isKw("function")) {
                    if (default and self.cur.kind == .identifier) {
                        // `export default App;` marks an earlier function
                        const name = try self.expectIdent("a component name");
                        try self.endStatement();
                        for (functions.items) |*f| {
                            if (std.mem.eql(u8, f.func.name, name.name)) {
                                f.exported = true;
                                f.default = true;
                                break;
                            }
                        } else return self.fail(name.loc, "`{s}` is not a function declared above", .{name.name});
                        continue;
                    }
                    return self.fail(self.cur.start, "only function components can be exported (`export default function App() {{ ... }}`)", .{});
                }
                const func = try self.parseFunction();
                try functions.append(self.arena, .{ .func = func, .exported = true, .default = default });
            } else if (self.isKw("function")) {
                const func = try self.parseFunction();
                try functions.append(self.arena, .{ .func = func, .exported = false, .default = false });
            } else if (self.isKw("const")) {
                const func = try self.parseArrowComponent();
                try functions.append(self.arena, .{ .func = func, .exported = false, .default = false });
            } else if (self.isP(";")) {
                self.advance();
            } else if (self.cur.kind == .eof) {
                break;
            } else {
                return self.fail(self.cur.start, "only `import` and function components are supported at the top level", .{});
            }
        }
        return .{ .imports = imports.items, .functions = functions.items };
    }

    /// `const App = () => { ... };` / `const App = () => <View />;` at the
    /// top level: a function component written as an arrow. A concise body
    /// becomes `{ return body; }`.
    pub fn parseArrowComponent(self: *Parser) Error!*ast.Function {
        self.advance(); // const
        const name = try self.expectIdent("a component name");
        if (self.eatP(":")) _ = try self.parseType(); // `: React.FC` and friends
        if (!self.isP("=")) return self.fail(self.cur.start, "only function components are supported at the top level (`const {s} = () => ...`)", .{name.name});
        self.advance();
        const value = try expr_parser.parseExpr(self);
        if (value.data != .arrow) {
            return self.fail(value.loc, "only function components are supported at the top level; top-level constants arrive in phase 2", .{});
        }
        try self.endStatement();
        const func = value.data.arrow;
        func.name = name.name;
        if (func.body == .expr) {
            const ret = try self.new(ast.Stmt, .{ .loc = func.body.expr.loc, .data = .{ .ret = func.body.expr } });
            const block = try self.arena.alloc(*ast.Stmt, 1);
            block[0] = ret;
            func.body = .{ .block = block };
        }
        return func;
    }

    pub fn parseImport(self: *Parser) Error!ast.Import {
        const loc = self.cur.start;
        self.advance(); // import
        var type_only = false;
        if (self.isKw("type")) {
            type_only = true;
            self.advance();
        }
        var names: std.ArrayList(ast.ImportName) = .empty;
        if (self.cur.kind == .identifier and !self.isKw("from")) {
            return self.fail(self.cur.start, "default imports are not supported; use `import {{ ... }} from \"zant\"`", .{});
        }
        if (self.isP("*")) return self.fail(self.cur.start, "namespace imports are not supported; use `import {{ ... }} from \"zant\"`", .{});
        try self.expectP("{");
        while (!self.isP("}")) {
            if (self.isKw("type")) {
                // `import { type Foo }`: the name is a type either way
                const s = self.save();
                self.advance();
                if (self.cur.kind != .identifier) self.restore(s);
            }
            const imported = try self.expectIdent("an imported name");
            var local = imported;
            if (self.isKw("as")) {
                self.advance();
                local = try self.expectIdent("a local name");
            }
            try names.append(self.arena, .{ .imported = imported, .local = local });
            if (!self.eatP(",")) break;
        }
        try self.expectP("}");
        if (!self.isKw("from")) return self.unexpected("from");
        self.advance();
        if (self.cur.kind != .string) return self.unexpected("a module name");
        const source_loc = self.cur.start;
        const source = lexer.decodeString(self.arena, self.text()) catch return self.fail(source_loc, "invalid escape in string", .{});
        self.advance();
        try self.endStatement();
        return .{ .loc = loc, .source = source, .source_loc = source_loc, .type_only = type_only, .names = names.items };
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
            if (self.isP("{") or self.isP("[")) {
                return self.fail(self.cur.start, "destructured parameters are not supported yet", .{});
            }
            if (self.isP("...")) return self.fail(self.cur.start, "rest parameters are not supported", .{});
            const name = try self.expectIdent("a parameter name");
            if (self.isP("?")) return self.fail(self.cur.start, "optional parameters are not supported yet", .{});
            var ty: ?ast.TypeRef = null;
            if (self.eatP(":")) ty = try self.parseType();
            if (self.isP("=")) return self.fail(self.cur.start, "default parameter values are not supported yet", .{});
            try params.append(self.arena, .{ .name = name, .ty = ty });
            if (!self.eatP(",")) break;
        }
        try self.expectP(")");
        return params.items;
    }

    pub fn parseReturnType(self: *Parser) Error!?ast.TypeRef {
        if (!self.eatP(":")) return null;
        return try self.parseType();
    }

    /// A named type, optionally dotted (`JSX.Element`).
    pub fn parseType(self: *Parser) Error!ast.TypeRef {
        if (self.cur.kind == .string) return self.fail(self.cur.start, "string literal types are not supported yet", .{});
        if (self.isP("(") or self.isP("{")) return self.fail(self.cur.start, "only named types (`number`, `string`, `boolean`, `u32`, ...) are supported", .{});
        const first = try self.expectIdent("a type");
        var end = first.loc + @as(u32, @intCast(first.name.len));
        while (self.isP(".")) {
            self.advance();
            const part = try self.expectIdent("a type name");
            end = part.loc + @as(u32, @intCast(part.name.len));
        }
        if (self.isP("[")) return self.fail(self.cur.start, "array types are not supported yet", .{});
        if (self.isP("<")) return self.fail(self.cur.start, "generic types are not supported", .{});
        if (self.isP("|")) return self.fail(self.cur.start, "union types are not supported yet", .{});
        return .{ .name = self.src[first.loc..end], .loc = first.loc };
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
        inline for (.{ "for", "while", "do", "switch", "try", "throw", "class", "async", "await" }) |kw| {
            if (self.isKw(kw)) return self.fail(loc, "`{s}` is not supported yet", .{kw});
        }
        const e = try expr_parser.parseExpr(self);
        try self.endStatement();
        return self.new(ast.Stmt, .{ .loc = loc, .data = .{ .expr = e } });
    }
};
