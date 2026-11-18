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

mod pen;
mod sinks;
#[cfg(test)]
mod tests;
mod trace;

use self::pen::*;
pub use self::sinks::*;
pub use self::trace::*;

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
