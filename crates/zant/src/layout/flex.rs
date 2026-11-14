//! CSS-style single-line flex distribution
//! (grow / shrink with min/max clamping, gaps, margins, justification).
#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Direction {
    #[default]
    Row,
    Column,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Wrap {
    #[default]
    Nowrap,
    Wrap,
    WrapReverse,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AlignContent {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    Stretch,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AlignSelf {
    #[default]
    Auto,
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Item {
    pub base: f32,
    pub grow: f32,
    pub shrink: f32,
    pub min: f32,
    pub max: f32,
    pub margin_start: f32,
    pub margin_end: f32,
}

impl Default for Item {
    fn default() -> Self {
        Item {
            base: 0.0,
            grow: 0.0,
            shrink: 1.0,
            min: 0.0,
            max: f32::INFINITY,
            margin_start: 0.0,
            margin_end: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AxisSize {
    /// Space occupied by item content (excluding margins).
    pub size: f32,
    pub margin_start: f32,
    pub margin_end: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Distribution {
    /// Absolute offset added before the first item (justify-content).
    pub start_offset: f32,
    /// Extra space inserted between adjacent items.
    pub between: f32,
    /// Leftover space in the main axis after flexing and justification.
    pub free: f32,
}

fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    lo.max(v.min(hi))
}

/// Resolves a single-line flex distribution. `available` excludes container
/// padding but includes item margins and gaps. `out` receives content sizes for
/// each item. The returned value is the leftover free space (usually 0 after a
/// successful grow/shrink pass, or the justified space if no flex factors).
pub fn resolve(items: &[Item], available: f32, gap: f32, out: &mut [f32]) -> f32 {
    debug_assert_eq!(items.len(), out.len());
    let n = items.len();
    if n == 0 {
        return available.max(0.0);
    }

    let mut total: f32 = 0.0;
    for (i, item) in items.iter().enumerate() {
        out[i] = clamp(item.base, item.min, item.max);
        total += out[i] + item.margin_start + item.margin_end;
    }
    let gap_total = gap * if n > 1 { (n - 1) as f32 } else { 0.0 };
    total += gap_total;

    let mut free = available - total;

    if free > 0.0001 {
        let mut grow_total: f32 = 0.0;
        for item in items {
            grow_total += item.grow.max(0.0);
        }
        if grow_total > 0.0001 {
            let mut remaining = free;
            let mut active = [true; 64];
            if n > active.len() {
                // The UI's node count per flex line is intentionally small. For
                // larger lists, fall back to a single proportional pass.
                for (i, item) in items.iter().enumerate() {
                    if item.grow <= 0.0 {
                        continue;
                    }
                    out[i] = clamp(
                        out[i] + remaining * item.grow / grow_total,
                        item.min,
                        item.max,
                    );
                }
                free = 0.0;
            } else {
                let mut pass: usize = 0;
                while pass < 8 && remaining > 0.0001 {
                    let mut weight: f32 = 0.0;
                    for (i, item) in items.iter().enumerate() {
                        if active[i] && item.grow > 0.0 {
                            weight += item.grow;
                        }
                    }
                    if weight <= 0.0001 {
                        break;
                    }
                    let mut used: f32 = 0.0;
                    for (i, item) in items.iter().enumerate() {
                        if !active[i] || item.grow <= 0.0 {
                            continue;
                        }
                        let add = remaining * item.grow / weight;
                        let old = out[i];
                        out[i] = clamp(out[i] + add, item.min, item.max);
                        used += out[i] - old;
                        if out[i] >= item.max - 0.0001 {
                            active[i] = false;
                        }
                    }
                    remaining -= used;
                    if used <= 0.0001 {
                        break;
                    }
                    pass += 1;
                }
                free = remaining;
            }
        }
    } else if free < -0.0001 {
        let mut shrink_weight: f32 = 0.0;
        for (i, item) in items.iter().enumerate() {
            shrink_weight += item.shrink.max(0.0) * out[i].max(0.0);
        }
        if shrink_weight > 0.0001 {
            let mut deficit = -free;
            let mut active = [true; 64];
            if n > active.len() {
                for (i, item) in items.iter().enumerate() {
                    let weight = item.shrink.max(0.0) * out[i].max(0.0);
                    if weight <= 0.0 {
                        continue;
                    }
                    out[i] = clamp(
                        out[i] - deficit * weight / shrink_weight,
                        item.min,
                        item.max,
                    );
                }
                free = 0.0;
            } else {
                let mut pass: usize = 0;
                while pass < 8 && deficit > 0.0001 {
                    let mut weight: f32 = 0.0;
                    for (i, item) in items.iter().enumerate() {
                        if active[i] && item.shrink > 0.0 {
                            weight += item.shrink.max(0.0) * out[i].max(0.0);
                        }
                    }
                    if weight <= 0.0001 {
                        break;
                    }
                    let mut used: f32 = 0.0;
                    for (i, item) in items.iter().enumerate() {
                        if !active[i] || item.shrink <= 0.0 {
                            continue;
                        }
                        let w = item.shrink.max(0.0) * out[i].max(0.0);
                        if w <= 0.0 {
                            continue;
                        }
                        let old = out[i];
                        out[i] = clamp(out[i] - deficit * w / weight, item.min, item.max);
                        used += old - out[i];
                        if out[i] <= item.min + 0.0001 {
                            active[i] = false;
                        }
                    }
                    deficit -= used;
                    if used <= 0.0001 {
                        break;
                    }
                    pass += 1;
                }
                free = -deficit;
            }
        }
    }

    if free > 0.0001 {
        return free;
    }
    if free < -0.0001 {
        return free;
    }
    0.0
}

pub fn justify_distribution(justify: Justify, free: f32, count: usize) -> Distribution {
    if count == 0 || free <= 0.0 {
        return Distribution::default();
    }
    let f = free.max(0.0);
    let n = count as f32;
    match justify {
        Justify::Start => Distribution {
            free: f,
            ..Default::default()
        },
        Justify::Center => Distribution {
            start_offset: f / 2.0,
            free: f,
            ..Default::default()
        },
        Justify::End => Distribution {
            start_offset: f,
            free: f,
            ..Default::default()
        },
        Justify::SpaceBetween => {
            if count > 1 {
                Distribution {
                    between: f / (n - 1.0),
                    free: f,
                    ..Default::default()
                }
            } else {
                Distribution {
                    free: f,
                    ..Default::default()
                }
            }
        }
        Justify::SpaceAround => Distribution {
            start_offset: f / (2.0 * n),
            between: f / n,
            free: f,
        },
        Justify::SpaceEvenly => Distribution {
            start_offset: f / (n + 1.0),
            between: f / (n + 1.0),
            free: f,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{Distribution, Item, Justify, justify_distribution, resolve};

    fn item(base: f32, grow: f32, shrink: f32, max: f32) -> Item {
        Item {
            base,
            grow,
            shrink,
            max,
            ..Default::default()
        }
    }

    #[test]
    fn row_grow_distribution() {
        let items = [
            item(100.0, 1.0, 1.0, f32::INFINITY),
            item(100.0, 1.0, 1.0, f32::INFINITY),
        ];
        let mut out = [0.0_f32; 2];
        let free = resolve(&items, 400.0, 0.0, &mut out);
        assert!((0.0_f32 - free).abs() <= 0.01);
        assert!((200.0_f32 - out[0]).abs() <= 0.01);
        assert!((200.0_f32 - out[1]).abs() <= 0.01);
    }

    #[test]
    fn grow_respects_max_size() {
        let items = [
            item(100.0, 1.0, 1.0, 120.0),
            item(100.0, 1.0, 1.0, f32::INFINITY),
        ];
        let mut out = [0.0_f32; 2];
        let _ = resolve(&items, 400.0, 0.0, &mut out);
        assert!((120.0_f32 - out[0]).abs() <= 0.01);
        assert!((280.0_f32 - out[1]).abs() <= 0.01);
    }

    #[test]
    fn shrink_weighted_by_base() {
        let items = [
            item(200.0, 0.0, 1.0, f32::INFINITY),
            item(100.0, 0.0, 1.0, f32::INFINITY),
        ];
        let mut out = [0.0_f32; 2];
        let _ = resolve(&items, 200.0, 0.0, &mut out);
        assert!((133.333_f32 - out[0]).abs() <= 0.01);
        assert!((66.666_f32 - out[1]).abs() <= 0.01);
    }

    #[test]
    fn gap_and_margins_participate_in_main_size() {
        let items = [
            Item {
                base: 50.0,
                margin_start: 5.0,
                margin_end: 5.0,
                ..Default::default()
            },
            Item {
                base: 50.0,
                margin_start: 5.0,
                margin_end: 5.0,
                ..Default::default()
            },
        ];
        let mut out = [0.0_f32; 2];
        let free = resolve(&items, 200.0, 10.0, &mut out);
        assert!((70.0_f32 - free).abs() <= 0.01);
    }

    #[test]
    fn justify_variants() {
        let between: Distribution = justify_distribution(Justify::SpaceBetween, 60.0, 3);
        assert!((30.0_f32 - between.between).abs() <= 0.01);
        let around = justify_distribution(Justify::SpaceAround, 60.0, 3);
        assert!((10.0_f32 - around.start_offset).abs() <= 0.01);
        let evenly = justify_distribution(Justify::SpaceEvenly, 60.0, 3);
        assert!((15.0_f32 - evenly.start_offset).abs() <= 0.01);
    }
}
