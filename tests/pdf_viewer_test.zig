//! zant.pdf: text geometry (EmbedPDF selection algorithms) and the viewer
//! (selection by dragging, word selection, copy text, search).
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const text = zant.pdf.text;
const Runtime = zant.ui_runtime.Runtime;

fn glyph(x: f32, y: f32, w: f32, cp: u21) zant.pdf.native_glyph {
    return .{ .x = x, .y = y, .width = w, .height = 10, .tight_x = x, .tight_y = y + 1, .tight_width = w, .tight_height = 8, .codepoint = cp, .font_size = 10, .run = -1, .flags = if (cp == ' ') 1 else if (cp == '\n') 2 else 0 };
}

fn line(buf: []zant.pdf.native_glyph, start: usize, s: []const u8, y: f32) usize {
    var x: f32 = 10;
    for (s, 0..) |c, i| {
        buf[start + i] = glyph(x, y, 6, c);
        if (c == '\n') buf[start + i].width = 0;
        x += 6;
    }
    return start + s.len;
}

test "pdf text: runs, hit testing, rects, words, lines, search" {
    var raw: [64]zant.pdf.native_glyph = undefined;
    var n = line(&raw, 0, "Hello world\n", 20);
    n = line(&raw, n, "second line", 40);
    var page = try text.PageText.build(std.testing.allocator, raw[0..n]);
    defer page.deinit(std.testing.allocator);
    // two visual lines → two runs (geometry grouping)
    try std.testing.expectEqual(@as(usize, 2), page.runs.len);
    // 'w' of "world" is glyph 6 at x = 46..52
    try std.testing.expectEqual(@as(?usize, 6), page.glyphAt(48, 25, 1.5));
    // tolerance: a little above the line still hits
    try std.testing.expect(page.glyphAt(48, 19.5, 1.5) != null);
    try std.testing.expectEqual(@as(?usize, null), page.glyphAt(300, 300, 1.5));
    // word / line expansion
    const w = page.wordAt(7).?;
    try std.testing.expectEqual(@as(usize, 6), w.from);
    try std.testing.expectEqual(@as(usize, 10), w.to);
    const l = page.lineAt(14).?;
    try std.testing.expectEqual(@as(usize, 12), l.from);
    try std.testing.expectEqual(@as(usize, 22), l.to);
    // a range across both lines gives one rect per line
    const rects = try page.rects(std.testing.allocator, 6, 17);
    defer std.testing.allocator.free(rects);
    try std.testing.expectEqual(@as(usize, 2), rects.len);
    try std.testing.expectApproxEqAbs(@as(f32, 46), rects[0].x, 0.01);
    // text and search (case-insensitive, line breaks match spaces)
    var out: std.ArrayListUnmanaged(u8) = .empty;
    defer out.deinit(std.testing.allocator);
    try page.appendText(std.testing.allocator, &out, 6, 17);
    try std.testing.expectEqualStrings("world\nsecond", out.items);
    var found: std.ArrayListUnmanaged(text.Slice) = .empty;
    defer found.deinit(std.testing.allocator);
    try page.search(std.testing.allocator, "WORLD  second", &found);
    try std.testing.expectEqual(@as(usize, 1), found.items.len);
    try std.testing.expectEqual(@as(usize, 6), found.items[0].from);
    try std.testing.expectEqual(@as(usize, 17), found.items[0].to);
}

fn minimalPdf(buf: []u8) ![]const u8 {
    const objects = [_][]const u8{
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R 6 0 R] /Count 2 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
        "<< /Length 46 >>\nstream\nBT /F1 24 Tf 20 120 Td (Hello Zant PDF) Tj ET\nendstream",
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Contents 7 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
        "<< /Length 43 >>\nstream\nBT /F1 24 Tf 20 120 Td (Second page) Tj ET\nendstream",
    };
    var w: std.Io.Writer = .fixed(buf);
    try w.writeAll("%PDF-1.4\n");
    var offsets: [objects.len]usize = undefined;
    for (objects, 0..) |body, i| {
        offsets[i] = w.end;
        try w.print("{d} 0 obj\n{s}\nendobj\n", .{ i + 1, body });
    }
    const xref = w.end;
    try w.print("xref\n0 {d}\n0000000000 65535 f \n", .{objects.len + 1});
    for (offsets) |off| try w.print("{d:0>10} 00000 n \n", .{off});
    try w.print("trailer\n<< /Size {d} /Root 1 0 R >>\nstartxref\n{d}\n%%EOF\n", .{ objects.len + 1, xref });
    return w.buffered();
}

const App = struct {
    viewer: *zant.pdf.Viewer,
    fn renderOpaque(ctx: *anyopaque) ui.Node {
        return render(@ptrCast(@alignCast(ctx)));
    }
    fn render(self: *App) ui.Node {
        return ui.column(.{ .grow = 1, .align_items = .stretch, .children = &.{zant.pdf.view(self.viewer, .{ .key = "pdf", .toolbar = false })} });
    }
};

fn frame(runtime: *Runtime, renderer: *zant.renderer.Renderer) void {
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, 400, 600, 1.0);
    renderer.end();
}

test "pdf viewer: open, layout, drag selection, word selection, copy text, search" {
    var buf: [4096]u8 = undefined;
    const data = try minimalPdf(&buf);
    const path = "zig-out/test-viewer.pdf";
    std.Io.Dir.cwd().createDirPath(std.testing.io, "zig-out") catch {};
    try zant.dialogs.writeFile(path, data);

    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var viewer = zant.pdf.Viewer.init(std.testing.allocator);
    defer viewer.deinit();
    try viewer.open(path);
    try std.testing.expectEqual(@as(usize, 2), viewer.pageCount());

    var app = App{ .viewer = &viewer };
    const runtime = try Runtime.init(std.testing.allocator, &app, App.renderOpaque, .{ .window = .{ .width = 400, .height = 600 } });
    defer runtime.deinit();
    var renderer = try zant.renderer.Renderer.createRaster(400, 600);
    defer renderer.deinit();
    frame(runtime, &renderer);
    frame(runtime, &renderer);
    // fit width: 300pt page in a 400px viewport with 16px padding
    try std.testing.expectApproxEqAbs(@as(f32, 368.0 / 300.0), viewer.zoom, 0.01);

    const doc = viewer.document().?;
    const t = doc.pageText(0).?;
    try std.testing.expect(t.count() >= 14);
    // glyph boxes of "Hello" and "PDF" → viewport points
    var first: ?usize = null;
    var last: ?usize = null;
    for (t.glyphs, 0..) |g, i| {
        if (g.codepoint == 'H' and first == null) first = i;
        if (g.codepoint == 'F') last = i;
    }
    const z = viewer.zoom;
    const page_x: f32 = (400 - 300 * z) / 2;
    const page_y: f32 = 16;
    const a = t.glyphs[first.?].box;
    const b = t.glyphs[last.?].box;
    const ax = page_x + (a.x + 1) * z;
    const ay = page_y + (a.y + a.height / 2) * z;
    const bx = page_x + (b.x + b.width - 1) * z;
    _ = runtime.dispatch(.{ .pointer_down = .{ .x = ax, .y = ay } });
    _ = runtime.dispatch(.{ .pointer_move = .{ .x = (ax + bx) / 2, .y = ay } });
    _ = runtime.dispatch(.{ .pointer_move = .{ .x = bx, .y = ay } });
    _ = runtime.dispatch(.{ .pointer_up = .{ .x = bx, .y = ay } });
    const selected = try viewer.selectedText(std.testing.allocator);
    defer std.testing.allocator.free(selected);
    try std.testing.expectEqualStrings("Hello Zant PDF", selected);
    frame(runtime, &renderer);

    // a click without a drag clears it; a double click selects a word
    _ = runtime.dispatch(.{ .pointer_down = .{ .x = ax, .y = ay } });
    _ = runtime.dispatch(.{ .pointer_up = .{ .x = ax, .y = ay } });
    try std.testing.expect(viewer.selection() == null);
    _ = runtime.dispatch(.{ .pointer_down = .{ .x = ax, .y = ay } });
    _ = runtime.dispatch(.{ .pointer_up = .{ .x = ax, .y = ay } });
    const word = try viewer.selectedText(std.testing.allocator);
    defer std.testing.allocator.free(word);
    try std.testing.expectEqualStrings("Hello", word);

    // search across pages
    viewer.search("second PAGE");
    frame(runtime, &renderer);
    try std.testing.expectEqual(@as(usize, 1), viewer.searchResults().len);
    try std.testing.expectEqual(@as(usize, 1), viewer.searchResults()[0].page);
    try std.testing.expectEqual(@as(?usize, 0), viewer.active_match);
}

test "pdf viewer: annotations (markup from selection, ink, undo / redo, annotated export)" {
    var buf: [4096]u8 = undefined;
    const data = try minimalPdf(&buf);
    const path = "zig-out/test-annot.pdf";
    std.Io.Dir.cwd().createDirPath(std.testing.io, "zig-out") catch {};
    try zant.dialogs.writeFile(path, data);

    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var viewer = zant.pdf.Viewer.init(std.testing.allocator);
    defer viewer.deinit();
    try viewer.open(path);
    var app = App{ .viewer = &viewer };
    const runtime = try Runtime.init(std.testing.allocator, &app, App.renderOpaque, .{ .window = .{ .width = 400, .height = 600 } });
    defer runtime.deinit();
    var renderer = try zant.renderer.Renderer.createRaster(400, 600);
    defer renderer.deinit();
    frame(runtime, &renderer);
    frame(runtime, &renderer);

    viewer.selectAll();
    const id = viewer.annotateSelection(.highlight, null) orelse return error.NoAnnotation;
    // one per page with text, the selection is consumed
    try std.testing.expectEqual(@as(usize, 2), viewer.annotations.all().len);
    try std.testing.expect(viewer.selection() == null);
    try std.testing.expectEqualStrings("Second page", viewer.annotations.get(id).?.text);
    frame(runtime, &renderer);

    // ink: drag with the pen tool on page 1
    viewer.setTool(.ink);
    _ = runtime.dispatch(.{ .pointer_down = .{ .x = 100, .y = 60 } });
    _ = runtime.dispatch(.{ .pointer_move = .{ .x = 140, .y = 80 } });
    _ = runtime.dispatch(.{ .pointer_move = .{ .x = 180, .y = 70 } });
    _ = runtime.dispatch(.{ .pointer_up = .{ .x = 180, .y = 70 } });
    try std.testing.expectEqual(@as(usize, 3), viewer.annotations.all().len);
    const ink = viewer.annotations.all()[2];
    try std.testing.expectEqual(zant.pdf.annotations.Kind.ink, ink.kind);
    try std.testing.expectEqual(@as(usize, 3), ink.strokes[0].len);
    frame(runtime, &renderer);

    // undo / redo
    try std.testing.expect(viewer.undo());
    try std.testing.expectEqual(@as(usize, 2), viewer.annotations.all().len);
    try std.testing.expect(viewer.redo());
    try std.testing.expectEqual(@as(usize, 3), viewer.annotations.all().len);

    // clicking an annotation with the select tool selects it; Delete removes it
    viewer.setTool(.select);
    frame(runtime, &renderer);
    _ = runtime.dispatch(.{ .pointer_down = .{ .x = 140, .y = 80 } });
    _ = runtime.dispatch(.{ .pointer_up = .{ .x = 140, .y = 80 } });
    try std.testing.expectEqual(ink.id, viewer.selected_annotation.?);
    _ = runtime.dispatch(.{ .key_down = .{ .key = .delete } });
    try std.testing.expectEqual(@as(usize, 2), viewer.annotations.all().len);
    try std.testing.expect(viewer.selected_annotation == null);

    // annotated copy: a valid PDF that still opens, larger than the source
    viewer.setAnnotationContents(id, "remember this");
    const out_path = "zig-out/test-annot-out.pdf";
    try viewer.saveAnnotated(out_path);
    const saved = try zant.pdf.Document.open(std.testing.allocator, out_path, null);
    defer saved.close();
    try std.testing.expectEqual(@as(usize, 2), saved.pageCount());
    const stat = try std.Io.Dir.cwd().statFile(std.testing.io, out_path, .{});
    try std.testing.expect(stat.size > data.len);
}
