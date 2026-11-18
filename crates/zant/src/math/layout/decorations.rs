//! Framed and colored boxes, cancellations, and strikeouts.

use crate::core::vector::{Command, Rect as VectorRect};
use crate::math::parser::{self, Node};

use super::{Box, Engine, Style, style::Metric};

impl Engine {
    pub(super) fn color_box(
        &mut self,
        background: u32,
        frame: Option<u32>,
        body: &[Node],
        style: Style,
    ) -> Box {
        let inner = self.list(body, style);
        let pad = 0.3 * self.em(style);
        let theta = self.rule(style);
        let mut out: Vec<Command> = Vec::new();
        let w = inner.width + 2.0 * pad;
        let top = -(inner.height + pad);
        let bottom = inner.depth + pad;
        out.push(Command::Rect(VectorRect {
            x: 0.0,
            y: top,
            width: w,
            height: bottom - top,
            color: background,
            radius: 0.0,
        }));
        if let Some(frame) = frame {
            let saved = self.color;
            self.color = frame;
            self.stroke(
                &[[0.0, top], [w, top], [w, bottom], [0.0, bottom], [0.0, top]],
                theta,
                &mut out,
            );
            self.color = saved;
        }
        self.translated(&inner, pad, 0.0, &mut out);
        Box {
            width: w,
            height: -top,
            depth: bottom,
            cmds: out,
            ..Box::default()
        }
    }

    // ── boxes and cancels ───────────────────────────────────────────────

    pub(super) fn boxed(&mut self, body: &[Node], style: Style) -> Box {
        let inner = self.list(body, style);
        let pad = 0.3 * self.em(style);
        let theta = self.rule(style);
        let mut out: Vec<Command> = Vec::new();
        self.translated(&inner, pad, 0.0, &mut out);
        let w = inner.width + 2.0 * pad;
        let top = -(inner.height + pad);
        let bottom = inner.depth + pad;
        self.stroke(
            &[[0.0, top], [w, top], [w, bottom], [0.0, bottom]],
            theta,
            &mut out,
        );
        // close the rectangle
        self.stroke(&[[0.0, bottom], [0.0, top]], theta, &mut out);
        Box {
            width: w,
            height: -top + theta,
            depth: bottom + theta,
            cmds: out,
            ..Box::default()
        }
    }

    pub(super) fn cancel(&mut self, body: &[Node], kind: parser::CancelKind, style: Style) -> Box {
        let inner = self.list(body, style);
        let theta = self.rule(style);
        let mut out: Vec<Command> = Vec::new();
        self.translated(&inner, 0.0, 0.0, &mut out);
        let w = inner.width;
        let top = -inner.height - 0.05 * self.em(style);
        let bottom = inner.depth + 0.05 * self.em(style);
        match kind {
            parser::CancelKind::Cancel => {
                self.stroke(&[[0.0, bottom], [w, top]], theta, &mut out);
            }
            parser::CancelKind::Bcancel => {
                self.stroke(&[[0.0, top], [w, bottom]], theta, &mut out);
            }
            parser::CancelKind::Xcancel => {
                self.stroke(&[[0.0, bottom], [w, top]], theta, &mut out);
                self.stroke(&[[0.0, top], [w, bottom]], theta, &mut out);
            }
            parser::CancelKind::Sout => {
                self.rect(
                    0.0,
                    -self.metric(style, Metric::AxisHeight) - theta / 2.0,
                    w,
                    theta,
                    &mut out,
                );
            }
        }
        Box {
            width: w,
            height: inner.height,
            depth: inner.depth,
            cmds: out,
            class: inner.class,
            ..Box::default()
        }
    }
}
