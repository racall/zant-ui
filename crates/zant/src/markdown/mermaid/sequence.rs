//! Sequence diagram parsing and drawing.

use super::Options;
use super::canvas::{Canvas, anchor_top, trim_line, trim_rhs, trim_spaces};
use crate::Str;
use crate::core::vector::{self, Drawing};
use crate::renderer::text::Family;

// ── sequence diagrams ─────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NoteSide {
    Over,
    Right,
    Left,
}

struct Message {
    from: usize,
    to: usize,
    text: Str,
    dashed: bool,
    arrow: bool,
    note: bool,
    /// note spans from..to (inclusive) or sits right/left of `from`
    note_side: NoteSide,
}

fn find_participant(names: &mut Vec<Str>, labels: &mut Vec<Str>, name: &str) -> usize {
    let t = name.trim_matches(|c: char| c == ' ' || c == '\t');
    for (i, existing) in names.iter().enumerate() {
        if existing.as_str() == t {
            return i;
        }
    }
    names.push(Str::copy(t));
    labels.push(Str::copy(t));
    names.len() - 1
}

pub(super) fn sequence(source: &str, options: &Options) -> Option<Drawing> {
    let mut names: Vec<Str> = Vec::new();
    let mut labels: Vec<Str> = Vec::new();
    let mut messages: Vec<Message> = Vec::new();
    let mut lines = source.split('\n');
    lines.next();
    for raw in lines {
        let line = trim_line(raw);
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }
        if line.starts_with("participant ") || line.starts_with("actor ") {
            let rest = &line[line.find(' ').expect("participant prefix has a space") + 1..];
            if let Some(p) = rest.find(" as ") {
                let index = find_participant(&mut names, &mut labels, &rest[..p]);
                labels[index] = Str::copy(trim_spaces(&rest[p + 4..]));
            } else {
                find_participant(&mut names, &mut labels, rest);
            }
            continue;
        }
        if line.starts_with("Note ") || line.starts_with("note ") {
            let Some(colon) = line.find(':') else {
                continue;
            };
            let head = &line[5..colon];
            let text = trim_spaces(&line[colon + 1..]);
            let mut side = NoteSide::Over;
            let mut who = head;
            if let Some(rest) = head.strip_prefix("right of ") {
                side = NoteSide::Right;
                who = rest;
            } else if let Some(rest) = head.strip_prefix("left of ") {
                side = NoteSide::Left;
                who = rest;
            } else if let Some(rest) = head.strip_prefix("over ") {
                who = rest;
            }
            let mut parts = who.split(',');
            let a = find_participant(
                &mut names,
                &mut labels,
                parts.next().expect("split yields at least one part"),
            );
            let b = match parts.next() {
                Some(part) => find_participant(&mut names, &mut labels, part),
                None => a,
            };
            messages.push(Message {
                from: a,
                to: b,
                text: Str::copy(text),
                dashed: false,
                arrow: false,
                note: true,
                note_side: side,
            });
            continue;
        }
        const OPS: [&str; 8] = ["-->>", "->>", "--)", "-)", "--x", "-x", "-->", "->"];
        for op in OPS {
            if let Some(p) = line.find(op) {
                let colon = line[p..].find(':').map(|c| p + c).unwrap_or(line.len());
                let lhs = &line[..p];
                let rhs = trim_rhs(&line[p + op.len()..colon]);
                let a = find_participant(&mut names, &mut labels, lhs);
                let b = find_participant(&mut names, &mut labels, rhs);
                let text = if colon < line.len() {
                    trim_spaces(&line[colon + 1..])
                } else {
                    ""
                };
                messages.push(Message {
                    from: a,
                    to: b,
                    text: Str::copy(text),
                    dashed: op.starts_with("--"),
                    arrow: op != "->" && op != "-->",
                    note: false,
                    note_side: NoteSide::Over,
                });
                break;
            }
        }
    }
    if names.is_empty() {
        return None;
    }
    let fs = options.font_size;
    let colors = options.colors;
    let n = names.len();
    let mut box_w: Vec<f32> = Vec::with_capacity(n);
    for label in &labels {
        box_w.push(vector::measure(label, fs, Family::Sans, 400, false).advance + 28.0);
    }
    // spacing between neighbours: fits the widest message between them
    let mut gaps = vec![0.0f32; n];
    for message in &messages {
        if message.note {
            continue;
        }
        let lo = message.from.min(message.to);
        let hi = message.from.max(message.to);
        let w = vector::measure(&message.text, fs * 0.9, Family::Sans, 400, false).advance + 30.0;
        if hi > lo {
            let per = w / (hi - lo) as f32;
            for gap in &mut gaps[lo..hi] {
                *gap = gap.max(per);
            }
        }
    }
    let mut xs = vec![0.0f32; n];
    let mut x: f32 = 8.0 + box_w[0] / 2.0;
    for i in 0..n {
        xs[i] = x;
        if i + 1 < n {
            x += ((box_w[i] + box_w[i + 1]) / 2.0 + 30.0).max(gaps[i]);
        }
    }
    let box_h = fs * 2.4;
    let row_h = fs * 2.6;
    let mut canvas = Canvas::new();
    let rows_start = 8.0 + box_h + 20.0;
    let mut bottom = rows_start;
    for message in &messages {
        bottom += if !message.note && message.from == message.to {
            row_h + 6.0
        } else {
            row_h
        };
    }
    // lifelines and participant boxes (top and bottom) under the messages
    for i in 0..n {
        canvas.line(xs[i], 8.0 + box_h, xs[i], bottom, colors.muted, 1.0, true);
        for top in [8.0f32, bottom] {
            canvas.rect(
                xs[i] - box_w[i] / 2.0,
                top,
                box_w[i],
                box_h,
                colors.node_fill,
                4.0,
            );
            canvas.rounded_border(
                xs[i] - box_w[i] / 2.0,
                top,
                box_w[i],
                box_h,
                4.0,
                colors.node_border,
            );
            canvas.centered_text(xs[i], top + box_h / 2.0, &labels[i], fs, colors.text);
        }
    }
    let mut y: f32 = rows_start;
    for message in &messages {
        if message.note {
            let nw =
                vector::measure(&message.text, fs * 0.88, Family::Sans, 400, false).advance + 20.0;
            let spans = message.note_side == NoteSide::Over && message.from != message.to;
            let lo = xs[message.from].min(xs[message.to]);
            let hi = xs[message.from].max(xs[message.to]);
            let w = if spans { nw.max(hi - lo + 40.0) } else { nw };
            let lx = match message.note_side {
                NoteSide::Right => xs[message.from] + 10.0,
                NoteSide::Left => xs[message.from] - 10.0 - nw,
                NoteSide::Over => (lo + hi) / 2.0 - w / 2.0,
            };
            canvas.rect(lx, y - fs * 0.9, w, fs * 1.8, 0xfffff5ad, 3.0);
            canvas.centered_text(lx + w / 2.0, y, &message.text, fs * 0.88, 0xff333333);
            y += row_h;
            continue;
        }
        let x0 = xs[message.from];
        let x1 = xs[message.to];
        if message.from == message.to {
            let lw: f32 = 34.0;
            canvas.poly(
                &[
                    [x0, y - 4.0],
                    [x0 + lw, y - 4.0],
                    [x0 + lw, y + 10.0],
                    [x0 + 4.0, y + 10.0],
                ],
                colors.edge,
                1.3,
                false,
            );
            canvas.arrow_head(x0, y + 10.0, -1.0, 0.0, colors.edge);
            canvas.text(
                x0 + lw + 6.0,
                y + fs * 0.35,
                &message.text,
                fs * 0.9,
                colors.text,
            );
            y += row_h + 6.0;
            continue;
        }
        let dir: f32 = if x1 > x0 { 1.0 } else { -1.0 };
        canvas.line(x0, y, x1, y, colors.edge, 1.3, message.dashed);
        if message.arrow {
            canvas.arrow_head(x1, y, dir, 0.0, colors.edge);
        } else {
            canvas.poly(
                &[
                    [x1 - dir * 8.0, y - 4.0],
                    [x1, y],
                    [x1 - dir * 8.0, y + 4.0],
                ],
                colors.edge,
                1.3,
                false,
            );
        }
        canvas.centered_text(
            (x0 + x1) / 2.0,
            y - fs * 0.75,
            &message.text,
            fs * 0.9,
            colors.text,
        );
        y += row_h;
    }
    let width = xs[n - 1] + box_w[n - 1] / 2.0 + 8.0 + 60.0;
    Some(anchor_top(canvas.finish(width, bottom + box_h + 8.0)))
}
