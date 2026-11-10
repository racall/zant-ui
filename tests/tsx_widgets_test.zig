//! Stateful document tags: generated app, real PDF backend and slot lifetime.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const widgets = ui.widgets;
const tsx = zant.tsx;
const Runtime = zant.ui_runtime.Runtime;
const gen = @import("tsx_golden_widgets");

fn render(ctx: *anyopaque) ui.Node {
    const app: *gen.App = @ptrCast(@alignCast(ctx));
    return app.render();
}

fn state(runtime: *Runtime, comptime T: type, id: u64) ?*T {
    const entry = runtime.local_states.entries.get(id) orelse return null;
    return @ptrCast(@alignCast(entry.ptr));
}

const markdown_id = tsx.childId(tsx.childId(0, "Reply", "answer"), "Markdown", "reply");
const pdf_id = tsx.childId(tsx.childId(0, "Document", "file"), "PdfViewer", "document");

test "tsx widgets retain caches, reopen PDFs and release slots on unmount" {
    var app = gen.App{};
    defer app.source.deinit();
    defer app.path.deinit();
    const runtime = try Runtime.init(std.testing.allocator, &app, render, .{});
    defer runtime.deinit();
    runtime.rebuildForTest();
    runtime.rebuildForTest(); // PDF open effect schedules the content build.
    // Stateless wrapper components carry ids but do not allocate their own slots.
    try std.testing.expectEqual(@as(usize, 4), runtime.local_states.count());
    const first = state(runtime, widgets.MarkdownState, markdown_id).?;
    const viewer = state(runtime, widgets.PdfViewerState, pdf_id).?;
    try std.testing.expectEqual(@as(usize, 1), viewer.viewer.pageCount());
    const doc = viewer.viewer.document().?;
    var key_buf: [48]u8 = undefined;
    const zoom_key = try std.fmt.bufPrint(&key_buf, "{s}-zin", .{viewer.prefix[0..viewer.prefix_len]});
    const zoom_button = runtime.tree.idOfKey(zoom_key).?;
    const zoom_action = runtime.tree.get(zoom_button).data.button.action.?;
    zoom_action.func(zoom_action.ctx);
    try std.testing.expectEqual(@as(f64, 1), app.changes);
    const zoom = viewer.viewer.zoom;
    const entries = first.cache.entries.items;
    const head = entries[0];
    const tail_id = entries[2].id;

    for ([_][]const u8{
        "# Title\n\nFirst paragraph.\n\nSecond **bold**",
        "# Title\n\nFirst paragraph.\n\nSecond **bold** grows",
        "# Title\n\nFirst paragraph.\n\nSecond **bold** grows\n\nThird",
    }) |source| {
        app.source.set(source);
        runtime.rebuildForTest();
        try std.testing.expectEqual(first, state(runtime, widgets.MarkdownState, markdown_id).?);
        try std.testing.expectEqual(head, first.cache.entries.items[0]);
        try std.testing.expectEqual(tail_id, first.cache.entries.items[2].id);
        try std.testing.expect(first.cache.parsed_last <= 2);
        try std.testing.expectEqual(doc, viewer.viewer.document().?);
        try std.testing.expectEqual(zoom, viewer.viewer.zoom);
    }
    runtime.rebuildForTest();
    try std.testing.expectEqual(@as(usize, 0), first.cache.parsed_last);

    // Reopening replaces the document and clears selection / annotations.
    viewer.viewer.zoom = 2;
    viewer.viewer.scroll_y = 50;
    viewer.viewer.setSelection(.{ .page = 0, .index = 0 }, .{ .page = 0, .index = 3 });
    app.path.set("tests/fixtures/no-such-widget.pdf");
    runtime.rebuildForTest();
    try std.testing.expect(viewer.viewer.document() == null);
    try std.testing.expect(viewer.viewer.open_error != null);
    try std.testing.expect(viewer.viewer.selection() == null);
    try std.testing.expectEqual(@as(f32, 0), viewer.viewer.scroll_y);
    runtime.rebuildForTest();
    try std.testing.expect(!viewer.pending); // Failed path is not opened again.
    app.path.set("tests/fixtures/widgets.pdf");
    runtime.rebuildForTest();
    try std.testing.expectEqual(@as(usize, 1), viewer.viewer.pageCount());
    try std.testing.expectEqual(viewer, state(runtime, widgets.PdfViewerState, pdf_id).?);

    app.visible = false;
    runtime.rebuildForTest();
    try std.testing.expectEqual(@as(usize, 2), runtime.local_states.count());
    try std.testing.expect(state(runtime, widgets.MarkdownState, markdown_id) == null);
    try std.testing.expect(state(runtime, widgets.PdfViewerState, pdf_id) == null);
    app.visible = true;
    runtime.rebuildForTest();
    try std.testing.expectEqual(@as(usize, 4), runtime.local_states.count());
    const fresh = state(runtime, widgets.PdfViewerState, pdf_id).?;
    try std.testing.expectEqual(@as(f32, 1), fresh.viewer.zoom);
    try std.testing.expectEqual(@as(usize, 1), fresh.viewer.pageCount());
}

const Manual = struct {
    visible: bool = true,
    password: ?[]const u8 = null,

    fn render(ctx: *anyopaque) ui.Node {
        const self: *@This() = @ptrCast(@alignCast(ctx));
        if (!self.visible) return ui.view(.{});
        return ui.column(.{ .children = &.{
            ui.styled(ui.markdown(11, .{ .source = "**Hello**" }), .{ .width = 210, .padding = ui.inset(8) }),
            ui.styled(ui.pdfViewer(12, .{
                .src = "tests/fixtures/widgets.pdf",
                .password = self.password,
                .toolbar = false,
                .height = 180,
            }), .{ .width = 260, .height = 200, .background = ui.currentTheme().color_fill_secondary }),
        } });
    }
};

test "Zig widgets support styled overrides, themes, sizes and password changes" {
    var app = Manual{};
    const runtime = try Runtime.init(std.testing.allocator, &app, Manual.render, .{});
    defer runtime.deinit();
    var renderer = try zant.renderer.Renderer.createRaster(800, 700);
    defer renderer.deinit();
    for ([_]zant.tokens.Theme{ zant.tokens.antLight(), zant.tokens.antDark() }) |theme| {
        runtime.setTheme(theme);
        for ([_][2]f32{ .{ 800, 700 }, .{ 360, 420 } }) |size| {
            runtime.needs_build = true;
            const canvas = renderer.begin(1).?;
            runtime.renderFrame(canvas, size[0], size[1], 1);
            renderer.end();
            const md = runtime.tree.boundsOfKey("md-b").?;
            const pdf = runtime.tree.boundsOfKey("pdf-c").?;
            try std.testing.expectEqual(@as(f32, 210), md.width);
            try std.testing.expectEqual(@as(f32, 260), pdf.width);
            try std.testing.expectEqual(@as(f32, 200), pdf.height);
        }
    }
    const pdf = state(runtime, widgets.PdfViewerState, 12).?;
    pdf.viewer.scroll_y = 60;
    app.password = "changed";
    runtime.rebuildForTest();
    try std.testing.expectEqualStrings("changed", pdf.password.?);
    try std.testing.expectEqual(@as(f32, 0), pdf.viewer.scroll_y);
    app.visible = false;
    runtime.rebuildForTest();
    try std.testing.expectEqual(@as(usize, 0), runtime.local_states.count());
}
