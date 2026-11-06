pub const geometry = @import("core/geometry.zig");
pub const unicode = @import("core/unicode.zig");
pub const node = @import("core/node.zig");
pub const icons = @import("core/icons.zig");
pub const vector = @import("core/vector.zig");
pub const hash = @import("core/hash.zig");
pub const motion = @import("motion/motion.zig");
pub const actions = @import("core/actions.zig");
pub const tree = @import("core/tree.zig");
pub const layout = @import("core/layout.zig");
pub const flex = @import("layout/flex.zig");
pub const events = @import("core/events.zig");
/// Platform-aware shortcuts: primary modifier (⌘ / Ctrl) and labels.
pub const keys = @import("core/keys.zig");
/// Per-user app data directories.
pub const paths = @import("core/paths.zig");
pub const tokens = @import("theme/tokens.zig");
pub const canvas = @import("renderer/canvas.zig");
pub const renderer = @import("renderer/renderer.zig");
pub const text = @import("renderer/text.zig");
pub const button = @import("components/button.zig");
pub const input = @import("components/input.zig");
pub const platform = @import("platform/platform.zig");
pub const app = @import("app.zig");
pub const gallery = @import("gallery.zig");
pub const ui = @import("ui.zig");
pub const ui_runtime = @import("ui_runtime.zig");
pub const toggle = @import("components/toggle.zig");
pub const select = @import("components/select.zig");
pub const store = @import("store.zig");
pub const default_theme = @import("theme/default.zig");
pub const launch = @import("launch.zig");

pub const WindowOptions = platform.WindowOptions;

pub const db = @import("db/mod.zig");

pub const markdown = @import("markdown/mod.zig");

test {
    _ = markdown;
    _ = math;
    _ = keys;
    _ = paths;
}

pub const math = @import("math/mod.zig");

pub const dialogs = @import("platform/dialog.zig");
/// PDF pages for `ui.pdfPreview` (PDFKit on macOS, PDFium on Windows).
pub const pdf = @import("platform/pdf.zig");
