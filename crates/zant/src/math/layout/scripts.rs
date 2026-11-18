//! Subscripts, superscripts, operator limits, and stacked labels.

use crate::core::vector::{self, Command, Text as VectorText};
use crate::math::{
    parser::{self, Node},
    symbols::Class,
};
use crate::renderer::text::Family;

use super::{Box, Engine, Size, Style, style::Metric};

impl Engine {
    // ── supsub (KaTeX functions/supsub.js) ──────────────────────────────

    pub(super) fn supsub(&mut self, s: &parser::SupSub, style: Style) -> Box {
        // operators with limits and braces with labels stack their scripts
        if let Some(base_node) = &s.base {
            match &**base_node {
                Node::Op(o) => {
                    let limits = o.forced.unwrap_or(o.limits && style.size == Size::Display);
                    if limits {
                        return self.op_limits(o, s, style);
                    }
                }
                Node::OverUnder(ou) => {
                    if matches!(
                        ou.kind,
                        parser::OverUnderKind::Overbrace | parser::OverUnderKind::Underbrace
                    ) {
                        return self.brace_label(ou, s, style);
                    }
                }
                _ => {}
            }
        }
        let base: Box = match &s.base {
            Some(b) => self.build(b, style),
            None => Box::default(),
        };
        let sup_style = style.sup();
        let sub_style = style.sub();
        let sup_box: Option<Box> = s.sup.as_ref().map(|n| self.build(n, sup_style));
        let sub_box: Option<Box> = s.sub.as_ref().map(|n| self.build(n, sub_style));

        let mut sup_shift: f32 = 0.0;
        let mut sub_shift: f32 = 0.0;
        if !base.is_char {
            sup_shift = base.height - self.metric(sup_style, Metric::SupDrop);
            sub_shift = base.depth + self.metric(sub_style, Metric::SubDrop);
        }
        let min_sup_shift = if style.size == Size::Display {
            self.metric(style, Metric::Sup1)
        } else if style.cramped {
            self.metric(style, Metric::Sup3)
        } else {
            self.metric(style, Metric::Sup2)
        };
        let x_height = self.metric(style, Metric::XHeight);
        if let (Some(sp), Some(sb)) = (&sup_box, &sub_box) {
            sup_shift = sup_shift.max(min_sup_shift).max(sp.depth + 0.25 * x_height);
            sub_shift = sub_shift.max(self.metric(style, Metric::Sub2));
            let rule_width = self.metric(style, Metric::DefaultRuleThickness);
            let max_width = 4.0 * rule_width;
            if (sup_shift - sp.depth) - (sb.height - sub_shift) < max_width {
                sub_shift = max_width - (sup_shift - sp.depth) + sb.height;
                let psi = 0.8 * x_height - (sup_shift - sp.depth);
                if psi > 0.0 {
                    sup_shift += psi;
                    sub_shift -= psi;
                }
            }
        } else if let Some(sb) = &sub_box {
            sub_shift = sub_shift
                .max(self.metric(style, Metric::Sub1))
                .max(sb.height - 0.8 * x_height);
        } else if let Some(sp) = &sup_box {
            sup_shift = sup_shift.max(min_sup_shift).max(sp.depth + 0.25 * x_height);
        }

        let mut out: Vec<Command> = Vec::new();
        self.translated(&base, 0.0, 0.0, &mut out);
        let script_space = 0.05 * self.options.size;
        let mut width = base.width;
        let mut height = base.height;
        let mut depth = base.depth;
        let mut scripts_width: f32 = 0.0;
        if let Some(sp) = &sup_box {
            self.translated(sp, base.width + base.italic, -sup_shift, &mut out);
            scripts_width = scripts_width.max(sp.width + base.italic);
            height = height.max(sup_shift + sp.height);
            depth = depth.max(sp.depth - sup_shift);
        }
        if let Some(sb) = &sub_box {
            self.translated(sb, base.width, sub_shift, &mut out);
            scripts_width = scripts_width.max(sb.width);
            depth = depth.max(sub_shift + sb.depth);
            height = height.max(sb.height - sub_shift);
        }
        width += scripts_width + script_space;
        Box {
            width,
            height,
            depth,
            cmds: out,
            class: base.class,
            ..Box::default()
        }
    }

    // ── big operators (KaTeX functions/op.js, assembleSupSub) ───────────

    fn op_box(&self, o: &parser::Op, style: Style) -> Box {
        let size = self.em(style);
        if !o.symbol {
            let mut b = self.text_box(&o.text, size, self.color);
            b.class = Class::Op;
            return b;
        }
        // KaTeX draws display operators from the Size2 font (∑ ≈ 1.45×, ∫ ≈ 2×)
        let factor: f32 = if style.size == Size::Display {
            if o.integral { 2.0 } else { 1.45 }
        } else if o.integral {
            1.15
        } else {
            1.0
        };
        let font_size = size * factor;
        let m = vector::measure(&o.text, font_size, Family::Math, 400, false);
        let axis = self.metric(style, Metric::AxisHeight);
        let baseline = -axis + (m.ascent - m.descent) / 2.0;
        let cmds = vec![Command::Text(VectorText {
            x: 0.0,
            y: baseline,
            text: crate::Str::copy(&o.text),
            size: font_size,
            family: Family::Math,
            weight: 400,
            italic: false,
            color: self.color,
            scale_y: 1.0,
        })];
        Box {
            width: m.advance,
            height: m.ascent - baseline,
            depth: m.descent + baseline,
            cmds,
            class: Class::Op,
            italic: if o.integral { 0.3 * font_size } else { 0.0 },
            ..Box::default()
        }
    }

    pub(super) fn op(&self, o: &parser::Op, style: Style) -> Box {
        self.op_box(o, style)
    }

    fn op_limits(&mut self, o: &parser::Op, s: &parser::SupSub, style: Style) -> Box {
        let base = self.op_box(o, style);
        let sup: Option<Box> = s.sup.as_ref().map(|n| self.build(n, style.sup()));
        let sub: Option<Box> = s.sub.as_ref().map(|n| self.build(n, style.sub()));
        let slant = base.italic;
        self.assemble_limits(base, sup, sub, style, slant)
    }

    fn assemble_limits(
        &self,
        base: Box,
        sup: Option<Box>,
        sub: Option<Box>,
        style: Style,
        slant: f32,
    ) -> Box {
        let mut width = base.width;
        if let Some(b) = &sup {
            width = width.max(b.width);
        }
        if let Some(b) = &sub {
            width = width.max(b.width);
        }
        let mut out: Vec<Command> = Vec::new();
        self.translated(&base, (width - base.width) / 2.0, 0.0, &mut out);
        let mut height = base.height;
        let mut depth = base.depth;
        let spacing5 = self.metric(style, Metric::BigOpSpacing5);
        if let Some(b) = &sup {
            let kern_sup = self
                .metric(style, Metric::BigOpSpacing1)
                .max(self.metric(style, Metric::BigOpSpacing3) - b.depth);
            let baseline = -(base.height + kern_sup + b.depth);
            self.translated(b, (width - b.width) / 2.0 + slant / 2.0, baseline, &mut out);
            height = base.height + kern_sup + b.depth + b.height + spacing5;
        }
        if let Some(b) = &sub {
            let kern_sub = self
                .metric(style, Metric::BigOpSpacing2)
                .max(self.metric(style, Metric::BigOpSpacing4) - b.height);
            let baseline = base.depth + kern_sub + b.height;
            self.translated(b, (width - b.width) / 2.0 - slant / 2.0, baseline, &mut out);
            depth = base.depth + kern_sub + b.height + b.depth + spacing5;
        }
        Box {
            width,
            height,
            depth,
            cmds: out,
            class: Class::Op,
            ..Box::default()
        }
    }

    pub(super) fn stack(
        &mut self,
        base_node: &Node,
        over: Option<&Node>,
        under: Option<&Node>,
        style: Style,
    ) -> Box {
        let mut base = self.build(base_node, style);
        base.class = Class::Rel;
        let base_is_char = base.is_char;
        let over: Option<Box> = over.map(|n| self.build(n, style.sup()));
        let under: Option<Box> = under.map(|n| self.build(n, style.sub()));
        let mut b = self.assemble_limits(base, over, under, style, 0.0);
        // \overset/\stackrel: tighter than operator limits, keep base class
        b.class = if base_is_char { Class::Rel } else { Class::Ord };
        if let Node::Group(g) = base_node
            && g.len() == 1
            && let Node::Atom(a) = &g[0]
        {
            b.class = a.class;
        }
        b
    }

    fn brace_label(&mut self, ou: &parser::OverUnder, s: &parser::SupSub, style: Style) -> Box {
        let base = self.over_under(ou, style);
        let label_node: Option<&Node> = if ou.kind == parser::OverUnderKind::Overbrace {
            s.sup.as_deref().or(s.sub.as_deref())
        } else {
            s.sub.as_deref().or(s.sup.as_deref())
        };
        let label: Option<Box> = label_node.map(|n| self.build(n, style.sup()));
        if ou.kind == parser::OverUnderKind::Overbrace {
            self.assemble_limits(base, label, None, style, 0.0)
        } else {
            self.assemble_limits(base, None, label, style, 0.0)
        }
    }
}
