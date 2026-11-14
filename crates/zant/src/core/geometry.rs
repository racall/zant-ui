//! Points, sizes, edges, rectangles and layout constraints.
#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub fn zero() -> Size {
        Size::default()
    }

    pub fn max(a: Size, b: Size) -> Size {
        Size {
            width: a.width.max(b.width),
            height: a.height.max(b.height),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    pub fn all(v: f32) -> Edges {
        Edges {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }

    pub fn symmetric(h: f32, v: f32) -> Edges {
        Edges {
            top: v,
            right: h,
            bottom: v,
            left: h,
        }
    }

    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn init(x: f32, y: f32, width: f32, height: f32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    pub fn size(&self) -> Size {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && py >= self.y && px < self.right() && py < self.bottom()
    }

    pub fn inset(&self, edges: Edges) -> Rect {
        Rect {
            x: self.x + edges.left,
            y: self.y + edges.top,
            width: (self.width - edges.horizontal()).max(0.0),
            height: (self.height - edges.vertical()).max(0.0),
        }
    }

    pub fn intersect(a: Rect, b: Rect) -> Rect {
        let x0 = a.x.max(b.x);
        let y0 = a.y.max(b.y);
        let x1 = a.right().min(b.right());
        let y1 = a.bottom().min(b.bottom());
        Rect {
            x: x0,
            y: y0,
            width: (x1 - x0).max(0.0),
            height: (y1 - y0).max(0.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Constraints {
    pub min_width: f32,
    pub max_width: f32,
    pub min_height: f32,
    pub max_height: f32,
}

impl Constraints {
    pub fn tight(width: f32, height: f32) -> Constraints {
        Constraints {
            min_width: width,
            max_width: width,
            min_height: height,
            max_height: height,
        }
    }

    pub fn loose(width: f32, height: f32) -> Constraints {
        Constraints {
            max_width: width,
            max_height: height,
            ..Default::default()
        }
    }
}

impl Default for Constraints {
    fn default() -> Self {
        Constraints {
            min_width: 0.0,
            max_width: f32::INFINITY,
            min_height: 0.0,
            max_height: f32::INFINITY,
        }
    }
}

// These cover the arithmetic (boundary inclusivity, clamping) that the layout
// code relies on.
#[cfg(test)]
mod tests {
    use super::{Constraints, Edges, Rect, Size};

    #[test]
    fn size_zero_and_max() {
        assert_eq!(
            Size::zero(),
            Size {
                width: 0.0,
                height: 0.0
            }
        );
        assert_eq!(
            Size {
                width: 0.0,
                height: 0.0
            },
            Size::default()
        );
        assert_eq!(
            Size::max(
                Size {
                    width: 10.0,
                    height: 40.0
                },
                Size {
                    width: 30.0,
                    height: 20.0
                }
            ),
            Size {
                width: 30.0,
                height: 40.0
            }
        );
    }

    #[test]
    fn edges_all_symmetric_horizontal_vertical() {
        let all = Edges::all(4.0);
        assert_eq!(
            all,
            Edges {
                top: 4.0,
                right: 4.0,
                bottom: 4.0,
                left: 4.0
            }
        );
        let sym = Edges::symmetric(3.0, 7.0);
        assert_eq!(
            sym,
            Edges {
                top: 7.0,
                right: 3.0,
                bottom: 7.0,
                left: 3.0
            }
        );
        assert_eq!(sym.horizontal(), 6.0);
        assert_eq!(sym.vertical(), 14.0);
    }

    #[test]
    fn rect_edges_and_size() {
        let r = Rect::init(10.0, 20.0, 100.0, 50.0);
        assert_eq!(r.right(), 110.0);
        assert_eq!(r.bottom(), 70.0);
        assert_eq!(
            r.size(),
            Size {
                width: 100.0,
                height: 50.0
            }
        );
    }

    #[test]
    fn rect_contains_uses_exclusive_right_and_bottom() {
        let r = Rect::init(10.0, 20.0, 100.0, 50.0);
        assert!(r.contains(10.0, 20.0));
        assert!(r.contains(109.99, 69.99));
        assert!(!r.contains(110.0, 40.0));
        assert!(!r.contains(50.0, 70.0));
        assert!(!r.contains(9.99, 40.0));
    }

    #[test]
    fn rect_inset_clamps_to_zero() {
        let r = Rect::init(10.0, 20.0, 100.0, 50.0);
        let inset = r.inset(Edges {
            top: 5.0,
            right: 10.0,
            bottom: 5.0,
            left: 20.0,
        });
        assert_eq!(inset, Rect::init(30.0, 25.0, 70.0, 40.0));
        let over = r.inset(Edges::all(200.0));
        assert_eq!(over.width, 0.0);
        assert_eq!(over.height, 0.0);
        assert_eq!((over.x, over.y), (210.0, 220.0));
    }

    #[test]
    fn rect_intersect_clamps_and_empties() {
        let a = Rect::init(0.0, 0.0, 100.0, 100.0);
        let b = Rect::init(50.0, 50.0, 100.0, 100.0);
        assert_eq!(Rect::intersect(a, b), Rect::init(50.0, 50.0, 50.0, 50.0));
        let far = Rect::init(200.0, 200.0, 10.0, 10.0);
        let empty = Rect::intersect(a, far);
        assert_eq!((empty.width, empty.height), (0.0, 0.0));
    }

    #[test]
    fn constraints_tight_loose_and_default() {
        assert_eq!(
            Constraints::tight(30.0, 40.0),
            Constraints {
                min_width: 30.0,
                max_width: 30.0,
                min_height: 40.0,
                max_height: 40.0,
            }
        );
        assert_eq!(
            Constraints::loose(30.0, 40.0),
            Constraints {
                min_width: 0.0,
                max_width: 30.0,
                min_height: 0.0,
                max_height: 40.0,
            }
        );
        let d = Constraints::default();
        assert_eq!(d.min_width, 0.0);
        assert_eq!(d.max_width, f32::INFINITY);
        assert_eq!(d.min_height, 0.0);
        assert_eq!(d.max_height, f32::INFINITY);
    }
}
