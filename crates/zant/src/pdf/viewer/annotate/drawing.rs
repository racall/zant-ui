//! Annotation nodes: highlight / underline / strike boxes, squiggles and ink strokes.

use super::*;

/// One markup / underline / strikethrough box in viewport px.
pub(super) fn markup_box(r: Rect, z: f32, color: u32, radius: f32) -> Node {
    ui::styled(
        ui::column(BoxOptions::default()),
        Style {
            absolute: Some(true),
            left: Some(r.x * z),
            top: Some(r.y * z),
            width: Some(r.width * z),
            height: Some(r.height * z),
            background: Some(color),
            radius: Some(radius),
            ..Default::default()
        },
    )
}

/// A zigzag polyline under the text baseline.
pub(super) fn squiggle(r: Rect, z: f32, color: u32) -> Node {
    let w = r.width * z;
    let amp = (r.height * z * 0.05).max(1.0);
    let step = (amp * 2.0).max(2.0);
    let n = ((w / step).ceil() + 1.0).max(2.0) as usize;
    let mut points = Vec::with_capacity(n);
    for i in 0..n {
        let x = w.min(i as f32 * step);
        points.push([x, if i % 2 == 0 { 0.0 } else { amp * 2.0 }]);
    }
    let drawing = Drawing {
        width: w,
        ascent: 0.0,
        descent: amp * 2.0,
        commands: vec![Command::Path(Path {
            points,
            color: color | 0xff00_0000,
            stroke_width: (amp * 0.8).max(1.0),
            closed: false,
        })],
        hash: 0,
    };
    ui::styled(
        ui::vector(ui::VectorOptions {
            drawing: Rc::new(drawing),
            ..Default::default()
        }),
        Style {
            absolute: Some(true),
            left: Some(r.x * z),
            top: Some(r.bottom() * z - amp * 2.0),
            ..Default::default()
        },
    )
}

/// Freehand strokes as one vector path per stroke.
pub(crate) fn ink_node(
    strokes: &[Vec<Point>],
    color: u32,
    width: f32,
    z: f32,
    size: [f32; 2],
    key: Option<Str>,
) -> Node {
    let mut commands = Vec::with_capacity(strokes.len());
    for stroke in strokes {
        let mut points = Vec::with_capacity(stroke.len().max(2));
        for point in stroke {
            points.push([point[0] * z, point[1] * z]);
        }
        // a single tap draws a dot
        if stroke.len() == 1 {
            points.push([stroke[0][0] * z + 0.01, stroke[0][1] * z]);
        }
        commands.push(Command::Path(Path {
            points,
            color,
            stroke_width: (width * z).max(1.0),
            closed: false,
        }));
    }
    let drawing = Drawing {
        width: size[0],
        ascent: 0.0,
        descent: size[1],
        commands,
        hash: 0,
    };
    ui::styled(
        ui::vector(ui::VectorOptions {
            key,
            drawing: Rc::new(drawing),
            ..Default::default()
        }),
        Style {
            absolute: Some(true),
            left: Some(0.0),
            top: Some(0.0),
            ..Default::default()
        },
    )
}
