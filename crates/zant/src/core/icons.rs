//! Zant icon set: line icons on a 24×24 grid with a uniform stroke (in the
//! spirit of Lucide), drawn as vector paths so every icon shares the same
//! optical size and weight at any scale.
//!
//! Segment counts are fixed: arcs are 32 steps / 33 points, circles 32 points,
//! rounded-rectangle corners 7 points per corner in 15° steps.
//!
//! [`Sink`] is the output trait; `Pen` stays internal. `paint` routes through [`Canvas`] (the base's safe wrapper) so no
//! `unsafe` is needed in `core/`.

use crate::core::geometry::Rect;
use crate::core::hash::Wyhash;
use crate::core::vector::{Command, Drawing, Path, Point, Rect as DrawingRect};
use crate::renderer::canvas::Canvas;

/// Radians per degree (the true π/180 rounded to `f32`, identical to
/// `f32::to_radians(1.0)`).
const RAD_PER_DEG: f32 = 0.017_453_292_f32;

/// The icon set. Declaration order is stable: it is the
/// order of [`Icon::ALL`] and the discriminant hashed by [`drawing`].
#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Icon {
    #[default]
    None,
    // navigation / chrome
    Menu,
    List,
    Search,
    Eye,
    EyeOff,
    More,
    MoreVertical,
    Close,
    Plus,
    Minus,
    Check,
    ChevronDown,
    ChevronUp,
    ChevronLeft,
    ChevronRight,
    ArrowDown,
    ArrowUp,
    // files / actions
    File,
    FileText,
    Folder,
    Save,
    Copy,
    Download,
    Upload,
    Link,
    ExternalLink,
    Pencil,
    Trash,
    Undo,
    Redo,
    /// two circling arrows: sync, reload
    Refresh,
    Settings,
    // view
    Sun,
    Moon,
    Columns,
    Keyboard,
    BookOpen,
    Maximize,
    Minimize,
    Focus,
    TypeSize,
    // formatting
    Heading,
    Bold,
    Italic,
    Strikethrough,
    Code,
    CodeBlock,
    Quote,
    ListBullet,
    ListOrdered,
    ListChecks,
    Table,
    Sigma,
    Image,
    MinusRule,
    // Windows caption buttons (drawn at 10px with a 1px stroke)
    WindowMinimize,
    WindowMaximize,
    WindowRestore,
    WindowClose,
    // status
    Info,
    Alert,
    Lightbulb,
    // devices and system
    Clipboard,
    Monitor,
    Wifi,
    WifiOff,
    Clock,
    Power,
}

impl Icon {
    /// Every variant, in declaration
    /// order (the order is part of the public contract — see the enum docs).
    pub const ALL: [Icon; 69] = [
        Icon::None,
        Icon::Menu,
        Icon::List,
        Icon::Search,
        Icon::Eye,
        Icon::EyeOff,
        Icon::More,
        Icon::MoreVertical,
        Icon::Close,
        Icon::Plus,
        Icon::Minus,
        Icon::Check,
        Icon::ChevronDown,
        Icon::ChevronUp,
        Icon::ChevronLeft,
        Icon::ChevronRight,
        Icon::ArrowDown,
        Icon::ArrowUp,
        Icon::File,
        Icon::FileText,
        Icon::Folder,
        Icon::Save,
        Icon::Copy,
        Icon::Download,
        Icon::Upload,
        Icon::Link,
        Icon::ExternalLink,
        Icon::Pencil,
        Icon::Trash,
        Icon::Undo,
        Icon::Redo,
        Icon::Refresh,
        Icon::Settings,
        Icon::Sun,
        Icon::Moon,
        Icon::Columns,
        Icon::Keyboard,
        Icon::BookOpen,
        Icon::Maximize,
        Icon::Minimize,
        Icon::Focus,
        Icon::TypeSize,
        Icon::Heading,
        Icon::Bold,
        Icon::Italic,
        Icon::Strikethrough,
        Icon::Code,
        Icon::CodeBlock,
        Icon::Quote,
        Icon::ListBullet,
        Icon::ListOrdered,
        Icon::ListChecks,
        Icon::Table,
        Icon::Sigma,
        Icon::Image,
        Icon::MinusRule,
        Icon::WindowMinimize,
        Icon::WindowMaximize,
        Icon::WindowRestore,
        Icon::WindowClose,
        Icon::Info,
        Icon::Alert,
        Icon::Lightbulb,
        Icon::Clipboard,
        Icon::Monitor,
        Icon::Wifi,
        Icon::WifiOff,
        Icon::Clock,
        Icon::Power,
    ];
}

/// Receives one stroked polyline in 24×24 icon space.
pub trait Sink {
    fn stroke(&mut self, points: &[Point], closed: bool);
    fn dot(&mut self, x: f32, y: f32, r: f32);
}

/// Sugar over [`Sink`] used by [`trace`].
struct Pen<'a> {
    sink: &'a mut dyn Sink,
}

impl Pen<'_> {
    fn line(&mut self, points: &[Point]) {
        self.sink.stroke(points, false);
    }

    fn closed(&mut self, points: &[Point]) {
        self.sink.stroke(points, true);
    }

    fn dot(&mut self, x: f32, y: f32, r: f32) {
        self.sink.dot(x, y, r);
    }

    /// 32 steps, 33 points (both ends included).
    fn arc(&mut self, cx: f32, cy: f32, r: f32, a0_deg: f32, a1_deg: f32) {
        let mut points: [Point; 33] = [[0.0; 2]; 33];
        let steps: usize = 32;
        for (i, point) in points.iter_mut().enumerate() {
            let t = i as f32 / steps as f32;
            let a = (a0_deg + (a1_deg - a0_deg) * t) * RAD_PER_DEG;
            *point = [cx + a.cos() * r, cy + a.sin() * r];
        }
        self.line(&points);
    }

    /// 32 points, closed.
    fn circle(&mut self, cx: f32, cy: f32, r: f32) {
        let mut points: [Point; 32] = [[0.0; 2]; 32];
        let len = points.len();
        for (i, point) in points.iter_mut().enumerate() {
            let a = i as f32 / len as f32 * std::f32::consts::TAU;
            *point = [cx + a.cos() * r, cy + a.sin() * r];
        }
        self.closed(&points);
    }

    /// Rounded rectangle outline: 7 points per corner (15° steps), closed.
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        let mut points: [Point; 28] = [[0.0; 2]; 28];
        let mut n: usize = 0;
        let corners = [
            [x + w - r, y + r, -90.0],
            [x + w - r, y + h - r, 0.0],
            [x + r, y + h - r, 90.0],
            [x + r, y + r, 180.0],
        ];
        for corner in corners {
            for k in 0..7 {
                let a = (corner[2] + k as f32 * 15.0) * RAD_PER_DEG;
                points[n] = [corner[0] + a.cos() * r, corner[1] + a.sin() * r];
                n += 1;
            }
        }
        self.closed(&points[..n]);
    }
}

/// Emits `icon` in 24×24 space.
pub fn trace(icon: Icon, sink: &mut dyn Sink) {
    let mut p = Pen { sink };
    match icon {
        Icon::None => {}
        Icon::Menu => {
            p.line(&[[4.0, 6.0], [20.0, 6.0]]);
            p.line(&[[4.0, 12.0], [20.0, 12.0]]);
            p.line(&[[4.0, 18.0], [20.0, 18.0]]);
        }
        Icon::List => {
            p.line(&[[9.0, 6.0], [20.0, 6.0]]);
            p.line(&[[9.0, 12.0], [20.0, 12.0]]);
            p.line(&[[9.0, 18.0], [20.0, 18.0]]);
            p.dot(4.5, 6.0, 1.1);
            p.dot(4.5, 12.0, 1.1);
            p.dot(4.5, 18.0, 1.1);
        }
        Icon::Search => {
            p.circle(10.5, 10.5, 6.5);
            p.line(&[[15.5, 15.5], [20.5, 20.5]]);
        }
        Icon::Eye => {
            p.closed(&[
                [2.0, 12.0],
                [4.5, 8.6],
                [8.0, 6.2],
                [12.0, 5.4],
                [16.0, 6.2],
                [19.5, 8.6],
                [22.0, 12.0],
                [19.5, 15.4],
                [16.0, 17.8],
                [12.0, 18.6],
                [8.0, 17.8],
                [4.5, 15.4],
            ]);
            p.circle(12.0, 12.0, 3.0);
        }
        Icon::EyeOff => {
            p.line(&[
                [4.0, 8.8],
                [2.0, 12.0],
                [4.5, 15.4],
                [8.0, 17.8],
                [12.0, 18.6],
                [15.0, 18.1],
            ]);
            p.line(&[
                [9.5, 5.8],
                [12.0, 5.4],
                [16.0, 6.2],
                [19.5, 8.6],
                [22.0, 12.0],
                [20.3, 14.4],
            ]);
            p.line(&[[3.0, 3.0], [21.0, 21.0]]);
        }
        Icon::More => {
            p.dot(5.5, 12.0, 1.4);
            p.dot(12.0, 12.0, 1.4);
            p.dot(18.5, 12.0, 1.4);
        }
        Icon::MoreVertical => {
            p.dot(12.0, 5.5, 1.4);
            p.dot(12.0, 12.0, 1.4);
            p.dot(12.0, 18.5, 1.4);
        }
        Icon::Close => {
            p.line(&[[6.0, 6.0], [18.0, 18.0]]);
            p.line(&[[18.0, 6.0], [6.0, 18.0]]);
        }
        Icon::Plus => {
            p.line(&[[12.0, 5.0], [12.0, 19.0]]);
            p.line(&[[5.0, 12.0], [19.0, 12.0]]);
        }
        Icon::Minus => p.line(&[[5.0, 12.0], [19.0, 12.0]]),
        Icon::Check => p.line(&[[5.0, 12.5], [10.0, 17.5], [19.5, 7.0]]),
        Icon::ChevronDown => p.line(&[[6.0, 9.0], [12.0, 15.0], [18.0, 9.0]]),
        Icon::ChevronUp => p.line(&[[6.0, 15.0], [12.0, 9.0], [18.0, 15.0]]),
        Icon::ChevronLeft => p.line(&[[15.0, 6.0], [9.0, 12.0], [15.0, 18.0]]),
        Icon::ChevronRight => p.line(&[[9.0, 6.0], [15.0, 12.0], [9.0, 18.0]]),
        Icon::ArrowDown => {
            p.line(&[[12.0, 5.0], [12.0, 19.0]]);
            p.line(&[[6.0, 13.0], [12.0, 19.0], [18.0, 13.0]]);
        }
        Icon::ArrowUp => {
            p.line(&[[12.0, 19.0], [12.0, 5.0]]);
            p.line(&[[6.0, 11.0], [12.0, 5.0], [18.0, 11.0]]);
        }
        Icon::File => {
            p.closed(&[
                [6.0, 2.5],
                [14.0, 2.5],
                [19.0, 7.5],
                [19.0, 21.5],
                [6.0, 21.5],
            ]);
            p.line(&[[14.0, 2.5], [14.0, 7.5], [19.0, 7.5]]);
        }
        Icon::FileText => {
            p.closed(&[
                [6.0, 2.5],
                [14.0, 2.5],
                [19.0, 7.5],
                [19.0, 21.5],
                [6.0, 21.5],
            ]);
            p.line(&[[14.0, 2.5], [14.0, 7.5], [19.0, 7.5]]);
            p.line(&[[9.0, 13.0], [16.0, 13.0]]);
            p.line(&[[9.0, 17.0], [16.0, 17.0]]);
        }
        Icon::Folder => p.closed(&[
            [3.0, 6.0],
            [9.0, 6.0],
            [11.0, 8.5],
            [21.0, 8.5],
            [21.0, 19.0],
            [3.0, 19.0],
        ]),
        Icon::Save => {
            p.closed(&[
                [4.0, 4.0],
                [16.5, 4.0],
                [20.0, 7.5],
                [20.0, 20.0],
                [4.0, 20.0],
            ]);
            p.line(&[[7.5, 4.0], [7.5, 9.0], [15.0, 9.0], [15.0, 4.0]]);
            p.line(&[[7.5, 20.0], [7.5, 14.0], [16.5, 14.0], [16.5, 20.0]]);
        }
        Icon::Copy => {
            p.rect(8.5, 8.5, 12.0, 12.0, 2.0);
            p.line(&[
                [4.5, 15.5],
                [3.5, 15.5],
                [3.5, 3.5],
                [15.5, 3.5],
                [15.5, 4.5],
            ]);
        }
        Icon::Download => {
            p.line(&[[12.0, 3.5], [12.0, 15.0]]);
            p.line(&[[7.0, 10.0], [12.0, 15.0], [17.0, 10.0]]);
            p.line(&[[4.0, 15.0], [4.0, 20.0], [20.0, 20.0], [20.0, 15.0]]);
        }
        Icon::Upload => {
            p.line(&[[12.0, 15.0], [12.0, 3.5]]);
            p.line(&[[7.0, 8.5], [12.0, 3.5], [17.0, 8.5]]);
            p.line(&[[4.0, 15.0], [4.0, 20.0], [20.0, 20.0], [20.0, 15.0]]);
        }
        Icon::Link => {
            p.line(&[[10.5, 13.5], [13.5, 10.5]]);
            p.arc(8.0, 16.0, 3.6, 45.0, 225.0);
            p.line(&[[5.5, 13.5], [8.5, 10.5]]);
            p.line(&[[10.5, 18.5], [13.5, 15.5]]);
            p.arc(16.0, 8.0, 3.6, 225.0, 405.0);
            p.line(&[[13.5, 5.5], [10.5, 8.5]]);
            p.line(&[[18.5, 10.5], [15.5, 13.5]]);
        }
        Icon::ExternalLink => {
            p.line(&[
                [18.0, 13.0],
                [18.0, 20.0],
                [4.0, 20.0],
                [4.0, 6.0],
                [11.0, 6.0],
            ]);
            p.line(&[[14.0, 4.0], [20.0, 4.0], [20.0, 10.0]]);
            p.line(&[[10.0, 14.0], [20.0, 4.0]]);
        }
        Icon::Pencil => {
            p.closed(&[
                [4.0, 20.0],
                [5.0, 15.5],
                [16.0, 4.5],
                [19.5, 8.0],
                [8.5, 19.0],
            ]);
            p.line(&[[13.5, 7.0], [17.0, 10.5]]);
        }
        Icon::Trash => {
            p.line(&[[3.5, 6.5], [20.5, 6.5]]);
            p.line(&[[9.0, 6.5], [9.0, 3.5], [15.0, 3.5], [15.0, 6.5]]);
            p.line(&[[5.5, 6.5], [6.5, 20.5], [17.5, 20.5], [18.5, 6.5]]);
        }
        Icon::Undo => {
            p.line(&[[8.0, 4.5], [3.5, 9.0], [8.0, 13.5]]);
            p.line(&[[3.5, 9.0], [14.5, 9.0]]);
            p.arc(14.5, 14.0, 5.0, -90.0, 90.0);
            p.line(&[[14.5, 19.0], [9.0, 19.0]]);
        }
        Icon::Refresh => {
            p.arc(12.0, 12.0, 8.5, 180.0, 320.0);
            p.line(&[[20.5, 3.5], [20.5, 8.5], [15.5, 8.5]]);
            p.arc(12.0, 12.0, 8.5, 0.0, 140.0);
            p.line(&[[3.5, 20.5], [3.5, 15.5], [8.5, 15.5]]);
        }
        Icon::Redo => {
            p.line(&[[16.0, 4.5], [20.5, 9.0], [16.0, 13.5]]);
            p.line(&[[20.5, 9.0], [9.5, 9.0]]);
            p.arc(9.5, 14.0, 5.0, 270.0, 90.0);
            p.line(&[[9.5, 19.0], [15.0, 19.0]]);
        }
        Icon::Settings => {
            p.circle(12.0, 12.0, 3.0);
            let mut points: [Point; 16] = [[0.0; 2]; 16];
            for (i, point) in points.iter_mut().enumerate() {
                let a = i as f32 / 16.0 * std::f32::consts::TAU;
                let r: f32 = if i % 2 == 0 { 9.0 } else { 7.0 };
                *point = [12.0 + a.cos() * r, 12.0 + a.sin() * r];
            }
            p.closed(&points);
        }
        Icon::Sun => {
            p.circle(12.0, 12.0, 4.0);
            for i in 0..8 {
                let a = i as f32 / 8.0 * std::f32::consts::TAU;
                p.line(&[
                    [12.0 + a.cos() * 7.0, 12.0 + a.sin() * 7.0],
                    [12.0 + a.cos() * 9.5, 12.0 + a.sin() * 9.5],
                ]);
            }
        }
        Icon::Moon => {
            // crescent: outer arc of r=9 around (12,12) from 0° to 270°,
            // back along an inner arc bulging towards the centre
            let mut points: [Point; 50] = [[0.0; 2]; 50];
            let mut n: usize = 0;
            for i in 0..25 {
                let a = (i as f32 / 24.0 * 270.0) * RAD_PER_DEG;
                points[n] = [12.0 + a.cos() * 9.0, 12.0 + a.sin() * 9.0];
                n += 1;
            }
            for i in 1..24 {
                let a = (225.0 - i as f32 / 24.0 * 180.0) * RAD_PER_DEG;
                points[n] = [16.5 + a.cos() * 6.364, 7.5 + a.sin() * 6.364];
                n += 1;
            }
            p.closed(&points[..n]);
        }
        Icon::Columns => {
            p.rect(3.5, 4.0, 17.0, 16.0, 2.0);
            p.line(&[[12.0, 4.0], [12.0, 20.0]]);
        }
        Icon::Keyboard => {
            p.rect(2.5, 6.0, 19.0, 12.0, 2.0);
            for x in [6.5_f32, 10.0, 13.5, 17.0] {
                p.dot(x, 9.75, 0.9);
            }
            for x in [8.25_f32, 11.75, 15.25] {
                p.dot(x, 12.25, 0.9);
            }
            p.line(&[[8.0, 15.0], [16.0, 15.0]]);
        }
        Icon::BookOpen => {
            p.closed(&[
                [2.5, 5.0],
                [8.5, 5.0],
                [12.0, 7.5],
                [12.0, 20.0],
                [8.5, 18.0],
                [2.5, 18.0],
            ]);
            p.closed(&[
                [21.5, 5.0],
                [15.5, 5.0],
                [12.0, 7.5],
                [12.0, 20.0],
                [15.5, 18.0],
                [21.5, 18.0],
            ]);
        }
        Icon::Maximize => {
            p.line(&[[4.0, 9.0], [4.0, 4.0], [9.0, 4.0]]);
            p.line(&[[15.0, 4.0], [20.0, 4.0], [20.0, 9.0]]);
            p.line(&[[20.0, 15.0], [20.0, 20.0], [15.0, 20.0]]);
            p.line(&[[9.0, 20.0], [4.0, 20.0], [4.0, 15.0]]);
        }
        Icon::Minimize => {
            p.line(&[[4.0, 9.0], [9.0, 9.0], [9.0, 4.0]]);
            p.line(&[[15.0, 4.0], [15.0, 9.0], [20.0, 9.0]]);
            p.line(&[[20.0, 15.0], [15.0, 15.0], [15.0, 20.0]]);
            p.line(&[[9.0, 20.0], [9.0, 15.0], [4.0, 15.0]]);
        }
        Icon::Focus => {
            p.circle(12.0, 12.0, 3.0);
            p.line(&[[4.0, 8.0], [4.0, 4.0], [8.0, 4.0]]);
            p.line(&[[16.0, 4.0], [20.0, 4.0], [20.0, 8.0]]);
            p.line(&[[20.0, 16.0], [20.0, 20.0], [16.0, 20.0]]);
            p.line(&[[8.0, 20.0], [4.0, 20.0], [4.0, 16.0]]);
        }
        Icon::TypeSize => {
            p.line(&[[3.0, 7.0], [3.0, 5.0], [13.0, 5.0], [13.0, 7.0]]);
            p.line(&[[8.0, 5.0], [8.0, 19.0]]);
            p.line(&[[13.0, 12.0], [13.0, 11.0], [21.0, 11.0], [21.0, 12.0]]);
            p.line(&[[17.0, 11.0], [17.0, 19.0]]);
        }
        Icon::Heading => {
            p.line(&[[6.0, 4.5], [6.0, 19.5]]);
            p.line(&[[18.0, 4.5], [18.0, 19.5]]);
            p.line(&[[6.0, 12.0], [18.0, 12.0]]);
        }
        Icon::Bold => {
            p.line(&[[7.0, 12.0], [13.5, 12.0]]);
            p.arc(13.5, 8.25, 3.75, -90.0, 90.0);
            p.line(&[[13.5, 4.5], [7.0, 4.5], [7.0, 19.5], [14.5, 19.5]]);
            p.arc(14.5, 15.75, 3.75, -90.0, 90.0);
        }
        Icon::Italic => {
            p.line(&[[10.0, 4.5], [19.0, 4.5]]);
            p.line(&[[5.0, 19.5], [14.0, 19.5]]);
            p.line(&[[14.5, 4.5], [9.5, 19.5]]);
        }
        Icon::Strikethrough => {
            p.line(&[[4.0, 12.0], [20.0, 12.0]]);
            p.arc(12.0, 8.0, 4.0, 160.0, 330.0);
            p.arc(12.0, 16.0, 4.0, -30.0, 160.0);
        }
        Icon::Code => {
            p.line(&[[8.5, 6.5], [3.0, 12.0], [8.5, 17.5]]);
            p.line(&[[15.5, 6.5], [21.0, 12.0], [15.5, 17.5]]);
        }
        Icon::CodeBlock => {
            p.rect(3.0, 3.5, 18.0, 17.0, 2.5);
            p.line(&[[10.0, 9.0], [7.0, 12.0], [10.0, 15.0]]);
            p.line(&[[14.0, 9.0], [17.0, 12.0], [14.0, 15.0]]);
        }
        Icon::Quote => {
            p.line(&[[5.0, 5.0], [5.0, 19.0]]);
            p.line(&[[10.0, 8.0], [20.0, 8.0]]);
            p.line(&[[10.0, 12.0], [20.0, 12.0]]);
            p.line(&[[10.0, 16.0], [16.0, 16.0]]);
        }
        Icon::ListBullet => {
            p.line(&[[9.0, 6.0], [20.0, 6.0]]);
            p.line(&[[9.0, 12.0], [20.0, 12.0]]);
            p.line(&[[9.0, 18.0], [20.0, 18.0]]);
            p.dot(4.5, 6.0, 1.2);
            p.dot(4.5, 12.0, 1.2);
            p.dot(4.5, 18.0, 1.2);
        }
        Icon::ListOrdered => {
            p.line(&[[10.0, 6.0], [20.0, 6.0]]);
            p.line(&[[10.0, 12.0], [20.0, 12.0]]);
            p.line(&[[10.0, 18.0], [20.0, 18.0]]);
            p.line(&[[4.0, 4.5], [5.5, 3.5], [5.5, 8.0]]);
            p.line(&[
                [3.8, 10.8],
                [5.0, 10.0],
                [6.4, 10.6],
                [3.8, 14.0],
                [6.6, 14.0],
            ]);
        }
        Icon::ListChecks => {
            p.line(&[[3.0, 6.5], [5.0, 8.5], [8.5, 4.5]]);
            p.line(&[[3.0, 15.5], [5.0, 17.5], [8.5, 13.5]]);
            p.line(&[[12.0, 6.5], [21.0, 6.5]]);
            p.line(&[[12.0, 15.5], [21.0, 15.5]]);
        }
        Icon::Table => {
            p.rect(3.0, 4.0, 18.0, 16.0, 2.0);
            p.line(&[[3.0, 9.5], [21.0, 9.5]]);
            p.line(&[[3.0, 15.0], [21.0, 15.0]]);
            p.line(&[[10.0, 4.0], [10.0, 20.0]]);
        }
        Icon::Sigma => p.line(&[
            [18.0, 6.5],
            [18.0, 4.0],
            [6.0, 4.0],
            [12.5, 12.0],
            [6.0, 20.0],
            [18.0, 20.0],
            [18.0, 17.5],
        ]),
        Icon::Image => {
            p.rect(3.0, 4.0, 18.0, 16.0, 2.5);
            p.circle(8.5, 9.5, 1.8);
            p.line(&[[21.0, 15.5], [16.0, 10.5], [5.0, 20.0]]);
        }
        Icon::MinusRule => {
            p.line(&[[3.0, 12.0], [21.0, 12.0]]);
            p.line(&[[3.0, 6.0], [9.0, 6.0]]);
            p.line(&[[3.0, 18.0], [9.0, 18.0]]);
        }
        Icon::Info => {
            p.circle(12.0, 12.0, 9.0);
            p.line(&[[12.0, 11.0], [12.0, 16.5]]);
            p.dot(12.0, 7.8, 1.1);
        }
        Icon::Alert => {
            p.closed(&[[12.0, 3.5], [21.5, 20.0], [2.5, 20.0]]);
            p.line(&[[12.0, 9.5], [12.0, 14.0]]);
            p.dot(12.0, 17.0, 1.1);
        }
        Icon::Lightbulb => {
            p.arc(12.0, 9.5, 6.0, 140.0, 400.0);
            p.line(&[[7.4, 13.3], [9.0, 16.0], [15.0, 16.0], [16.6, 13.3]]);
            p.line(&[[9.5, 19.5], [14.5, 19.5]]);
        }
        Icon::Clipboard => {
            p.rect(5.0, 4.5, 14.0, 17.0, 2.0);
            p.rect(8.5, 2.5, 7.0, 4.0, 1.2);
        }
        Icon::Monitor => {
            p.rect(3.0, 4.0, 18.0, 12.5, 2.0);
            p.line(&[[12.0, 16.5], [12.0, 20.5]]);
            p.line(&[[8.0, 20.5], [16.0, 20.5]]);
        }
        Icon::Wifi | Icon::WifiOff => {
            p.dot(12.0, 19.0, 1.3);
            p.arc(12.0, 19.5, 4.5, 225.0, 315.0);
            p.arc(12.0, 19.5, 9.0, 225.0, 315.0);
            p.arc(12.0, 19.5, 13.5, 228.0, 312.0);
            if icon == Icon::WifiOff {
                p.line(&[[3.5, 3.5], [20.5, 20.5]]);
            }
        }
        Icon::Clock => {
            p.circle(12.0, 12.0, 9.0);
            p.line(&[[12.0, 7.0], [12.0, 12.0], [15.5, 14.0]]);
        }
        Icon::Power => {
            p.arc(12.0, 13.0, 8.0, 305.0, 595.0);
            p.line(&[[12.0, 3.0], [12.0, 11.5]]);
        }
        // 24 units = the 10px glyph box of Windows 11 caption buttons
        Icon::WindowMinimize => p.line(&[[0.0, 12.0], [24.0, 12.0]]),
        Icon::WindowMaximize => p.rect(0.0, 0.0, 24.0, 24.0, 2.4),
        Icon::WindowRestore => {
            p.rect(0.0, 4.8, 19.2, 19.2, 2.4);
            p.line(&[
                [4.8, 4.8],
                [4.8, 2.4],
                [7.2, 0.0],
                [21.6, 0.0],
                [24.0, 2.4],
                [24.0, 16.8],
                [21.6, 19.2],
                [19.2, 19.2],
            ]);
        }
        Icon::WindowClose => {
            p.line(&[[0.0, 0.0], [24.0, 24.0]]);
            p.line(&[[24.0, 0.0], [0.0, 24.0]]);
        }
    }
}

/// Stroke width in icon units for a given pixel size (≈1.5px at 16px).
pub fn default_stroke(size: f32) -> f32 {
    (size / 24.0 * 2.0).max(1.2)
}

// ── immediate painting (buttons) ──────────────────────────────────────────

/// [`Sink`] that strokes directly on a [`Canvas`].
struct CanvasSink {
    canvas: Canvas,
    /// Top-left of the icon box, in canvas space.
    x: f32,
    y: f32,
    scale: f32,
    color: u32,
    width: f32,
}

impl Sink for CanvasSink {
    fn stroke(&mut self, points: &[Point], closed: bool) {
        // At most 128 points per polyline are painted.
        let count = points.len().min(128);
        let translated: Vec<Point> = points[..count]
            .iter()
            .map(|point| {
                [
                    self.x + point[0] * self.scale,
                    self.y + point[1] * self.scale,
                ]
            })
            .collect();
        self.canvas
            .stroke_path(&translated, self.color, self.width, closed);
    }

    fn dot(&mut self, x: f32, y: f32, r: f32) {
        let rr = r * self.scale + self.width * 0.25;
        let rect = Rect::init(
            self.x + x * self.scale - rr,
            self.y + y * self.scale - rr,
            rr * 2.0,
            rr * 2.0,
        );
        self.canvas.fill_round_rect(rect, rr, self.color);
    }
}

/// Paints `icon` centred at (cx, cy) with a `size`-px box.
pub fn paint(
    canvas: Canvas,
    icon: Icon,
    cx: f32,
    cy: f32,
    size: f32,
    color: u32,
    stroke: Option<f32>,
) {
    let width = stroke.unwrap_or_else(|| default_stroke(size));
    // A translucent colour (disabled / secondary text) would darken wherever
    // the strokes overlap and blur the glyph: draw it opaque into a layer and
    // apply the alpha once.
    let alpha = color >> 24;
    let layered = alpha > 0 && alpha < 0xff;
    if layered {
        let pad = width + 1.0;
        canvas.save_layer_alpha(
            Rect::init(
                cx - size / 2.0 - pad,
                cy - size / 2.0 - pad,
                size + pad * 2.0,
                size + pad * 2.0,
            ),
            alpha as f32 / 255.0,
        );
    }
    let mut sink = CanvasSink {
        canvas,
        x: cx - size / 2.0,
        y: cy - size / 2.0,
        scale: size / 24.0,
        color: if layered { color | 0xff00_0000 } else { color },
        width,
    };
    trace(icon, &mut sink);
    if layered {
        canvas.restore();
    }
}

// ── retained drawings (ui.icon) ───────────────────────────────────────────

/// [`Sink`] that collects [`vector::Command`]s.
struct BuildSink {
    commands: Vec<Command>,
    scale: f32,
    top: f32,
    color: u32,
    width: f32,
}

impl Sink for BuildSink {
    fn stroke(&mut self, points: &[Point], closed: bool) {
        let translated: Vec<Point> = points
            .iter()
            .map(|point| [point[0] * self.scale, point[1] * self.scale - self.top])
            .collect();
        self.commands.push(Command::Path(Path {
            points: translated,
            color: self.color,
            stroke_width: self.width,
            closed,
        }));
    }

    fn dot(&mut self, x: f32, y: f32, r: f32) {
        let rr = r * self.scale + self.width * 0.25;
        self.commands.push(Command::Rect(DrawingRect {
            x: x * self.scale - rr,
            y: y * self.scale - self.top - rr,
            width: rr * 2.0,
            height: rr * 2.0,
            radius: rr,
            color: self.color,
        }));
    }
}

/// A `size`×`size` drawing whose box sits entirely above the baseline.
pub fn drawing(icon: Icon, size: f32, color: u32, stroke: Option<f32>) -> Drawing {
    let width = stroke.unwrap_or_else(|| default_stroke(size));
    let mut sink = BuildSink {
        commands: Vec::new(),
        scale: size / 24.0,
        top: size,
        color,
        width,
    };
    trace(icon, &mut sink);
    let BuildSink { commands, .. } = sink;

    // Wyhash (seed 0x69636f6e) over the discriminant as a `u32`
    // (`#[repr(u32)]`), `size`, `color` and the stroke width. This is an
    // internal cache key, so only stability matters.
    let mut hasher = Wyhash::init(0x6963_6f6e);
    hasher.update(&(icon as u32).to_le_bytes());
    hasher.update(&size.to_le_bytes());
    hasher.update(&color.to_le_bytes());
    hasher.update(&width.to_le_bytes());
    Drawing {
        width: size,
        ascent: size,
        descent: 0.0,
        commands,
        hash: hasher.finish(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::renderer::Renderer;

    #[test]
    fn every_icon_traces() {
        for icon in Icon::ALL {
            let drawing = drawing(icon, 18.0, 0xff00_0000, None);
            if icon != Icon::None {
                assert!(
                    !drawing.commands.is_empty(),
                    "{icon:?} produced no commands"
                );
            }
        }
    }

    #[test]
    fn painting_moves_pixels() {
        let mut renderer = Renderer::create_raster(64, 64).expect("raster renderer");
        {
            let canvas = renderer.begin(1.0).expect("frame canvas");
            canvas.clear(0xffff_ffff);
            paint(canvas, Icon::Check, 32.0, 32.0, 32.0, 0xff00_0000, None);
        }
        renderer.end();
        let mut changed = 0;
        for y in 0..64 {
            for x in 0..64 {
                if renderer.pixel_at(x, y) != Some(0xffff_ffff) {
                    changed += 1;
                }
            }
        }
        assert!(
            changed > 0,
            "painting the icon must move at least one pixel"
        );
    }
}
