//! PDF: documents, text geometry and the viewer (`zant.pdf.view`).
//! macOS uses PDFKit, Windows PDFium (`pdfium.dll` next to the exe).
const legacy = @import("../platform/pdf.zig");

pub const Document = @import("document.zig").Document;
pub const Link = @import("document.zig").Link;
pub const Size = @import("document.zig").Size;
pub const OutlineEntry = @import("document.zig").OutlineEntry;
pub const OpenError = @import("document.zig").OpenError;
pub const text = @import("text.zig");
/// Raw glyph record of the native backends (tests, custom geometry).
pub const native_glyph = @import("native.zig").Glyph;
pub const Renderer = @import("renderer.zig").Renderer;

const viewer = @import("viewer.zig");
pub const Viewer = viewer.Viewer;
pub const ViewOptions = viewer.ViewOptions;
pub const Labels = viewer.Labels;
pub const Fit = viewer.Fit;
pub const Pos = viewer.Pos;
pub const Range = viewer.Range;
pub const Match = viewer.Match;
pub const zoom_steps = viewer.zoom_steps;
pub const view = viewer.view;
pub const Tool = viewer.Tool;
pub const annotations = viewer.annots;
pub const Annotation = viewer.annots.Annotation;

/// Path-based helpers used by `ui.pdfPreview` (single page preview).
pub const pageCount = legacy.pageCount;
pub const renderPage = legacy.renderPage;
pub const pageText = legacy.pageText;
