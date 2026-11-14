//! Math layout tests: sizes, display vs inline style, scripts, the error
//! rendering of a formula that does not parse and the display-formula node.
//!
//! The smoke test at the bottom lays out the 18 formulas of the `zant-math`
//! example and asserts the drawings are non-empty and sane.

use zant::core::vector::Command;
use zant::math::layout::{Drawing, Options, layout};

fn lay(source: &str, display: bool) -> Drawing {
    layout(
        source,
        Options {
            size: 16.0,
            display,
            ..Default::default()
        },
    )
}

#[test]
fn math_layout_produces_sized_drawings() {
    let cases = [
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
    ];
    for source in cases {
        let drawing = lay(source, true);
        assert!(drawing.width > 10.0, "width of {source}");
        assert!(drawing.ascent > 5.0, "ascent of {source}");
        assert!(!drawing.commands.is_empty(), "commands of {source}");
    }
}

#[test]
fn display_fractions_are_taller_than_inline_ones() {
    let display = lay("\\frac{a}{b}", true);
    let inline = lay("\\frac{a}{b}", false);
    assert!(
        display.ascent + display.descent > inline.ascent + inline.descent,
        "display {} vs inline {}",
        display.ascent + display.descent,
        inline.ascent + inline.descent
    );
}

#[test]
fn superscript_is_raised_and_subscript_lowered() {
    let plain = lay("x", false);
    let sup = lay("x^2", false);
    let sub = lay("x_2", false);
    assert!(
        sup.ascent > plain.ascent,
        "sup {} vs {}",
        sup.ascent,
        plain.ascent
    );
    assert!(
        sub.descent > plain.descent,
        "sub {} vs {}",
        sub.descent,
        plain.descent
    );
}

#[test]
fn parse_errors_render_the_source_in_the_error_colour() {
    let drawing = layout(
        "\\frac{a}{",
        Options {
            error_color: 0xffff_0000,
            ..Default::default()
        },
    );
    assert_eq!(drawing.commands.len(), 1);
    match &drawing.commands[0] {
        Command::Text(text) => assert_eq!(text.color, 0xffff_0000),
        other => panic!("expected text, got {other:?}"),
    }
}

#[test]
fn matrix_like_inline_formula_lays_out_as_math_not_an_error() {
    let drawing = layout(
        "U=[u_1\\ \\cdots\\ u_m]",
        Options {
            display: false,
            error_color: 0xffff_0000,
            ..Default::default()
        },
    );
    assert!(drawing.width > 0.0);
    assert!(drawing.commands.len() > 1);
}

#[test]
fn math_block_renders_a_vector_node() {
    zant::ui::begin_frame();
    let node = zant::math::block("\\frac{1}{2}", Options::default());
    zant::ui::end_frame();
    assert_eq!(
        node.element().children[0].element().kind,
        zant::core::node::NodeKind::Vector
    );
}

/// Smoke test: every formula of the `zant-math` example lays out into a
/// non-empty drawing with a sane width and extent.
#[test]
fn example_formulas_lay_out() {
    let formulas = [
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
    let error_color = Options::default().error_color;
    for (i, formula) in formulas.iter().enumerate() {
        // Exactly the options the example uses for each block.
        let drawing = layout(
            formula,
            Options {
                size: 17.0,
                max_width: Some(900.0),
                ..Default::default()
            },
        );
        println!(
            "formula {}: width={:.2} ascent={:.2} descent={:.2} commands={}",
            i + 1,
            drawing.width,
            drawing.ascent,
            drawing.descent,
            drawing.commands.len()
        );
        assert!(
            !drawing.commands.is_empty(),
            "formula {} produced no commands: {formula}",
            i + 1
        );
        // A parse error would be rendered as one text run in the error colour.
        let failed = drawing.commands.len() == 1
            && matches!(&drawing.commands[0], Command::Text(text) if text.color == error_color);
        assert!(!failed, "formula {} failed to parse: {formula}", i + 1);
        assert!(
            drawing.width > 1.0 && drawing.width < 4000.0,
            "formula {} width {} is not sane",
            i + 1,
            drawing.width
        );
        assert!(
            drawing.height() > 1.0 && drawing.height() < 1000.0,
            "formula {} height {} is not sane",
            i + 1,
            drawing.height()
        );
        assert!(drawing.width.is_finite() && drawing.ascent.is_finite());
        assert!(drawing.descent.is_finite() && drawing.descent >= 0.0);
    }
}
