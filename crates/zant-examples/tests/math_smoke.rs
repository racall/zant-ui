//! Headless smoke test for the `zant-math` binary (WP-3.11): the 18-formula
//! gallery is built through [`Runtime`] and painted on a raster canvas; every
//! formula becomes a non-empty retained vector drawing.

use zant::core::node::{NodeData, NodeKind};
use zant::math::block;
use zant::math::layout::{self, Options as LayoutOptions};
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant_examples::math_app::{FORMULAS, MathApp, run_options};

const WIDTH: f32 = 980.0;
const HEIGHT: f32 = 1100.0;

fn frame(runtime: &mut Runtime, renderer: &mut Renderer) {
    let canvas = renderer.begin(1.0).expect("frame canvas");
    runtime.render_frame(canvas, WIDTH, HEIGHT, 1.0);
    renderer.end();
}

fn app() -> (Runtime, Renderer) {
    let mut runtime = Runtime::init(MathApp, run_options());
    let mut renderer = Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("raster");
    frame(&mut runtime, &mut renderer);
    (runtime, renderer)
}

#[test]
fn math_gallery_has_the_eighteen_formulas() {
    assert_eq!(FORMULAS.len(), 18);
    // the Zig array includes the mhchem (\ce / \pu), array, xarrow and
    // colorbox formulas
    assert!(FORMULAS.iter().any(|formula| formula.contains("\\ce{")));
    assert!(FORMULAS.iter().any(|formula| formula.contains("\\pu{")));
    assert!(
        FORMULAS
            .iter()
            .any(|formula| formula.contains("\\begin{pmatrix}"))
    );
    assert!(
        FORMULAS
            .iter()
            .any(|formula| formula.contains("\\xrightarrow"))
    );
    assert!(
        FORMULAS
            .iter()
            .any(|formula| formula.contains("\\colorbox"))
    );
}

#[test]
fn math_gallery_builds_and_paints_one_vector_per_formula() {
    let (runtime, _renderer) = app();
    let vectors: Vec<&NodeData> = runtime
        .tree
        .nodes
        .iter()
        .filter(|node| node.alive && matches!(node.data, NodeData::Vector(_)))
        .map(|node| &node.data)
        .collect();
    assert_eq!(
        vectors.len(),
        FORMULAS.len(),
        "one retained drawing per formula"
    );
    for data in vectors {
        let NodeData::Vector(vector) = data else {
            unreachable!("filtered on Vector")
        };
        assert!(vector.drawing.width > 0.0, "a laid-out formula has a width");
        assert!(
            !vector.drawing.commands.is_empty(),
            "a laid-out formula has drawing commands"
        );
    }
    // the header line of the gallery
    let header = runtime.tree.nodes.iter().any(|node| {
        node.alive
            && matches!(&node.data, NodeData::Text(data)
                if data.content == "Zant math (KaTeX layout, STIX Two Math)")
    });
    assert!(header, "the gallery header text");
    assert!(
        runtime
            .tree
            .nodes
            .iter()
            .any(|node| node.alive && node.visual.background == Some(0xfffc_fcfa))
    );
}

#[test]
fn math_block_is_the_centred_display_wrapper() {
    let node = block(
        "x = 1",
        LayoutOptions {
            size: 17.0,
            max_width: Some(900.0),
            ..LayoutOptions::default()
        },
    );
    // the Zig `math.block` row: 100% wide, centred, half-em vertical margins,
    // one vector child
    assert_eq!(node.element().layout.width_percent, Some(100.0));
    assert_eq!(
        node.element().layout.justify,
        zant::layout::flex::Justify::Center
    );
    assert_eq!(node.element().layout.margin.top, 8.5);
    assert_eq!(node.element().layout.margin.bottom, 8.5);
    assert_eq!(node.element().children.len(), 1);
    assert_eq!(node.element().children[0].element().kind, NodeKind::Vector);
    let drawing = layout::layout(
        "\\ce{2H2 + O2 -> 2H2O}",
        LayoutOptions {
            size: 17.0,
            ..LayoutOptions::default()
        },
    );
    assert!(drawing.width > 0.0 && !drawing.commands.is_empty());
}
