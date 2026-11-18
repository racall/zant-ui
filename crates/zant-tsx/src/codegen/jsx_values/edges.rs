//! Edge props (padding, margin, border sides): per-side values and their `Edges` code.

use super::*;

/// One side of an `Edges` field.
#[derive(Clone, Debug)]
pub struct Edge {
    pub code: String,
    /// The original source expression, for the "all sides share one value"
    /// test (`ui.inset`). Compared by address.
    pub source: usize,
}

/// Use `ui::inset` only when every side shares the same original expression.
pub(super) fn edges_value(edges: &[Option<Edge>; 4]) -> String {
    let top = edges[0].as_ref().map_or("0.0", |edge| edge.code.as_str());
    let right = edges[1].as_ref().map_or("0.0", |edge| edge.code.as_str());
    let bottom = edges[2].as_ref().map_or("0.0", |edge| edge.code.as_str());
    let left = edges[3].as_ref().map_or("0.0", |edge| edge.code.as_str());
    let all_sides = edges.iter().all(Option::is_some);
    if all_sides {
        let source = edges[0].as_ref().expect("edge").source;
        let same_source = edges
            .iter()
            .all(|edge| edge.as_ref().is_some_and(|edge| edge.source == source));
        if same_source {
            return inset(top);
        }
    }
    format!(
        "zant::core::geometry::Edges {{ top: {top}, right: {right}, bottom: {bottom}, left: {left} }}"
    )
}

/// Object edge keys share the later-wins behavior of padding aliases.
pub fn edges(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    prop: &Prop,
    e: &ast::Expr,
    root: &mut Fields,
) -> Result<()> {
    let fields = root.group(prop.parent);
    if let ast::ExprData::Object(properties) = &e.data {
        for property in properties {
            if property.spread {
                return fail(g, property.value.loc, "object spread is not supported yet");
            }
            let Some(selected) = edge_sides(&property.key.name) else {
                return fail(
                    g,
                    property.key.loc,
                    "unknown edge; valid keys: top, right, bottom, left, horizontal, vertical",
                );
            };
            let intersection = Sides {
                top: selected.top && prop.sides.top,
                right: selected.right && prop.sides.right,
                bottom: selected.bottom && prop.sides.bottom,
                left: selected.left && prop.sides.left,
            };
            sides(g, c, fields, prop.field, intersection, &property.value)?;
        }
        // An empty object still writes an Edges value whose sides default to zero.
        fields.entry(prop.field);
        return Ok(());
    }
    sides(g, c, fields, prop.field, prop.sides, e)
}

/// One source expression applied to every side can later become ui.inset.
pub(super) fn sides(
    g: &mut Generator<'_>,
    c: &mut scope::Context,
    fields: &mut Fields,
    field: &str,
    selected: Sides,
    e: &ast::Expr,
) -> Result<()> {
    let value = expr::emit(g, c, e, Some(Type::F32), false)?.code;
    let source = expression_address(e);
    let entry = fields.entry(field);
    let mask = [selected.top, selected.right, selected.bottom, selected.left];
    for (index, set) in mask.iter().enumerate() {
        if *set {
            entry.edges[index] = Some(Edge {
                code: value.clone(),
                source,
            });
        }
    }
    Ok(())
}

/// Accept the documented side names rather than arbitrary object fields.
pub(super) fn edge_sides(name: &[u8]) -> Option<Sides> {
    let sides = match name {
        b"top" => Sides {
            top: true,
            ..Sides::NONE
        },
        b"right" => Sides {
            right: true,
            ..Sides::NONE
        },
        b"bottom" => Sides {
            bottom: true,
            ..Sides::NONE
        },
        b"left" => Sides {
            left: true,
            ..Sides::NONE
        },
        b"horizontal" => Sides::HORIZONTAL,
        b"vertical" => Sides::VERTICAL,
        _ => return None,
    };
    Some(sides)
}
