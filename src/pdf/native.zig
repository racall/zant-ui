//! C ABI of the native PDF backends (src/platform/pdf.h): PDFKit on macOS,
//! PDFium on Windows. All functions are thread-safe.
pub const Doc = opaque {};

pub const Glyph = extern struct {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    tight_x: f32,
    tight_y: f32,
    tight_width: f32,
    tight_height: f32,
    codepoint: u32,
    font_size: f32,
    run: c_int,
    flags: c_int,
};

pub const Link = extern struct {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    dest_page: c_int,
    dest_y: f32,
    uri: [1024]u8,
};

pub const OutlineItem = extern struct {
    depth: c_int,
    page: c_int,
    title: [256]u8,
};

pub extern fn zant_pdf_open(path: [*:0]const u8, password: ?[*:0]const u8, locked: ?*c_int) ?*Doc;
pub extern fn zant_pdf_close(doc: *Doc) void;
pub extern fn zant_pdf_count(doc: *Doc) c_int;
pub extern fn zant_pdf_size(doc: *Doc, page: c_int, width: *f32, height: *f32) c_int;
pub extern fn zant_pdf_render(doc: *Doc, page: c_int, page_width: c_int, page_height: c_int, x: c_int, y: c_int, width: c_int, height: c_int, pixels: [*]u8, stride: c_int) c_int;
pub extern fn zant_pdf_glyphs(doc: *Doc, page: c_int, out: ?[*]Glyph, capacity: c_int) c_int;
pub extern fn zant_pdf_links(doc: *Doc, page: c_int, out: ?[*]Link, capacity: c_int) c_int;
pub extern fn zant_pdf_outline(doc: *Doc, out: ?[*]OutlineItem, capacity: c_int) c_int;

pub const Annotation = extern struct {
    page: c_int,
    kind: c_int,
    color: u32,
    width: f32,
    rects: ?[*]const f32 = null,
    rect_count: c_int = 0,
    points: ?[*]const f32 = null,
    stroke_lengths: ?[*]const c_int = null,
    stroke_count: c_int = 0,
    contents: ?[*:0]const u8 = null,
};

pub extern fn zant_pdf_save_annotated(source_path: [*:0]const u8, password: ?[*:0]const u8, out_path: [*:0]const u8, annotations: [*]const Annotation, count: c_int) c_int;
