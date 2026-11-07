//! Text selection (`BoxOptions.selectable`) in rendered Markdown: drag
//! across blocks, word / block clicks, copied text (formulas as LaTeX,
//! markers, gutters and controls left out).
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const md = zant.markdown;
const Runtime = zant.ui_runtime.Runtime;

const Doc = struct {
    cache: md.Cache,
    source: []const u8,

    fn renderOpaque(ctx: *anyopaque) ui.Node {
        const self: *Doc = @ptrCast(@alignCast(ctx));
        return ui.column(.{ .grow = 1, .padding = ui.inset(20), .align_items = .stretch, .children = &.{
            md.streamdown(&self.cache, self.source, .{ .render = .{ .key_prefix = "doc", .content_width = 560 } }),
        } });
    }
};

fn frame(runtime: *Runtime, renderer: *zant.renderer.Renderer) void {
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, 600, 700, 1.0);
    renderer.end();
}

fn firstRich(runtime: *Runtime) ?*zant.node.Node {
    for (runtime.tree.nodes.items) |node| {
        if (!node.alive) continue;
        if (node.data == .rich_text) return node;
    }
    return null;
}

test "selection: drag across Markdown blocks copies clean text" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var doc = Doc{
        .cache = md.Cache.init(std.testing.allocator),
        .source = "Hello **world** and $x^2$ math\n\n- item one\n- item two\n\n```js\nlet a = 1\n```\n\nLast line",
    };
    defer doc.cache.deinit();
    const runtime = try Runtime.init(std.testing.allocator, &doc, Doc.renderOpaque, .{ .window = .{ .width = 600, .height = 700 } });
    defer runtime.deinit();
    var renderer = try zant.renderer.Renderer.createRaster(600, 700);
    defer renderer.deinit();
    frame(runtime, &renderer);

    const para = firstRich(runtime) orelse return error.NoText;
    const b = para.bounds;
    // press at the very start of the first paragraph, drag far below
    _ = runtime.dispatch(.{ .pointer_down = .{ .x = b.x + 1, .y = b.y + b.height / 2 } });
    _ = runtime.dispatch(.{ .pointer_move = .{ .x = 300, .y = 400 } });
    _ = runtime.dispatch(.{ .pointer_move = .{ .x = 590, .y = 690 } });
    _ = runtime.dispatch(.{ .pointer_up = .{ .x = 590, .y = 690 } });
    const all = try runtime.selectedText(std.testing.allocator);
    defer std.testing.allocator.free(all);
    try std.testing.expectEqualStrings("Hello world and $x^2$ math\nitem one\nitem two\nlet a = 1\nLast line", all);
    frame(runtime, &renderer);
    try std.testing.expect(para.text_sel != null);

    // a plain click clears it
    _ = runtime.dispatch(.{ .pointer_down = .{ .x = b.x + 1, .y = b.y + b.height / 2 } });
    _ = runtime.dispatch(.{ .pointer_up = .{ .x = b.x + 1, .y = b.y + b.height / 2 } });
    const none = try runtime.selectedText(std.testing.allocator);
    defer std.testing.allocator.free(none);
    try std.testing.expectEqualStrings("", none);
    try std.testing.expect(para.text_sel == null);

    // double click a word (after the multi-click window, a fresh count)
    runtime.last_click_ms -= 2000;
    const layout = try para.data.rich_text.ensureLayout(para.bounds.width);
    _ = layout;
    _ = runtime.dispatch(.{ .pointer_down = .{ .x = b.x + 60, .y = b.y + b.height / 2 } });
    _ = runtime.dispatch(.{ .pointer_up = .{ .x = b.x + 60, .y = b.y + b.height / 2 } });
    _ = runtime.dispatch(.{ .pointer_down = .{ .x = b.x + 60, .y = b.y + b.height / 2 } });
    _ = runtime.dispatch(.{ .pointer_up = .{ .x = b.x + 60, .y = b.y + b.height / 2 } });
    const word = try runtime.selectedText(std.testing.allocator);
    defer std.testing.allocator.free(word);
    try std.testing.expectEqualStrings("world", word);

    // Escape clears; ⌘A / Ctrl+A selects everything in the document again
    _ = runtime.dispatch(.{ .key_down = .{ .key = .escape } });
    const cleared = try runtime.selectedText(std.testing.allocator);
    defer std.testing.allocator.free(cleared);
    try std.testing.expectEqualStrings("", cleared);
}

const Widgets = struct {
    calls: usize = 0,
    fn render(self: *Widgets, info: md.BlockInfo) ?ui.Node {
        self.calls += 1;
        if (std.mem.eql(u8, info.code, "skip")) return null;
        return ui.text(std.fmt.allocPrint(info.arena, "widget:{s}:{s}", .{ info.meta, info.code }) catch "", .{ .key = "widget-text" });
    }
    fn call(ctx: *anyopaque, info: md.BlockInfo) ?ui.Node {
        return render(@ptrCast(@alignCast(ctx)), info);
    }
};

const Loader = struct {
    img: ?zant.image.Image = null,
    fn get(ctx: *anyopaque, url: []const u8) ?zant.image.Image {
        const self: *Loader = @ptrCast(@alignCast(ctx));
        return if (std.mem.eql(u8, url, "https://example.com/a.png")) self.img else null;
    }
};

test "markdown: custom block renderers, image loader and data URIs" {
    ui.beginFrame();
    defer ui.endFrame();
    var widgets = Widgets{};
    var loader = Loader{};
    var pixels: [8 * 4 * 4]u8 = undefined;
    @memset(&pixels, 0x80);
    loader.img = zant.image.Image.fromPixels(&pixels, 8, 4, 32);
    defer loader.img.?.deinit();
    const renderers = [_]md.BlockRenderer{.{ .lang = "widget", .ctx = &widgets, .func = Widgets.call }};
    var doc = try md.parse(std.testing.allocator,
        \\```widget {"id":3}
        \\chart
        \\```
        \\
        \\```widget
        \\skip
        \\```
        \\
        \\![remote](https://example.com/a.png)
        \\
        \\![pending](https://example.com/b.png)
        \\
        \\![inline](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==)
    );
    defer doc.deinit();
    const node = md.renderDocument(&doc, .{
        .block_renderers = &renderers,
        .image_loader = .{ .ctx = &loader, .func = Loader.get },
    });
    try std.testing.expectEqual(@as(usize, 2), widgets.calls);
    // walk the spec: the widget text, two images (loaded + data URI), one placeholder
    var found_widget = false;
    var images: usize = 0;
    var placeholder = false;
    walk(node, &found_widget, &images, &placeholder);
    try std.testing.expect(found_widget);
    try std.testing.expectEqual(@as(usize, 2), images);
    try std.testing.expect(placeholder);
}

fn walk(node: ui.Node, widget: *bool, images: *usize, placeholder: *bool) void {
    const e = node.element;
    if (e.kind == .text and std.mem.eql(u8, e.content, "widget:{\"id\":3}:chart")) widget.* = true;
    if (e.kind == .image and e.image_id != 0) images.* += 1;
    if (e.kind == .text and std.mem.indexOf(u8, e.content, "b.png") != null) placeholder.* = true;
    for (e.children) |c| walk(c, widget, images, placeholder);
}

test "markdown: LaTeX \\( \\) and \\[ \\] delimiters (opt-in)" {
    const src = "Area is \\(\\pi r^2\\) here.\n\n\\[\n\\int_0^1 x\\,dx\n\\]\n\nInline \\[a+b\\] too. Escaped \\[x\\] stays with the option off.";
    var on = try md.parseWith(std.testing.allocator, src, .{ .latex_delimiters = true });
    defer on.deinit();
    try std.testing.expect(on.blocks[0] == .paragraph);
    var math_spans: usize = 0;
    for (on.blocks[0].paragraph) |s| if (s.style.math) {
        math_spans += 1;
        try std.testing.expectEqualStrings("\\pi r^2", s.text);
    };
    try std.testing.expectEqual(@as(usize, 1), math_spans);
    try std.testing.expect(on.blocks[1] == .math);
    try std.testing.expectEqualStrings("\\int_0^1 x\\,dx", on.blocks[1].math.source);
    var off = try md.parseWith(std.testing.allocator, src, .{});
    defer off.deinit();
    for (off.blocks) |b| try std.testing.expect(b != .math);
}

test "selection: a modal over selectable text blocks it; text inside a panel still selects" {
    const App = struct {
        modal: bool = true,
        fn renderOpaque(ctx: *anyopaque) ui.Node {
            const self: *@This() = @ptrCast(@alignCast(ctx));
            const base = ui.column(.{ .key = "doc", .selectable = true, .padding = ui.inset(20), .children = &.{ui.paragraph("Background text that must not be selected through the modal", .{})} });
            if (!self.modal) return ui.column(.{ .grow = 1, .children = &.{base} });
            return ui.column(.{ .grow = 1, .children = &.{
                base,
                ui.styled(ui.column(.{ .key = "panel", .block_pointer = true, .align_items = .stretch, .children = &.{
                    ui.column(.{ .key = "inner", .selectable = true, .padding = ui.inset(10), .align_items = .stretch, .children = &.{ui.paragraph("Panel text", .{})} }),
                } }), .{ .absolute = true, .left = 0, .top = 0, .width = 400, .height = 300, .background = 0xffffffff }),
            } });
        }
    };
    var app = App{};
    const runtime = try Runtime.init(std.testing.allocator, &app, App.renderOpaque, .{ .window = .{ .width = 400, .height = 300 } });
    defer runtime.deinit();
    var renderer = try zant.renderer.Renderer.createRaster(400, 300);
    defer renderer.deinit();
    frame2(runtime, &renderer);
    // blank part of the panel, over the background text: nothing selected
    _ = runtime.dispatch(.{ .pointer_down = .{ .x = 30, .y = 200 } });
    _ = runtime.dispatch(.{ .pointer_move = .{ .x = 380, .y = 200 } });
    _ = runtime.dispatch(.{ .pointer_up = .{ .x = 380, .y = 200 } });
    try std.testing.expect(runtime.sel_root == null);
    // the panel's own text selects
    _ = runtime.dispatch(.{ .pointer_down = .{ .x = 12, .y = 22 } });
    _ = runtime.dispatch(.{ .pointer_move = .{ .x = 300, .y = 22 } });
    _ = runtime.dispatch(.{ .pointer_up = .{ .x = 300, .y = 22 } });
    const got = try runtime.selectedText(std.testing.allocator);
    defer std.testing.allocator.free(got);
    try std.testing.expectEqualStrings("Panel text", got);
}

fn frame2(runtime: *Runtime, renderer: *zant.renderer.Renderer) void {
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, 400, 300, 1.0);
    renderer.end();
}

test "markdown: a multi-line \\[ \\] formula with `=` / `+` lines stays one block" {
    var cache = md.Cache.init(std.testing.allocator);
    defer cache.deinit();
    const src = "套正弦和角公式：\n\n\\[\n\\sin75^\\circ\n=\n\\sin45^\\circ\\cos30^\\circ\n+\n\\cos45^\\circ\\sin30^\\circ\n\\]\n\n代入特殊角值：";
    const entries = try cache.update(src, .{ .mode = .streaming, .parse = .{ .latex_delimiters = true } });
    try std.testing.expectEqual(@as(usize, 3), entries.len);
    try std.testing.expect(entries[1].doc.blocks[0] == .math);
    try std.testing.expectEqualStrings("\\sin75^\\circ\n=\n\\sin45^\\circ\\cos30^\\circ\n+\n\\cos45^\\circ\\sin30^\\circ", entries[1].doc.blocks[0].math.source);
    // while streaming, an unclosed formula is one growing block too
    const partial = try cache.update(src[0 .. std.mem.indexOf(u8, src, "+").? + 2], .{ .mode = .streaming, .parse = .{ .latex_delimiters = true } });
    try std.testing.expectEqual(@as(usize, 2), partial.len);
    try std.testing.expect(partial[1].doc.blocks[0] == .math);
}

test "markdown: a streaming formula hides its half-arrived closing delimiter" {
    const cases = [_][2][]const u8{
        .{ "\\[\nf(x) = a + c\\", "f(x) = a + c" }, // `\]` cut after the backslash
        .{ "\\[\n\\begin{aligned} a &= b \\\\\n&= c \\\\", "\\begin{aligned} a &= b \\\\\n&= c \\\\" }, // `\\` is a line break
        .{ "$$\nx^2\n$", "x^2" }, // `$$` cut after one `$`
    };
    for (cases) |case| {
        var doc = try md.parseWith(std.testing.allocator, case[0], .{ .latex_delimiters = true });
        defer doc.deinit();
        try std.testing.expect(doc.blocks[0] == .math);
        try std.testing.expect(!doc.blocks[0].math.closed);
        try std.testing.expectEqualStrings(case[1], doc.blocks[0].math.source);
    }
    // through the streaming pipeline (remend + block cache) as an app sees it
    var cache = md.Cache.init(std.testing.allocator);
    defer cache.deinit();
    const entries = try cache.update("代回：\n\n\\[\nf(x) = a + c\\", .{ .mode = .streaming, .parse = .{ .latex_delimiters = true } });
    try std.testing.expect(entries[entries.len - 1].doc.blocks[0] == .math);
    try std.testing.expectEqualStrings("f(x) = a + c", entries[entries.len - 1].doc.blocks[0].math.source);
}
