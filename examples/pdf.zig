//! PDF viewer example: `zig build run-pdf -- path/to/file.pdf`.
//!
//! `zant.pdf.view` with continuous scrolling, zoom (toolbar, ⌘/Ctrl + wheel,
//! pinch), text selection and copy, search and links. ZANT_PDF_PAGE picks
//! the first page shown (0-based; "last" for the last page),
//! ZANT_PDF_ZOOM a manual zoom (e.g. 1.5), ZANT_PDF_SEARCH an initial
//! search, ZANT_PDF_DEMO=1 adds sample annotations, and ZANT_PDF_PROBE=1
//! prints document facts without a window. ⌘S / Ctrl+S saves an annotated
//! copy.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;

const State = struct {
    viewer: *zant.pdf.Viewer,
    dark: bool = false,

    fn render(self: *State) ui.Node {
        return ui.column(.{
            .grow = 1,
            .align_items = .stretch,
            .children = &.{zant.pdf.view(self.viewer, .{ .key = "pdf", .labels = .zh, .annotation_tools = true })},
        });
    }

    pub fn onKey(self: *State, key: zant.events.KeyEvent) bool {
        if (!key.modifiers.primary() or !zant.events.characterEquals(key.key, 's')) return false;
        const gpa = self.viewer.allocator;
        const out = (zant.dialogs.saveFileAs(gpa, "annotated.pdf", "pdf") catch return true) orelse return true;
        defer gpa.free(out);
        self.viewer.saveAnnotated(out) catch |err| std.debug.print("save failed: {s}\n", .{@errorName(err)});
        return true;
    }

    pub fn theme(self: *State) zant.tokens.Theme {
        return if (self.dark) zant.tokens.antDark() else zant.tokens.antLight();
    }
};

pub fn main(init: std.process.Init) !void {
    const args = try init.minimal.args.toSlice(init.arena.allocator());
    if (args.len < 2) {
        std.debug.print("usage: zant-pdf <file.pdf>\n", .{});
        return;
    }
    const path = args[1];
    if (init.environ_map.get("ZANT_PDF_PROBE") != null) {
        const doc = zant.pdf.Document.open(init.gpa, path, null) catch |err| {
            std.debug.print("open failed: {s}\n", .{@errorName(err)});
            return;
        };
        defer doc.close();
        const count = doc.pageCount();
        const size = if (count > 0) doc.pageSize(0) else zant.pdf.Size{ .width = 0, .height = 0 };
        const t = if (count > 0) doc.pageText(0) else null;
        const glyphs = if (t) |pt| pt.count() else 0;
        const runs = if (t) |pt| pt.runs.len else 0;
        const first = try doc.pageTextString(init.gpa, 0);
        defer init.gpa.free(first);
        var links: usize = 0;
        for (0..count) |i| links += doc.pageLinks(i).len;
        std.debug.print("pages={d} size0={d:.1}x{d:.1} glyphs0={d} runs0={d} text0={d} links={d} outline={d}\n", .{ count, size.width, size.height, glyphs, runs, first.len, links, doc.outline().len });
        return;
    }
    const viewer = try init.gpa.create(zant.pdf.Viewer);
    viewer.* = .init(init.gpa);
    defer {
        viewer.deinit();
        init.gpa.destroy(viewer);
    }
    var state = State{ .viewer = viewer, .dark = init.environ_map.get("ZANT_DARK") != null };
    state.viewer.open(path) catch {};
    if (init.environ_map.get("ZANT_PDF_ZOOM")) |z| state.viewer.setZoom(std.fmt.parseFloat(f32, z) catch 1);
    if (init.environ_map.get("ZANT_PDF_PAGE")) |page| {
        const n = state.viewer.pageCount();
        state.viewer.goToPage(if (std.mem.eql(u8, page, "last")) n -| 1 else std.fmt.parseInt(usize, page, 10) catch 0, 0);
    }
    if (init.environ_map.get("ZANT_PDF_SEARCH")) |q| state.viewer.search(q);
    if (init.environ_map.get("ZANT_PDF_DEMO") != null) demoAnnotations(viewer);
    try zant.launch.run(init.gpa, state, State.render, .{
        .title = "Zant PDF",
        .window = .{ .width = 900, .height = 900, .min_width = 480, .min_height = 480 },
    });
}

/// Sample annotations on the first page with text (visual checks).
fn demoAnnotations(v: *zant.pdf.Viewer) void {
    const doc = v.document() orelse return;
    for (0..doc.pageCount()) |page| {
        const t = doc.pageText(page) orelse continue;
        if (t.count() < 400) continue;
        const kinds = [_]zant.pdf.annotations.Kind{ .highlight, .underline, .strikeout, .squiggly };
        for (kinds, 0..) |kind, i| {
            v.setSelection(.{ .page = page, .index = 60 + i * 80 }, .{ .page = page, .index = 100 + i * 80 });
            _ = v.annotateSelection(kind, null);
        }
        const size = doc.pageSize(page);
        var stroke: [40][2]f32 = undefined;
        for (&stroke, 0..) |*p, i| {
            const f: f32 = @floatFromInt(i);
            p.* = .{ size.width * 0.15 + f * 6, size.height * 0.7 + @sin(f / 3) * 20 };
        }
        const strokes = [_][]const [2]f32{&stroke};
        _ = v.annotations.add(.{ .id = 0, .page = page, .kind = .ink, .color = 0xffe5484d, .strokes = &strokes, .width = 2 }) catch {};
        const note = [_]zant.pdf.annotations.Rect{.{ .x = size.width * 0.85, .y = size.height * 0.2, .width = 18, .height = 18 }};
        _ = v.annotations.add(.{ .id = 0, .page = page, .kind = .note, .color = 0xffffc53d, .rects = &note, .contents = "note" }) catch {};
        v.goToPage(page, 0);
        v.setSelection(.{ .page = page, .index = 420 }, .{ .page = page, .index = 470 });
        return;
    }
}
