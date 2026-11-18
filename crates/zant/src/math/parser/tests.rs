//! Parser unit tests and example formula smoke coverage.

use super::*;

#[test]
fn parse_structures() {
    let nodes = parse("\\frac{a}{b} + x_i^2 + \\sqrt[3]{y} \\left( \\sum_{k=1}^n k \\right)")
        .expect("parse structures");
    assert!(matches!(nodes[0], Node::Frac(_)));
    assert!(matches!(nodes[2], Node::SupSub(_)));
    assert!(matches!(nodes[4], Node::Sqrt(_)));
    assert!(matches!(nodes[5], Node::LeftRight(_)));
    let env = parse("\\begin{pmatrix} 1 & 2 \\\\ 3 & 4 \\end{pmatrix}").expect("pmatrix");
    match &env[0] {
        Node::Array(array) => {
            assert_eq!(array.rows.len(), 2);
            assert_eq!(array.cols.len(), 2);
        }
        other => panic!("expected array, got {other:?}"),
    }
    assert_eq!(parse("\\frac{a}{b"), Err(ParseError));
    let cases = parse("f(x) = \\begin{cases} 1 & x > 0 \\\\ 0 & \\text{otherwise} \\end{cases}")
        .expect("cases");
    assert!(matches!(cases.last(), Some(Node::Array(_))));
}

#[test]
fn parse_errors() {
    assert_eq!(parse("x^1^2"), Err(ParseError)); // double superscript
    assert_eq!(parse("\\left( x"), Err(ParseError)); // missing \right
    assert_eq!(parse("\\begin{nope} x \\end{nope}"), Err(ParseError));
    assert_eq!(parse("}"), Err(ParseError));
    assert_eq!(parse("\\frac"), Err(ParseError));
}

#[test]
fn parse_font_commands() {
    let nodes = parse("\\mathbb{R}").expect("mathbb");
    match &nodes[0] {
        Node::Group(group) => match &group[0] {
            Node::Atom(atom) => {
                assert_eq!(atom.text, "R");
                assert_eq!(atom.variant, fonts::Variant::DoubleStruck);
            }
            other => panic!("expected atom, got {other:?}"),
        },
        other => panic!("expected group, got {other:?}"),
    }
    let nodes = parse("\\text{hi}").expect("text");
    match &nodes[0] {
        Node::Text(text) => {
            assert_eq!(text.text, "hi");
            assert_eq!(text.variant, fonts::Variant::Roman);
        }
        other => panic!("expected text, got {other:?}"),
    }
}

#[test]
fn delimiters_and_arrays() {
    let nodes = parse("\\left( x \\right)").expect("left right");
    match &nodes[0] {
        Node::LeftRight(lr) => {
            assert_eq!(lr.left, "(");
            assert_eq!(lr.right, ")");
        }
        other => panic!("expected left_right, got {other:?}"),
    }
    let nodes = parse("\\begin{bmatrix} 1 \\end{bmatrix}").expect("bmatrix");
    match &nodes[0] {
        Node::Array(array) => {
            assert_eq!(array.left, "[");
            assert_eq!(array.right, "]");
        }
        other => panic!("expected array, got {other:?}"),
    }
}

#[test]
fn mhchem_dispatch() {
    let nodes = parse("\\ce{H2O}").expect("\\ce");
    assert!(matches!(&nodes[0], Node::Group(group) if !group.is_empty()));
    let nodes = parse("\\pu{1 kg}").expect("\\pu");
    assert!(matches!(&nodes[0], Node::Group(_)));
}

#[test]
fn dimen_and_color() {
    assert_eq!(parse_dimen("3mu"), 3.0 / 18.0);
    assert_eq!(parse_dimen("2pt"), 0.2);
    assert_eq!(parse_dimen("1.5"), 1.5);
    assert_eq!(parse_color("#ff0000"), Some(0xffff0000));
    assert_eq!(parse_color("#f00"), Some(0xffff0000));
    assert_eq!(parse_color("red"), Some(0xffdf0030));
    assert_eq!(parse_color("nope"), None);
}
/// Every formula of the `zant-math` example parses (the parser smoke
/// corpus).
#[test]
fn example_formulas_parse() {
    let formulas: [&str; 18] = [
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
    for formula in formulas {
        let nodes = parse(formula).unwrap_or_else(|err| panic!("{formula:?}: {err}"));
        assert!(!nodes.is_empty(), "{formula:?} produced no nodes");
    }
}
