//! Native Mermaid renderer for the diagram types AI answers and notes use
//! most: `flowchart`/`graph`, `sequenceDiagram` and `pie`. Produces a
//! [`vector::Drawing`]; unsupported diagrams return `None` and are shown as code.

use crate::Str;
use crate::core::hash::Wyhash;
use crate::core::vector::{self, Command, Drawing, Path, Point, Rect, Text};
use crate::renderer::text::Family;

/// Theme colours supplied by the markdown renderer (see
/// `markdown::render::diagram`: text / node fill / node border / edge / label
/// background / muted).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Colors {
    pub text: u32,
    pub node_fill: u32,
    pub node_border: u32,
    pub edge: u32,
    pub label_bg: u32,
    pub muted: u32,
}

/// `colors` is mandatory, so there is no `Default` impl: start from
/// [`Options::new`].
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub font_size: f32,
    pub colors: Colors,
    pub max_width: f32,
}

impl Options {
    /// The default options with `colors`: 14 px text, diagrams wider than
    /// 640 px shrink to fit.
    pub fn new(colors: Colors) -> Options {
        Options {
            font_size: 14.0,
            colors,
            max_width: 640.0,
        }
    }
}

const PALETTE: [u32; 10] = [
    0xff4e79a7, 0xfff28e2b, 0xffe15759, 0xff76b7b2, 0xff59a14f, 0xffedc948, 0xffb07aa1, 0xffff9da7,
    0xff9c755f, 0xffbab0ac,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Flowchart,
    Sequence,
    Pie,
}

pub fn detect(source: &str) -> Option<Kind> {
    let mut in_front_matter = false;
    for raw in source.split('\n') {
        let line = trim_line(raw);
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }
        if line == "---" {
            in_front_matter = !in_front_matter;
            continue;
        }
        if in_front_matter {
            continue;
        }
        let word_end = line.find([' ', '\t']).unwrap_or(line.len());
        let word = &line[..word_end];
        if word == "graph" || word == "flowchart" {
            return Some(Kind::Flowchart);
        }
        if word == "sequenceDiagram" {
            return Some(Kind::Sequence);
        }
        if word == "pie" {
            return Some(Kind::Pie);
        }
        return None;
    }
    None
}

pub fn render(source: &str, options: Options) -> Option<Drawing> {
    let kind = detect(source)?;
    let mut drawing = match kind {
        Kind::Flowchart => flowchart(source, &options),
        Kind::Sequence => sequence(source, &options),
        Kind::Pie => pie(source, &options),
    }?;
    // wide diagrams shrink to the column (like display math)
    if options.max_width > 0.0 && drawing.width > options.max_width {
        drawing = drawing.scaled(options.max_width / drawing.width);
    }
    let mut hasher = Wyhash::init(0x6d6d64);
    hasher.update(source.as_bytes());
    // The options are hashed as their little-endian bytes: font_size, then
    // the six colours in declaration order, then max_width.
    hasher.update(&options.font_size.to_le_bytes());
    for color in colors_in_memory_order(options.colors) {
        hasher.update(&color.to_le_bytes());
    }
    hasher.update(&options.max_width.to_le_bytes());
    drawing.hash = hasher.finish();
    Some(drawing)
}

/// The colours in `Colors` declaration order (the order `mem::asBytes` saw).
fn colors_in_memory_order(colors: Colors) -> [u32; 6] {
    [
        colors.text,
        colors.node_fill,
        colors.node_border,
        colors.edge,
        colors.label_bg,
        colors.muted,
    ]
}

// ── shared drawing helpers ────────────────────────────────────────────────

struct Canvas {
    commands: Vec<Command>,
}

impl Canvas {
    fn new() -> Canvas {
        Canvas {
            commands: Vec::new(),
        }
    }

    fn text(&mut self, x: f32, baseline: f32, text: &str, size: f32, color: u32) {
        self.commands.push(Command::Text(Text {
            x,
            y: baseline,
            text: Str::copy(text),
            size,
            family: Family::Sans,
            weight: 400,
            italic: false,
            color,
            scale_y: 1.0,
        }));
    }

    fn centered_text(&mut self, cx: f32, cy: f32, text: &str, size: f32, color: u32) {
        let m = vector::measure(text, size, Family::Sans, 400, false);
        self.text(
            cx - m.advance / 2.0,
            cy + (m.font_ascent - m.font_descent) / 2.0,
            text,
            size,
            color,
        );
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: u32, radius: f32) {
        self.commands.push(Command::Rect(Rect {
            x,
            y,
            width: w,
            height: h,
            color,
            radius,
        }));
    }

    fn poly(&mut self, points: &[Point], color: u32, stroke_width: f32, closed: bool) {
        self.commands.push(Command::Path(Path {
            points: points.to_vec(),
            color,
            stroke_width,
            closed,
        }));
    }

    #[allow(clippy::too_many_arguments)]
    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: u32, width: f32, dashed: bool) {
        if !dashed {
            return self.poly(&[[x0, y0], [x1, y1]], color, width, false);
        }
        let len = ((x1 - x0) * (x1 - x0) + (y1 - y0) * (y1 - y0)).sqrt();
        if len < 0.1 {
            return;
        }
        let dash: f32 = 5.0;
        let mut t: f32 = 0.0;
        while t < len {
            let e = (t + dash).min(len);
            self.poly(
                &[
                    [x0 + (x1 - x0) * t / len, y0 + (y1 - y0) * t / len],
                    [x0 + (x1 - x0) * e / len, y0 + (y1 - y0) * e / len],
                ],
                color,
                width,
                false,
            );
            t += dash * 2.0;
        }
    }

    fn arrow_head(&mut self, x: f32, y: f32, dx: f32, dy: f32, color: u32) {
        let len = 0.001f32.max((dx * dx + dy * dy).sqrt());
        let ux = dx / len;
        let uy = dy / len;
        let s: f32 = 8.0;
        self.poly(
            &[
                [x, y],
                [x - ux * s - uy * s * 0.5, y - uy * s + ux * s * 0.5],
                [x - ux * s + uy * s * 0.5, y - uy * s - ux * s * 0.5],
            ],
            color,
            0.0,
            true,
        );
    }

    fn rounded_border(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, color: u32) {
        // approximate rounded rectangle outline
        let mut points: Vec<Point> = Vec::new();
        let corners: [[f32; 3]; 4] = [
            [x + w - r, y + r, -std::f32::consts::FRAC_PI_2],
            [x + w - r, y + h - r, 0.0],
            [x + r, y + h - r, std::f32::consts::FRAC_PI_2],
            [x + r, y + r, std::f32::consts::PI],
        ];
        for corner in corners {
            let mut k: usize = 0;
            while k <= 6 {
                let a = corner[2] + k as f32 / 6.0 * std::f32::consts::FRAC_PI_2;
                points.push([corner[0] + a.cos() * r, corner[1] + a.sin() * r]);
                k += 1;
            }
        }
        self.poly(&points, color, 1.2, true);
    }

    /// Finishes with the top-left at (0,0); `ascent` = full height.
    fn finish(self, width: f32, height: f32) -> Drawing {
        Drawing {
            width,
            ascent: height,
            descent: 0.0,
            commands: self.commands,
            hash: 0,
        }
    }
}

/// Shifts every command up by `height` so the drawing's top is y = -ascent
/// (drawings are baseline-anchored; diagrams sit entirely above it).
fn anchor_top(drawing: Drawing) -> Drawing {
    let ascent = drawing.ascent;
    let commands = drawing
        .commands
        .iter()
        .map(|command| vector::translate(command, 0.0, -ascent))
        .collect();
    Drawing {
        width: drawing.width,
        ascent: drawing.ascent,
        descent: drawing.descent,
        commands,
        hash: drawing.hash,
    }
}

fn trim_line(value: &str) -> &str {
    value.trim_matches(|c: char| c == ' ' || c == '\t' || c == '\r')
}

fn trim_spaces(value: &str) -> &str {
    value.trim_matches(' ')
}

fn trim_leading(value: &str) -> &str {
    value.trim_start_matches([' ', '\t'])
}

fn trim_rhs(value: &str) -> &str {
    value.trim_matches(|c: char| c == ' ' || c == '+' || c == '-')
}

fn strip_quotes(text: &str) -> &str {
    let s = text.trim_matches(|c: char| c == ' ' || c == '\t');
    let bytes = s.as_bytes();
    if s.len() >= 2 && (bytes[0] == b'"' || bytes[0] == b'\'') && bytes[s.len() - 1] == bytes[0] {
        return &s[1..s.len() - 1];
    }
    s
}

// ── flowchart ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Rect,
    Round,
    Stadium,
    Circle,
    Diamond,
    Hexagon,
    Subroutine,
    Cylinder,
    Asymmetric,
}

struct FNode {
    id: Str,
    label: Str,
    shape: Shape,
    layer: usize,
    order: f32,
    w: f32,
    h: f32,
    x: f32,
    y: f32,
}

impl FNode {
    fn new(id: &str) -> FNode {
        FNode {
            id: Str::copy(id),
            label: Str::copy(id),
            shape: Shape::Rect,
            layer: 0,
            order: 0.0,
            w: 0.0,
            h: 0.0,
            x: 0.0,
            y: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EdgeStyle {
    Solid,
    Dotted,
    Thick,
}

struct FEdge {
    from: usize,
    to: usize,
    label: Str,
    arrow: bool,
    style: EdgeStyle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    Td,
    Bt,
    Lr,
    Rl,
}

#[derive(Default)]
struct Flow {
    nodes: Vec<FNode>,
    edges: Vec<FEdge>,
}

struct Link<'a> {
    arrow: bool,
    style: EdgeStyle,
    label: Str,
    rest: &'a str,
}

impl Flow {
    fn node_index(&mut self, id: &str) -> usize {
        for (i, node) in self.nodes.iter().enumerate() {
            if node.id.as_str() == id {
                return i;
            }
        }
        self.nodes.push(FNode::new(id));
        self.nodes.len() - 1
    }

    /// Parses `id`, `id[label]`, `id(label)`, ... Returns node index and rest.
    fn parse_node<'a>(&mut self, text: &'a str) -> Option<(usize, &'a str)> {
        let t = trim_leading(text);
        let bytes = t.as_bytes();
        let mut i: usize = 0;
        while i < t.len() {
            let c = bytes[i];
            let hyphen = c == b'-'
                && i + 1 < t.len()
                && bytes[i + 1].is_ascii_alphanumeric()
                && !(i + 1 < t.len() && bytes[i + 1] == b'-');
            if c.is_ascii_alphanumeric() || c == b'_' || c >= 0x80 || hyphen {
                i += 1;
            } else {
                break;
            }
        }
        if i == 0 {
            return None;
        }
        let id = &t[..i];
        let index = self.node_index(id);
        let mut rest = &t[i..];
        const OPENS: [(&str, &str, Shape); 12] = [
            ("(((", ")))", Shape::Circle),
            ("((", "))", Shape::Circle),
            ("([", "])", Shape::Stadium),
            ("[[", "]]", Shape::Subroutine),
            ("[(", ")]", Shape::Cylinder),
            ("{{", "}}", Shape::Hexagon),
            ("[/", "/]", Shape::Rect),
            ("[\\", "\\]", Shape::Rect),
            ("(", ")", Shape::Round),
            ("[", "]", Shape::Rect),
            ("{", "}", Shape::Diamond),
            (">", "]", Shape::Asymmetric),
        ];
        for (open, close, shape) in OPENS {
            if rest.starts_with(open) {
                let Some(relative) = rest[open.len()..].find(close) else {
                    continue;
                };
                let close_at = open.len() + relative;
                let node = &mut self.nodes[index];
                node.label = clean_label(&rest[open.len()..close_at]);
                node.shape = shape;
                rest = &rest[close_at + close.len()..];
                break;
            }
        }
        // :::class suffix
        if rest.starts_with(":::") {
            let mut j: usize = 3;
            while j < rest.len()
                && (rest.as_bytes()[j].is_ascii_alphanumeric()
                    || rest.as_bytes()[j] == b'_'
                    || rest.as_bytes()[j] == b'-')
            {
                j += 1;
            }
            rest = &rest[j..];
        }
        Some((index, rest))
    }

    fn parse_link<'a>(&self, text: &'a str) -> Option<Link<'a>> {
        let t = trim_leading(text);
        if t.len() < 2 {
            return None;
        }
        let bytes = t.as_bytes();
        let mut label = Str::default();
        // inline text form: -- text --> / == text ==> / -. text .->
        if t.starts_with("-- ") || t.starts_with("== ") || t.starts_with("-. ") {
            const CLOSERS: [&str; 6] = ["-->", "---", "==>", "===", ".->", ".-"];
            let mut best: Option<usize> = None;
            let mut best_len: usize = 0;
            for closer in CLOSERS {
                if let Some(relative) = t[3..].find(closer) {
                    let at = 3 + relative;
                    let replace = match best {
                        None => true,
                        Some(best_at) => at < best_at,
                    };
                    if replace {
                        best = Some(at);
                        best_len = closer.len();
                    }
                }
            }
            if let Some(at) = best {
                label = clean_label(trim_spaces(&t[3..at]));
                let style = if bytes[0] == b'=' {
                    EdgeStyle::Thick
                } else if bytes[1] == b'.' {
                    EdgeStyle::Dotted
                } else {
                    EdgeStyle::Solid
                };
                let arrow = bytes[at + best_len - 1] == b'>';
                return Some(Link {
                    arrow,
                    style,
                    label,
                    rest: &t[at + best_len..],
                });
            }
        }
        let mut i: usize = 0;
        while i < t.len() && b"-=.<>ox".contains(&bytes[i]) {
            // stop "o"/"x" heads that start an identifier
            if (bytes[i] == b'o' || bytes[i] == b'x')
                && i + 1 < t.len()
                && bytes[i + 1].is_ascii_alphanumeric()
            {
                break;
            }
            i += 1;
        }
        let op = &t[..i];
        if op.len() < 2 || (!op.contains('-') && !op.contains('=')) {
            return None;
        }
        let mut rest = &t[i..];
        if !rest.is_empty() && rest.as_bytes()[0] == b'|' {
            let relative = rest[1..].find('|')?;
            let close = 1 + relative;
            label = clean_label(&rest[1..close]);
            rest = &rest[close + 1..];
        }
        Some(Link {
            arrow: op.bytes().any(|b| b == b'>' || b == b'o' || b == b'x'),
            style: if op.contains('=') {
                EdgeStyle::Thick
            } else if op.contains('.') {
                EdgeStyle::Dotted
            } else {
                EdgeStyle::Solid
            },
            label,
            rest,
        })
    }

    /// `A & B --> C --> D`
    fn parse_statement(&mut self, statement: &str) {
        let mut rest = statement;
        let mut current: Vec<usize> = Vec::new();
        loop {
            let Some((index, tail)) = self.parse_node(rest) else {
                return;
            };
            current.push(index);
            rest = trim_leading(tail);
            if !rest.is_empty() && rest.as_bytes()[0] == b'&' {
                rest = &rest[1..];
                continue;
            }
            break;
        }
        loop {
            let Some(link) = self.parse_link(rest) else {
                return;
            };
            rest = link.rest;
            let mut targets: Vec<usize> = Vec::new();
            loop {
                let Some((index, tail)) = self.parse_node(rest) else {
                    return;
                };
                targets.push(index);
                rest = trim_leading(tail);
                if !rest.is_empty() && rest.as_bytes()[0] == b'&' {
                    rest = &rest[1..];
                    continue;
                }
                break;
            }
            for from in &current {
                for to in &targets {
                    self.edges.push(FEdge {
                        from: *from,
                        to: *to,
                        label: link.label.clone(),
                        arrow: link.arrow,
                        style: link.style,
                    });
                }
            }
            current = targets;
        }
    }
}

fn clean_label(raw: &str) -> Str {
    let mut t = strip_quotes(raw);
    let bytes = t.as_bytes();
    // markdown strings "`**x**`" and <br> line breaks: keep text simple
    if t.len() >= 2 && bytes[0] == b'`' && bytes[t.len() - 1] == b'`' {
        t = &t[1..t.len() - 1];
    }
    if !t.contains("<br") && !t.contains('*') {
        return Str::copy(t);
    }
    let bytes = t.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(t.len());
    let mut i: usize = 0;
    while i < t.len() {
        if bytes[i] == b'<'
            && let Some(relative) = t[i..].find('>')
        {
            out.push(b' ');
            i = i + relative + 1;
            continue;
        }
        if bytes[i] != b'*' {
            out.push(bytes[i]);
        }
        i += 1;
    }
    Str::from(String::from_utf8(out).expect("clean_label preserves valid UTF-8"))
}

fn dfs_visit(flow: &Flow, state: &mut [u8], back: &mut [bool], v: usize) {
    state[v] = 1;
    for (k, edge) in flow.edges.iter().enumerate() {
        if edge.from != v {
            continue;
        }
        if state[edge.to] == 1 {
            back[k] = true;
        } else if state[edge.to] == 0 {
            dfs_visit(flow, state, back, edge.to);
        }
    }
    state[v] = 2;
}

fn flowchart(source: &str, options: &Options) -> Option<Drawing> {
    let mut flow = Flow::default();
    let mut direction = Direction::Td;
    let mut header_seen = false;
    for raw in source.split(['\n', ';']) {
        let line = trim_line(raw);
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }
        if !header_seen {
            header_seen = true;
            let mut words = line.split(' ').filter(|word| !word.is_empty());
            let _ = words.next();
            if let Some(dir) = words.next() {
                if dir.eq_ignore_ascii_case("LR") {
                    direction = Direction::Lr;
                } else if dir.eq_ignore_ascii_case("RL") {
                    direction = Direction::Rl;
                } else if dir.eq_ignore_ascii_case("BT") {
                    direction = Direction::Bt;
                }
            }
            continue;
        }
        const SKIP: [&str; 8] = [
            "classDef",
            "class ",
            "style ",
            "linkStyle",
            "click ",
            "subgraph",
            "end",
            "direction",
        ];
        if SKIP.iter().any(|prefix| line.starts_with(prefix)) {
            continue;
        }
        flow.parse_statement(line);
    }
    if flow.nodes.is_empty() {
        return None;
    }
    let fs = options.font_size;

    // measure nodes
    for node in &mut flow.nodes {
        let m = vector::measure(&node.label, fs, Family::Sans, 400, false);
        node.w = (m.advance + 28.0).max(56.0);
        node.h = fs * 2.6;
        match node.shape {
            Shape::Diamond => {
                node.w = (node.w * 1.35).max(node.h * 1.6);
                node.h *= 1.45;
            }
            Shape::Circle => {
                node.w = node.w.max(node.h);
                node.h = node.w;
            }
            Shape::Hexagon => node.w += 16.0,
            _ => {}
        }
    }

    // layering: longest path, ignoring back edges found by DFS
    let mut back = vec![false; flow.edges.len()];
    {
        let mut state = vec![0u8; flow.nodes.len()]; // 0 new, 1 active, 2 done
        for v in 0..flow.nodes.len() {
            if state[v] == 0 {
                dfs_visit(&flow, &mut state, &mut back, v);
            }
        }
    }
    let mut changed = true;
    let mut guard: usize = 0;
    while changed && guard < flow.nodes.len() + 2 {
        guard += 1;
        changed = false;
        for (k, edge) in flow.edges.iter().enumerate() {
            if back[k] || edge.from == edge.to {
                continue;
            }
            let from_layer = flow.nodes[edge.from].layer;
            if flow.nodes[edge.to].layer < from_layer + 1 {
                flow.nodes[edge.to].layer = from_layer + 1;
                changed = true;
            }
        }
    }
    let mut layer_count: usize = 0;
    for node in &flow.nodes {
        layer_count = layer_count.max(node.layer + 1);
    }

    // ordering: appearance order, then two barycenter sweeps
    for (i, node) in flow.nodes.iter_mut().enumerate() {
        node.order = i as f32;
    }
    for _ in 0..2 {
        for v in 0..flow.nodes.len() {
            let layer = flow.nodes[v].layer;
            let mut sum: f32 = 0.0;
            let mut count: f32 = 0.0;
            for edge in &flow.edges {
                if edge.to == v && flow.nodes[edge.from].layer < layer {
                    sum += flow.nodes[edge.from].order;
                    count += 1.0;
                }
            }
            if count > 0.0 {
                flow.nodes[v].order = sum / count + v as f32 * 0.001;
            }
        }
    }

    let horizontal = direction == Direction::Lr || direction == Direction::Rl;
    let rank_gap: f32 = if horizontal { 56.0 } else { 48.0 };
    let node_gap: f32 = 28.0;
    // per layer: sorted members, extent along the cross axis
    let mut layer_main = vec![0.0f32; layer_count]; // thickness of layer along main axis
    for node in &flow.nodes {
        let thickness = if horizontal { node.w } else { node.h };
        layer_main[node.layer] = layer_main[node.layer].max(thickness);
    }
    let mut cross_max: f32 = 0.0;
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); layer_count];
    for (i, node) in flow.nodes.iter().enumerate() {
        members[node.layer].push(i);
    }
    for layer in &mut members {
        layer.sort_by(|a, b| flow.nodes[*a].order.total_cmp(&flow.nodes[*b].order));
        let mut extent: f32 = 0.0;
        for (k, idx) in layer.iter().enumerate() {
            if k > 0 {
                extent += node_gap;
            }
            extent += if horizontal {
                flow.nodes[*idx].h
            } else {
                flow.nodes[*idx].w
            };
        }
        cross_max = cross_max.max(extent);
    }
    let mut main_pos: f32 = 8.0;
    for (layer, layer_members) in members.iter().enumerate() {
        let mut extent: f32 = 0.0;
        for (k, idx) in layer_members.iter().enumerate() {
            if k > 0 {
                extent += node_gap;
            }
            extent += if horizontal {
                flow.nodes[*idx].h
            } else {
                flow.nodes[*idx].w
            };
        }
        let mut cross = 8.0 + (cross_max - extent) / 2.0;
        for idx in layer_members {
            let node = &mut flow.nodes[*idx];
            if horizontal {
                node.x = main_pos + (layer_main[layer] - node.w) / 2.0;
                node.y = cross;
                cross += node.h + node_gap;
            } else {
                node.y = main_pos + (layer_main[layer] - node.h) / 2.0;
                node.x = cross;
                cross += node.w + node_gap;
            }
        }
        main_pos += layer_main[layer] + rank_gap;
    }
    let mut width: f32 = 0.0;
    let mut height: f32 = 0.0;
    for node in &flow.nodes {
        width = width.max(node.x + node.w + 8.0);
        height = height.max(node.y + node.h + 8.0);
    }
    // flip for BT / RL
    if direction == Direction::Bt || direction == Direction::Rl {
        for node in &mut flow.nodes {
            if direction == Direction::Bt {
                node.y = height - node.y - node.h;
            } else {
                node.x = width - node.x - node.w;
            }
        }
    }

    let mut canvas = Canvas::new();
    let colors = options.colors;
    let mut back_lane: f32 = 0.0;
    // edges first (under nodes)
    for (k, edge) in flow.edges.iter().enumerate() {
        let a = &flow.nodes[edge.from];
        let b = &flow.nodes[edge.to];
        let lw: f32 = if edge.style == EdgeStyle::Thick {
            2.6
        } else {
            1.3
        };
        let dashed = edge.style == EdgeStyle::Dotted;
        let (label_x, label_y);
        if back[k] || edge.from == edge.to {
            // route around the side of the diagram
            back_lane += 14.0;
            if horizontal {
                let y = height + back_lane - 6.0;
                let ax = a.x + a.w / 2.0;
                let bx = b.x + b.w / 2.0;
                canvas.line(ax, a.y + a.h, ax, y, colors.edge, lw, dashed);
                canvas.line(ax, y, bx, y, colors.edge, lw, dashed);
                canvas.line(bx, y, bx, b.y + b.h, colors.edge, lw, dashed);
                if edge.arrow {
                    canvas.arrow_head(bx, b.y + b.h, 0.0, -1.0, colors.edge);
                }
                label_x = (ax + bx) / 2.0;
                label_y = y;
            } else {
                let x = width + back_lane - 6.0;
                let ay = a.y + a.h / 2.0;
                let by = b.y + b.h / 2.0;
                canvas.line(a.x + a.w, ay, x, ay, colors.edge, lw, dashed);
                canvas.line(x, ay, x, by, colors.edge, lw, dashed);
                canvas.line(x, by, b.x + b.w, by, colors.edge, lw, dashed);
                if edge.arrow {
                    canvas.arrow_head(b.x + b.w, by, -1.0, 0.0, colors.edge);
                }
                label_x = x;
                label_y = (ay + by) / 2.0;
            }
        } else if horizontal {
            let forward = b.x >= a.x;
            let x0 = if forward { a.x + a.w } else { a.x };
            let x1 = if forward { b.x } else { b.x + b.w };
            let y0 = a.y + a.h / 2.0;
            let y1 = b.y + b.h / 2.0;
            let mid = (x0 + x1) / 2.0;
            if (y0 - y1).abs() < 1.0 {
                canvas.line(x0, y0, x1, y1, colors.edge, lw, dashed);
            } else {
                canvas.poly(
                    &[[x0, y0], [mid, y0], [mid, y1], [x1, y1]],
                    colors.edge,
                    lw,
                    false,
                );
            }
            if edge.arrow {
                canvas.arrow_head(x1, y1, if forward { 1.0 } else { -1.0 }, 0.0, colors.edge);
            }
            label_x = mid;
            label_y = (y0 + y1) / 2.0;
        } else {
            let down = b.y >= a.y;
            let x0 = a.x + a.w / 2.0;
            let x1 = b.x + b.w / 2.0;
            let y0 = if down { a.y + a.h } else { a.y };
            let y1 = if down { b.y } else { b.y + b.h };
            let mid = y1
                - (if down {
                    rank_gap / 2.0
                } else {
                    -rank_gap / 2.0
                });
            if (x0 - x1).abs() < 1.0 {
                canvas.line(x0, y0, x1, y1, colors.edge, lw, dashed);
            } else {
                canvas.poly(
                    &[[x0, y0], [x0, mid], [x1, mid], [x1, y1]],
                    colors.edge,
                    lw,
                    false,
                );
            }
            if edge.arrow {
                canvas.arrow_head(x1, y1, 0.0, if down { 1.0 } else { -1.0 }, colors.edge);
            }
            label_x = (x0 + x1) / 2.0;
            label_y = if (x0 - x1).abs() < 1.0 {
                (y0 + y1) / 2.0
            } else {
                mid
            };
        }
        if !edge.label.is_empty() {
            let m = vector::measure(&edge.label, fs * 0.86, Family::Sans, 400, false);
            canvas.rect(
                label_x - m.advance / 2.0 - 4.0,
                label_y - fs * 0.62,
                m.advance + 8.0,
                fs * 1.24,
                colors.label_bg,
                3.0,
            );
            canvas.centered_text(label_x, label_y, &edge.label, fs * 0.86, colors.muted);
        }
    }
    // nodes
    for node in &flow.nodes {
        match node.shape {
            Shape::Diamond => {
                let points = [
                    [node.x + node.w / 2.0, node.y],
                    [node.x + node.w, node.y + node.h / 2.0],
                    [node.x + node.w / 2.0, node.y + node.h],
                    [node.x, node.y + node.h / 2.0],
                ];
                canvas.poly(&points, colors.node_fill, 0.0, true);
                canvas.poly(&points, colors.node_border, 1.2, true);
            }
            Shape::Hexagon => {
                let d: f32 = 12.0;
                let points = [
                    [node.x + d, node.y],
                    [node.x + node.w - d, node.y],
                    [node.x + node.w, node.y + node.h / 2.0],
                    [node.x + node.w - d, node.y + node.h],
                    [node.x + d, node.y + node.h],
                    [node.x, node.y + node.h / 2.0],
                ];
                canvas.poly(&points, colors.node_fill, 0.0, true);
                canvas.poly(&points, colors.node_border, 1.2, true);
            }
            Shape::Asymmetric => {
                let d: f32 = 12.0;
                let points = [
                    [node.x, node.y],
                    [node.x + node.w, node.y],
                    [node.x + node.w, node.y + node.h],
                    [node.x, node.y + node.h],
                    [node.x + d, node.y + node.h / 2.0],
                ];
                canvas.poly(&points, colors.node_fill, 0.0, true);
                canvas.poly(&points, colors.node_border, 1.2, true);
            }
            _ => {
                let r: f32 = match node.shape {
                    Shape::Round => 8.0,
                    Shape::Stadium => node.h / 2.0,
                    Shape::Circle => node.w / 2.0,
                    Shape::Cylinder => 10.0,
                    _ => 3.0,
                };
                canvas.rect(node.x, node.y, node.w, node.h, colors.node_fill, r);
                canvas.rounded_border(
                    node.x,
                    node.y,
                    node.w,
                    node.h,
                    r.min(node.w.min(node.h) / 2.0),
                    colors.node_border,
                );
                if node.shape == Shape::Subroutine {
                    canvas.line(
                        node.x + 7.0,
                        node.y,
                        node.x + 7.0,
                        node.y + node.h,
                        colors.node_border,
                        1.2,
                        false,
                    );
                    canvas.line(
                        node.x + node.w - 7.0,
                        node.y,
                        node.x + node.w - 7.0,
                        node.y + node.h,
                        colors.node_border,
                        1.2,
                        false,
                    );
                }
            }
        }
        canvas.centered_text(
            node.x + node.w / 2.0,
            node.y + node.h / 2.0,
            &node.label,
            fs,
            colors.text,
        );
    }
    if horizontal {
        height += back_lane;
    } else {
        width += back_lane;
    }
    Some(anchor_top(canvas.finish(width, height)))
}

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

fn sequence(source: &str, options: &Options) -> Option<Drawing> {
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

// ── pie charts ────────────────────────────────────────────────────────────

fn pie(source: &str, options: &Options) -> Option<Drawing> {
    let mut title = "";
    let mut labels: Vec<Str> = Vec::new();
    let mut values: Vec<f32> = Vec::new();
    for raw in source.split('\n') {
        let line = trim_line(raw);
        if line.starts_with("pie") {
            if let Some(at) = line.find("title ") {
                title = trim_spaces(&line[at + 6..]);
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("title ") {
            title = trim_spaces(rest);
            continue;
        }
        let Some(colon) = line.rfind(':') else {
            continue;
        };
        let Ok(value) = trim_spaces(&line[colon + 1..]).parse::<f32>() else {
            continue;
        };
        if value <= 0.0 {
            continue;
        }
        labels.push(Str::copy(strip_quotes(&line[..colon])));
        values.push(value);
    }
    if values.is_empty() {
        return None;
    }
    let mut total: f32 = 0.0;
    for value in &values {
        total += value;
    }
    let fs = options.font_size;
    let r: f32 = 90.0;
    let cx: f32 = 8.0 + r;
    let mut top: f32 = 8.0;
    let mut canvas = Canvas::new();
    if !title.is_empty() {
        canvas.text(8.0, top + fs, title, fs * 1.1, options.colors.text);
        top += fs * 2.0;
    }
    let cy = top + r;
    let mut angle: f32 = -std::f32::consts::FRAC_PI_2;
    let mut legend_w: f32 = 0.0;
    for (i, value) in values.iter().enumerate() {
        let sweep = value / total * std::f32::consts::TAU;
        let steps: usize = ((sweep / std::f32::consts::TAU * 72.0) as usize).max(2);
        let mut points: Vec<Point> = Vec::new();
        points.push([cx, cy]);
        for k in 0..=steps {
            let a = angle + sweep * k as f32 / steps as f32;
            points.push([cx + a.cos() * r, cy + a.sin() * r]);
        }
        let color = PALETTE[i % PALETTE.len()];
        canvas.poly(&points, color, 0.0, true);
        canvas.poly(&points, options.colors.label_bg, 1.5, true);
        angle += sweep;
        // legend
        let ly = top + 6.0 + i as f32 * fs * 1.8;
        canvas.rect(cx + r + 28.0, ly, fs * 0.9, fs * 0.9, color, 2.0);
        let pct = format!("{}  {:.1}%", labels[i], value / total * 100.0);
        canvas.text(
            cx + r + 28.0 + fs * 1.4,
            ly + fs * 0.8,
            &pct,
            fs * 0.92,
            options.colors.text,
        );
        legend_w = legend_w.max(vector::measure(&pct, fs * 0.92, Family::Sans, 400, false).advance);
    }
    let legend_h = values.len() as f32 * fs * 1.8;
    Some(anchor_top(canvas.finish(
        cx + r + 28.0 + fs * 1.4 + legend_w + 8.0,
        (cy + r).max(top + legend_h) + 8.0,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_colors() -> Colors {
        Colors {
            text: 0xff000000,
            node_fill: 0xffeeeeee,
            node_border: 0xff999999,
            edge: 0xff666666,
            label_bg: 0xffffffff,
            muted: 0xff888888,
        }
    }

    fn test_options(max_width: f32) -> Options {
        Options {
            max_width,
            ..Options::new(test_colors())
        }
    }

    #[test]
    fn options_new_uses_the_default_size_and_width() {
        let options = Options::new(test_colors());
        assert_eq!(options.font_size, 14.0);
        assert_eq!(options.max_width, 640.0);
        assert_eq!(options.colors.text, 0xff000000);
    }

    #[test]
    fn flowchart_parsing() {
        let mut flow = Flow::default();
        flow.parse_statement("A[Start] --> B{Is it?} -->|Yes| C(Done)");
        flow.parse_statement("B -- No --> D[[Retry]]");
        assert_eq!(4, flow.nodes.len());
        assert_eq!(3, flow.edges.len());
        assert_eq!(flow.edges[1].label, "Yes");
        assert_eq!(flow.edges[2].label, "No");
        assert_eq!(Shape::Diamond, flow.nodes[1].shape);
        assert_eq!(Some(Kind::Flowchart), detect("%% c\ngraph TD\nA-->B"));
    }

    #[test]
    fn wide_flowcharts_shrink_to_max_width() {
        let src = "flowchart LR\n  A[A very long first node] --> B[Another long node]\n  B --> C[Third long node] --> D[Fourth long node] --> E[Fifth long node]";
        let wide = render(src, test_options(0.0)).expect("wide diagram");
        assert!(wide.width > 300.0);
        let fitted = render(src, test_options(300.0)).expect("fitted diagram");
        assert!(
            (fitted.width - 300.0).abs() < 0.5,
            "fitted width = {}",
            fitted.width
        );
    }

    #[test]
    fn flowchart_diagrams_render() {
        let src = "flowchart TD\n  A[Start] --> B{Is it?}\n  B -->|Yes| C(Done)\n  B -->|No| D[[Retry]]\n  D -.-> A";
        let drawing = render(src, test_options(640.0)).expect("flowchart");
        assert_eq!(
            drawing.descent, 0.0,
            "diagrams sit entirely above the baseline"
        );
        assert!(drawing.width > 0.0);
        assert!(drawing.ascent > 0.0);
        assert!(!drawing.commands.is_empty());
        assert_ne!(drawing.hash, 0, "render pins the content hash");
        assert_eq!(drawing.hash, render(src, test_options(640.0)).unwrap().hash);
        assert_eq!(detect(src), Some(Kind::Flowchart));
    }

    #[test]
    fn sequence_diagrams_render() {
        let src = "sequenceDiagram\n  participant A as Alice\n  participant B as Bob\n  A->>B: Hello\n  B-->>A: Hi\n  Note over A,B: shared\n  A->A: retry";
        let drawing = render(src, test_options(640.0)).expect("sequence diagram");
        assert_eq!(drawing.descent, 0.0);
        assert!(drawing.width > 0.0);
        assert!(drawing.ascent > 0.0);
        assert!(!drawing.commands.is_empty());
        assert_eq!(detect(src), Some(Kind::Sequence));
    }

    /// A single node pins the layout constants: `8` margin, `fs * 2.6`
    /// box height, `advance + 28` box width (min `56`), radius `3`, and the
    /// diagram is anchored entirely above the baseline.
    #[test]
    fn flowchart_single_node_layout_constants() {
        let options = test_options(640.0);
        let drawing = render("flowchart TD\n  A[Start]", options).expect("flowchart");
        let fs = options.font_size;
        let node_h = fs * 2.6;
        let advance = vector::measure("Start", fs, Family::Sans, 400, false).advance;
        let node_w = (advance + 28.0).max(56.0);
        assert!(
            (drawing.ascent - (node_h + 16.0)).abs() < 1e-4,
            "ascent = {}",
            drawing.ascent
        );
        assert!(
            (drawing.width - (node_w + 16.0)).abs() < 1e-4,
            "width = {}",
            drawing.width
        );
        // rect + rounded border path + centered label
        assert_eq!(drawing.commands.len(), 3);
        match &drawing.commands[0] {
            Command::Rect(rect) => {
                assert_eq!(rect.x, 8.0);
                assert_eq!(rect.y, 8.0 - drawing.ascent);
                assert_eq!(rect.width, node_w);
                assert_eq!(rect.height, node_h);
                assert_eq!(rect.color, options.colors.node_fill);
                assert_eq!(rect.radius, 3.0);
            }
            other => panic!("expected a node rect, got {other:?}"),
        }
        match &drawing.commands[2] {
            Command::Text(text) => {
                assert_eq!(text.text, "Start");
                assert_eq!(text.size, fs);
                assert_eq!(text.color, options.colors.text);
                assert_eq!(text.family, Family::Sans);
            }
            other => panic!("expected the node label, got {other:?}"),
        }
    }

    #[test]
    fn detection_skips_front_matter_and_comments() {
        assert_eq!(
            detect("---\ntitle: x\n---\ngraph TD\nA-->B"),
            Some(Kind::Flowchart)
        );
        assert_eq!(
            detect("%% comment\nflowchart LR\nA-->B"),
            Some(Kind::Flowchart)
        );
        assert_eq!(
            detect("\n\nsequenceDiagram\nA->>B: hi"),
            Some(Kind::Sequence)
        );
        // Never leaves front matter -> no diagram kind.
        assert_eq!(detect("---\ntitle: x\n---"), None);
        assert_eq!(detect(""), None);
    }

    #[test]
    fn pie_charts_render() {
        let src = "pie title Pets\n  \"Dogs\" : 30\n  \"Cats\" : 20";
        let drawing = render(src, test_options(640.0)).expect("pie chart");
        assert_eq!(drawing.descent, 0.0);
        assert!(drawing.width > 0.0);
        assert!(drawing.ascent > 0.0);
        assert!(!drawing.commands.is_empty());
        assert_eq!(detect(src), Some(Kind::Pie));
    }

    /// Unsupported or empty diagrams fall back to a code block (`render` → None).
    #[test]
    fn unsupported_diagrams_fall_back() {
        let options = test_options(640.0);
        assert_eq!(None, detect("classDiagram\n  A --> B"));
        assert_eq!(None, detect("not a diagram"));
        assert!(render("classDiagram\n  A --> B", options).is_none());
        // A recognised header with no drawable content also falls back.
        assert!(render("flowchart TD\n", options).is_none());
        assert!(render("sequenceDiagram\n", options).is_none());
        assert!(render("pie\n", options).is_none());
    }
}
