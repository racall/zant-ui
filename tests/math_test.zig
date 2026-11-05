const std = @import("std");
const zant = @import("zant");
const math = zant.math;

fn lay(arena: std.mem.Allocator, src: []const u8, display: bool) !math.Drawing {
    return math.layout(arena, src, .{ .size = 16, .display = display });
}

test "math layout produces sized drawings" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    const cases = [_][]const u8{
        "a^2 + b^2 = c^2",
        "\\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}",
        "\\sum_{k=1}^{n} k = \\frac{n(n+1)}{2}",
        "\\int_0^\\infty e^{-x^2}\\,dx = \\frac{\\sqrt{\\pi}}{2}",
        "\\left( \\frac{a}{b} \\right)^2",
        "\\begin{pmatrix} 1 & 0 \\\\ 0 & 1 \\end{pmatrix}",
        "f(x) = \\begin{cases} x & x \\ge 0 \\\\ -x & \\text{otherwise} \\end{cases}",
        "\\begin{aligned} a &= b + c \\\\ &= d \\end{aligned}",
        "\\hat{x} \\vec{v} \\overline{AB} \\underbrace{a+b}_{n}",
        "\\mathbb{R}^n \\mathcal{L} \\mathbf{F} \\operatorname{tr}(A)",
        "\\lim_{x \\to 0} \\frac{\\sin x}{x} = 1",
        "\\sqrt[3]{x} \\binom{n}{k} \\boxed{E=mc^2}",
    };
    for (cases) |src| {
        const d = try lay(a, src, true);
        try std.testing.expect(d.width > 10);
        try std.testing.expect(d.ascent > 5);
        try std.testing.expect(d.commands.len > 0);
    }
}

test "display fractions are taller than inline ones" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const display = try lay(arena.allocator(), "\\frac{a}{b}", true);
    const inline_ = try lay(arena.allocator(), "\\frac{a}{b}", false);
    try std.testing.expect(display.ascent + display.descent > inline_.ascent + inline_.descent);
}

test "superscript is raised and subscript lowered" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const plain = try lay(arena.allocator(), "x", false);
    const sup = try lay(arena.allocator(), "x^2", false);
    const sub = try lay(arena.allocator(), "x_2", false);
    try std.testing.expect(sup.ascent > plain.ascent);
    try std.testing.expect(sub.descent > plain.descent);
}

test "parse errors render the source in the error colour" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const d = try math.layout(arena.allocator(), "\\frac{a}{", .{ .error_color = 0xffff0000 });
    try std.testing.expectEqual(@as(usize, 1), d.commands.len);
    try std.testing.expectEqual(@as(u32, 0xffff0000), d.commands[0].text.color);
}

test "math block renders a vector node" {
    zant.ui.beginFrame();
    defer zant.ui.endFrame();
    const node = math.block("\\frac{1}{2}", .{});
    try std.testing.expect(node.element.children[0].element.kind == .vector);
}

test "matrix-like inline formula lays out as math, not an error" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const d = try math.layout(arena.allocator(), "U=[u_1\\ \\cdots\\ u_m]", .{ .display = false, .error_color = 0xffff0000 });
    try std.testing.expect(d.width > 0);
    try std.testing.expect(d.commands.len > 1);
}
