//! `zant-math`: port of `examples/math.zig` (WP-3.11).
//!
//! The math gallery: a title line and 18 display formulas laid out with the
//! KaTeX engine (`math::layout::layout`) and drawn as retained vectors, in a
//! vertically scrolling column. `math/mod.zig`'s `block` wrapper target
//! (WP-3.6 left the UI wrapper to this WP) is [`math_block`] here: it is
//! `zant.math.block` line for line (`display = true`, 1em vertical margin
//! split in half, centred full-width row).
#![forbid(unsafe_code)]

use std::rc::Rc;

use zant::core::geometry::Edges;
use zant::layout::flex;
use zant::math::layout::{self, Options as LayoutOptions};
use zant::platform::platform::WindowOptions;
use zant::ui::{self, BoxOptions, RunOptions, TextOptions, VectorOptions};

/// `examples/math.zig: formulas` verbatim (18 formulas, including `\ce` /
/// `\pu`, arrays, xarrows and `\colorbox`).
pub const FORMULAS: [&str; 18] = [
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
    "\\mathrm{2H_2 + O_2} \\xrightarrow[\\Delta]{\\text{Pt}} \\mathrm{2H_2O} \\quad \\mathrm{N_2 + 3H_2} \\xrightleftharpoons[\\text{high } T]{\\text{cat.}} \\mathrm{2NH_3} \\quad A \\xRightarrow{f} B",
    "\\sum_{\\substack{0<i<m \\\\ 0<j<n}} P(i,j) \\quad \\colorbox{#ffe58f}{$x^2$} \\quad \\fcolorbox{red}{white}{key} \\quad {\\large L}{\\small s} \\quad X \\xhookrightarrow{f} Y \\xmapsto{g} Z \\xlongequal{\\text{def}} W",
    "\\ce{2H2 + O2 ->[\\text{点燃}] 2H2O} \\quad \\ce{N2 + 3H2 <=>[\\text{催化剂}][\\text{高温高压}] 2NH3} \\quad \\ce{CaCO3 ->[\\Delta] CaO + CO2 ^}",
    "\\ce{SO4^2- + Ba^2+ -> BaSO4 v} \\quad \\ce{[Cu(NH3)4]^2+} \\quad \\ce{CuSO4*5H2O} \\quad \\ce{^{227}_{90}Th} \\quad \\ce{Fe^{III}} \\quad \\pu{123 kJ mol-1} \\quad \\pu{1.2e3 kg m/s^2}",
];

/// Zig `math/mod.zig: block`: a centred display formula (KaTeX
/// `.katex-display`: 1em vertical margin).
pub fn math_block(source: &str, options: LayoutOptions) -> ui::Node {
    let options = LayoutOptions {
        display: true,
        ..options
    };
    let drawing = layout::layout(source, options);
    ui::row(BoxOptions {
        width_percent: Some(100.0),
        justify: flex::Justify::Center,
        margin: Edges {
            top: 0.5 * options.size,
            bottom: 0.5 * options.size,
            ..Edges::default()
        },
        children: vec![ui::vector(VectorOptions {
            drawing: Rc::new(drawing),
            ..VectorOptions::default()
        })],
        ..BoxOptions::default()
    })
}

/// Zig `State` (`{ dummy: u8 = 0 }`).
pub struct MathApp;

impl MathApp {
    /// Zig `render`.
    fn build(&mut self) -> ui::Node {
        let mut rows: Vec<ui::Node> = Vec::with_capacity(FORMULAS.len() + 1);
        rows.push(ui::text(
            "Zant math (KaTeX layout, STIX Two Math)",
            TextOptions {
                size: 13.0,
                color: Some(0xff8b918c),
                ..TextOptions::default()
            },
        ));
        for formula in FORMULAS {
            rows.push(math_block(
                formula,
                LayoutOptions {
                    size: 17.0,
                    max_width: Some(900.0),
                    ..LayoutOptions::default()
                },
            ));
        }
        // the root node never scrolls itself: wrap the content in a scroll column
        ui::column(BoxOptions {
            grow: 1.0,
            background: Some(0xfffcfcfa),
            align_items: flex::Align::Stretch,
            children: vec![ui::column(BoxOptions {
                grow: 1.0,
                scroll: true,
                padding: ui::inset(28.0),
                gap: 2.0,
                align_items: flex::Align::Stretch,
                children: rows,
                ..BoxOptions::default()
            })],
            ..BoxOptions::default()
        })
    }
}

impl ui::App for MathApp {
    fn render(&mut self) -> ui::Node {
        self.build()
    }
}

/// Zig `launch.run(.., .{ .title = "Zant Math", .window = .{ .width = 980,
/// .height = 1100 } })`.
pub fn run_options() -> RunOptions {
    RunOptions {
        title: String::from("Zant Math"),
        window: WindowOptions {
            width: 980.0,
            height: 1100.0,
            ..WindowOptions::default()
        },
        ..RunOptions::default()
    }
}
