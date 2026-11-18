//! Array and matrix environments, including alignment and rules.

use crate::core::vector::Command;
use crate::math::{
    parser::{self, Node},
    symbols::Class,
};

use super::{Box, Engine, Size, Style, style::Metric};

impl Engine {
    // ── arrays (KaTeX environments/array.js) ────────────────────────────

    pub(super) fn array(&mut self, a: &parser::Array, outer: Style) -> Box {
        let cell_style: Style = if a.small {
            Style::new(Size::Script)
        } else if a.display {
            Style::DISPLAY
        } else {
            Style::TEXT
        };
        let em_size = self.em(outer);
        let arraystretch: f32 = if a.small { 0.5 } else { 1.0 };
        let baselineskip = 1.2 * em_size;
        let arrayskip = arraystretch * baselineskip;
        let strut_h = 0.7 * arrayskip;
        let strut_d = 0.3 * arrayskip;
        let jot: f32 = if a.aligned || a.display {
            0.3 * em_size
        } else {
            0.0
        };
        let ncols = a.cols.len();

        let mut cells: Vec<Vec<Box>> = Vec::with_capacity(a.rows.len());
        let mut col_widths = vec![0.0f32; ncols];
        let mut row_h = vec![0.0f32; a.rows.len()];
        let mut row_d = vec![0.0f32; a.rows.len()];
        for (r, row) in a.rows.iter().enumerate() {
            let mut row_boxes: Vec<Box> = Vec::with_capacity(ncols);
            let mut h = strut_h;
            let mut d = strut_d;
            for c in 0..ncols {
                // aligned: KaTeX starts every right-hand cell with an empty
                // ord so a leading relation (`&=`) keeps its spacing
                let box_ = if c >= row.len() {
                    Box::default()
                } else if a.aligned && c % 2 == 1 {
                    let mut nodes: Vec<Node> = Vec::with_capacity(row[c].len() + 1);
                    nodes.push(Node::Group(Vec::new()));
                    nodes.extend(row[c].iter().cloned());
                    self.list(&nodes, cell_style)
                } else {
                    self.list(&row[c], cell_style)
                };
                col_widths[c] = col_widths[c].max(box_.width);
                h = h.max(box_.height);
                d = d.max(box_.depth);
                row_boxes.push(box_);
            }
            row_h[r] = h;
            row_d[r] = d + jot;
            cells.push(row_boxes);
        }
        // vertical positions
        let mut row_y = vec![0.0f32; a.rows.len()];
        let mut total: f32 = 0.0;
        for r in 0..a.rows.len() {
            total += row_h[r];
            row_y[r] = total;
            total += row_d[r];
            if r < a.row_gaps.len() {
                total += a.row_gaps[r] * em_size;
            }
        }
        let offset = total / 2.0 + self.metric(outer, Metric::AxisHeight);

        // horizontal positions
        let mut col_x = vec![0.0f32; ncols];
        let mut x: f32 = 0.0;
        let sep = a.col_sep * em_size;
        for c in 0..ncols {
            if c > 0 {
                if a.aligned {
                    x += if c % 2 == 0 { 1.0 * em_size } else { 0.0 };
                } else {
                    x += 2.0 * sep;
                }
            }
            col_x[c] = x;
            x += col_widths[c];
        }
        let width = x;
        let mut out: Vec<Command> = Vec::new();
        for r in 0..a.rows.len() {
            let baseline = row_y[r] - offset;
            for c in 0..ncols {
                let box_ = &cells[r][c];
                let slack = col_widths[c] - box_.width;
                let dx: f32 = match a.cols[c] {
                    b'l' => 0.0,
                    b'r' => slack,
                    _ => slack / 2.0,
                };
                self.translated(box_, col_x[c] + dx, baseline, &mut out);
            }
        }
        let theta = self.metric(outer, Metric::DefaultRuleThickness);
        for &before in &a.col_rules {
            let rx = if before == 0 {
                -sep
            } else if before >= ncols {
                width + sep
            } else {
                col_x[before] - sep
            };
            self.rect(rx - theta / 2.0, -offset, theta, total, &mut out);
        }
        for &before in &a.row_rules {
            let mut ry: f32 = 0.0;
            for r in 0..before.min(a.rows.len()) {
                ry += row_h[r] + row_d[r];
            }
            self.rect(0.0, ry - offset - theta / 2.0, width, theta, &mut out);
        }
        let mut box_ = Box {
            width,
            height: offset,
            depth: total - offset,
            cmds: out,
            class: Class::Ord,
            ..Box::default()
        };
        if !a.left.is_empty() || !a.right.is_empty() {
            let total_h = self.left_right_total(box_.height, box_.depth, outer);
            let gap = if !a.left.is_empty() && a.right.is_empty() {
                0.2 * em_size
            } else {
                0.0
            };
            box_ = self.wrap_delims_gapped(box_, &a.left, &a.right, outer, total_h, gap);
        }
        box_
    }

    fn wrap_delims_gapped(
        &self,
        inner: Box,
        left: &str,
        right: &str,
        style: Style,
        total: f32,
        gap: f32,
    ) -> Box {
        let mut padded = inner;
        if gap > 0.0 {
            let mut out: Vec<Command> = Vec::new();
            self.translated(&padded, gap, 0.0, &mut out);
            padded.cmds = out;
            padded.width += gap;
        }
        self.wrap_delims(padded, left, right, style, total)
    }
}
