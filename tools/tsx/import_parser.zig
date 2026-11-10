//! Import clauses, including React's ignored default / namespace bindings.
const std = @import("std");
const ast = @import("ast.zig");
const lexer = @import("lexer.zig");
const Parser = @import("parser.zig").Parser;
const Error = @import("parser.zig").Error;

/// Parse an import; only React permits default and namespace clauses.
pub fn parse(self: *Parser) Error!ast.Import {
    const loc = self.cur.start;
    self.advance(); // import
    var type_only = false;
    if (self.isKw("type")) {
        type_only = true;
        self.advance();
    }
    var names: std.ArrayList(ast.ImportName) = .empty;
    var default_name: ?ast.Name = null;
    var namespace_name: ?ast.Name = null;
    if (self.cur.kind == .string) {
        return self.fail(self.cur.start, "side-effect imports are not supported", .{});
    }
    if (self.cur.kind == .identifier and !self.isKw("from")) {
        default_name = try self.expectIdent("a local name");
        if (!self.eatP(",")) {
            if (!self.isKw("from")) return self.unexpected("from");
        } else if (!self.isP("{") and !self.isP("*")) return self.unexpected("{ or *");
    }
    if (self.eatP("*")) {
        if (!self.isKw("as")) return self.unexpected("as");
        self.advance();
        namespace_name = try self.expectIdent("a local name");
    } else if (default_name == null or self.isP("{")) {
        try self.expectP("{");
        while (!self.isP("}")) {
            var name_type_only = false;
            if (self.isKw("type")) {
                // `import { type Foo }`: retain the modifier for Zig imports
                const saved = self.save();
                self.advance();
                if (self.cur.kind != .identifier) self.restore(saved) else name_type_only = true;
            }
            const imported = try self.expectIdent("an imported name");
            var local = imported;
            if (self.isKw("as")) {
                self.advance();
                local = try self.expectIdent("a local name");
            }
            try names.append(self.arena, .{
                .imported = imported,
                .local = local,
                .type_only = name_type_only,
            });
            if (!self.eatP(",")) break;
        }
        try self.expectP("}");
    }
    if (!self.isKw("from")) return self.unexpected("from");
    self.advance();
    if (self.cur.kind != .string) return self.unexpected("a module name");
    const source_loc = self.cur.start;
    const source = lexer.decodeString(self.arena, self.text()) catch {
        return self.fail(source_loc, "invalid escape in string", .{});
    };
    self.advance();
    try self.endStatement();
    if (!std.mem.eql(u8, source, "react")) {
        if (default_name) |name| return self.fail(name.loc, "default imports are only supported from \"react\"", .{});
        if (namespace_name) |name| {
            return self.fail(name.loc, "namespace imports are only supported from \"react\"", .{});
        }
    }
    return .{
        .loc = loc,
        .source = source,
        .source_loc = source_loc,
        .type_only = type_only,
        .names = names.items,
        .default_name = default_name,
        .namespace_name = namespace_name,
    };
}
