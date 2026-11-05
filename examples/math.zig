//! Math gallery: `zig build run-math`.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;

const formulas = [_][]const u8{
    "x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}",
    "\\sum_{k=1}^{n} k^2 = \\frac{n(n+1)(2n+1)}{6}",
    "\\int_{-\\infty}^{\\infty} e^{-x^2}\\,dx = \\sqrt{\\pi}",
    "\\lim_{x \\to 0} \\frac{\\sin x}{x} = 1 \\qquad \\prod_{i=1}^{n} a_i",
    "\\left( \\sum_{i=1}^n a_i b_i \\right)^2 \\le \\left( \\sum_{i=1}^n a_i^2 \\right) \\left( \\sum_{i=1}^n b_i^2 \\right)",
    "A = \\begin{pmatrix} a_{11} & a_{12} & a_{13} \\\\ a_{21} & a_{22} & a_{23} \\\\ a_{31} & a_{32} & a_{33} \\end{pmatrix}, \\quad \\det\\begin{vmatrix} a & b \\\\ c & d \\end{vmatrix} = ad - bc",
    "|x| = \\begin{cases} x & \\text{if } x \\ge 0 \\\\ -x & \\text{otherwise} \\end{cases}",
    "\\begin{aligned} (a+b)^2 &= a^2 + 2ab + b^2 \\\\ &\\ne a^2 + b^2 \\end{aligned}",
    "\\hat{\\theta} = \\arg\\max_{\\theta} \\mathcal{L}(\\theta), \\quad \\vec{F} = m\\vec{a}, \\quad \\overline{z} = a - bi",
    "\\nabla \\times \\mathbf{E} = -\\frac{\\partial \\mathbf{B}}{\\partial t}, \\quad \\mathbb{R}^n, \\mathbb{Z}, \\mathfrak{g}",
    "\\sqrt[3]{x^3 + y^3} \\quad \\binom{n}{k} = \\frac{n!}{k!(n-k)!} \\quad \\boxed{E = mc^2}",
    "\\underbrace{1 + 2 + \\cdots + n}_{n \\text{ terms}} \\quad \\overbrace{a + b}^{2} \\quad e^{i\\pi} + 1 = 0",
    "f(x) = \\frac{1}{\\sigma\\sqrt{2\\pi}} e^{-\\frac{(x-\\mu)^2}{2\\sigma^2}}",
    "\\frac{1}{1 + \\frac{1}{1 + \\frac{1}{x}}} \\quad \\frac{\\partial^2 u}{\\partial t^2} = c^2 \\nabla^2 u",
};

const State = struct { dummy: u8 = 0 };

fn render(_: *State) ui.Node {
    var rows: [formulas.len + 1]ui.Node = undefined;
    rows[0] = ui.text("Zant math (KaTeX layout, STIX Two Math)", .{ .size = 13, .color = 0xff8b918c });
    for (formulas, 0..) |f, i| rows[i + 1] = zant.math.block(f, .{ .size = 17, .max_width = 900 });
    // the root node never scrolls itself: wrap the content in a scroll column
    return ui.column(.{
        .grow = 1,
        .background = 0xfffcfcfa,
        .align_items = .stretch,
        .children = &.{ui.column(.{
            .grow = 1,
            .scroll = true,
            .padding = ui.inset(28),
            .gap = 2,
            .align_items = .stretch,
            .children = &rows,
        })},
    });
}

pub fn main(init: std.process.Init) !void {
    try zant.launch.run(init.gpa, State{}, render, .{
        .title = "Zant Math",
        .window = .{ .width = 980, .height = 1100 },
    });
}
