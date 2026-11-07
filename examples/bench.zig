//! Headless performance benchmark (no window): drives the runtime with a
//! raster canvas and measures whole frames (rebuild + layout + paint) for
//! typical heavy scenarios. Deterministic, so runs before / after a change
//! (and on macOS / Windows) are comparable.
//!
//!   zig build -Doptimize=ReleaseFast --prefix zig-out-release
//!   ./zig-out-release/bin/zant-bench            # all scenarios
//!   ./zig-out-release/bin/zant-bench open scroll   # some of them
//!
//! Scenarios: open (first frame of a long chat), stream (reply streaming
//! into it), typing (an input above a long document), hover (pointer
//! moving over a long list of rows), resize (window width changing),
//! scroll (wheel scrolling the chat), caret (idle frames of a blinking
//! caret). ZANT_BENCH_MESSAGES sets the chat length (default 30);
//! ZANT_BENCH_LIST=1 renders the chat with a virtual list where the build
//! supports one (older builds ignore it, so the same command compares them).
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const md = zant.markdown;
const Runtime = zant.ui_runtime.Runtime;

var fake_now: i64 = 1_000_000;
var bench_list = false;
fn fakeClock() i64 {
    return fake_now;
}

const message_template =
    \\## 第 {d} 条：二次函数的最值
    \\
    \\设 $f(x) = ax^2 + bx + c$（$a \neq 0$），配方得 $f(x) = a\left(x + \frac{{b}}{{2a}}\right)^2 + \frac{{4ac-b^2}}{{4a}}$。当 $a > 0$ 时在顶点取得**最小值**，当 $a < 0$ 时取得**最大值**；这一段文字比较长，用来测试长段落在不同宽度下的换行与排版性能，包含中文、English words 和 `inline code`。
    \\
    \\$$
    \\h(t) = \begin{{cases}} 1, & t < 0 \\ 1 - t^2, & 0 \le t \le 1 \\ 2 - 2t, & t > 1 \end{{cases}}
    \\$$
    \\
    \\1. 对称轴 $x = -\frac{{b}}{{2a}}$；
    \\2. 比较端点 $f(m)$、$f(n)$；
    \\3. 写出结论。
    \\
    \\| $x$ | 0 | 1 | 2 | 3 |
    \\| :-: | :-: | :-: | :-: | :-: |
    \\| $f(x)$ | 3 | -3 | -5 | -3 |
    \\
    \\```python
    \\def f(x):
    \\    return 2 * x**2 - 8 * x + 3
    \\
    \\for x in range(0, 6):
    \\    print(x, f(x))  # value {d}
    \\```
    \\
;

const Chat = struct {
    gpa: std.mem.Allocator,
    texts: [][]u8,
    caches: []md.Cache,
    reply: md.Cache,
    reply_text: std.ArrayListUnmanaged(u8) = .empty,
    input_value: std.ArrayListUnmanaged(u8) = .empty,
    with_input: bool = false,
    use_list: bool = false,
    list: ui.ListState,

    fn init(gpa: std.mem.Allocator, n: usize) !*Chat {
        const self = try gpa.create(Chat);
        self.* = .{ .gpa = gpa, .texts = try gpa.alloc([]u8, n), .caches = try gpa.alloc(md.Cache, n), .reply = md.Cache.init(gpa), .list = .init(gpa), .use_list = bench_list };
        for (self.texts, self.caches, 0..) |*t, *c, i| {
            // ~5 KB per message, like real assistant replies
            t.* = try std.fmt.allocPrint(gpa, message_template ++ message_template ++ message_template ++ message_template, .{ i, i, i + 1, i + 1, i + 2, i + 2, i + 3, i + 3 });
            c.* = md.Cache.init(gpa);
        }
        return self;
    }

    fn onInput(self: *Chat, value: []const u8) void {
        self.input_value.clearRetainingCapacity();
        self.input_value.appendSlice(self.gpa, value) catch {};
    }

    fn message(self: *Chat, i: usize) ui.Node {
        if (i == self.texts.len) return ui.column(.{ .padding = ui.insetXY(16, 12), .align_items = .stretch, .children = &.{
            md.streamdown(&self.reply, self.reply_text.items, .{ .mode = .streaming, .render = .{ .key_prefix = "reply", .content_width = 680, .font_size = 15, .is_animating = true, .caret = .block } }),
        } });
        const key = std.fmt.allocPrint(ui.frameAllocator(), "m{d}", .{i}) catch "m";
        return ui.column(.{ .padding = ui.insetXY(16, 12), .radius = 12, .background = 0xffffffff, .align_items = .stretch, .children = &.{
            md.streamdown(&self.caches[i], self.texts[i], .{ .render = .{ .key_prefix = key, .content_width = 680, .font_size = 15 } }),
        } });
    }

    fn render(ctx: *anyopaque) ui.Node {
        const self: *Chat = @ptrCast(@alignCast(ctx));
        const arena = ui.frameAllocator();
        if (self.use_list) {
            var top: std.ArrayListUnmanaged(ui.Node) = .empty;
            if (self.with_input) top.append(arena, ui.input(.{ .key = "input", .value = self.input_value.items, .placeholder = "Type…", .on_change = ui.textAction(self, onInput) })) catch {};
            top.append(arena, ui.list(&self.list, .{
                .key = "chat",
                .count = self.texts.len + @as(usize, if (self.reply_text.items.len > 0) 1 else 0),
                .item = ui.listItem(self, message),
                .alignment = .bottom,
                .follow_tail = true,
                .gap = 16,
                .padding = ui.inset(20),
            })) catch {};
            return ui.column(.{ .grow = 1, .align_items = .stretch, .background = 0xfff5f5f5, .children = top.items });
        }
        var rows: std.ArrayListUnmanaged(ui.Node) = .empty;
        for (self.texts, self.caches, 0..) |t, *c, i| {
            const key = std.fmt.allocPrint(arena, "m{d}", .{i}) catch "m";
            rows.append(arena, ui.column(.{ .key = key, .padding = ui.insetXY(16, 12), .radius = 12, .background = 0xffffffff, .align_items = .stretch, .children = &.{
                md.streamdown(c, t, .{ .render = .{ .key_prefix = key, .content_width = 680, .font_size = 15 } }),
            } })) catch {};
        }
        if (self.reply_text.items.len > 0) rows.append(arena, ui.column(.{ .key = "reply-box", .padding = ui.insetXY(16, 12), .align_items = .stretch, .children = &.{
            md.streamdown(&self.reply, self.reply_text.items, .{ .mode = .streaming, .render = .{ .key_prefix = "reply", .content_width = 680, .font_size = 15, .is_animating = true, .caret = .block } }),
        } })) catch {};
        var top: std.ArrayListUnmanaged(ui.Node) = .empty;
        if (self.with_input) top.append(arena, ui.input(.{ .key = "input", .value = self.input_value.items, .placeholder = "Type…", .on_change = ui.textAction(self, onInput) })) catch {};
        top.append(arena, ui.column(.{ .key = "chat", .grow = 1, .shrink = 1, .basis = 0, .scroll = true, .padding = ui.inset(20), .gap = 16, .align_items = .stretch, .children = rows.items })) catch {};
        return ui.column(.{ .grow = 1, .align_items = .stretch, .background = 0xfff5f5f5, .children = top.items });
    }
};

const Rows = struct {
    fn render(ctx: *anyopaque) ui.Node {
        _ = ctx;
        const arena = ui.frameAllocator();
        const rows = arena.alloc(ui.Node, 400) catch return ui.column(.{});
        for (rows, 0..) |*r, i| {
            r.* = ui.row(.{ .key = std.fmt.allocPrint(arena, "r{d}", .{i}) catch "r", .gap = 8, .padding = ui.insetXY(12, 6), .align_items = .center, .children = &.{
                ui.text(std.fmt.allocPrint(arena, "Row {d}", .{i}) catch "", .{ .size = 13 }),
                ui.paragraph("A description that is long enough to wrap in narrow windows and exercise text layout.", .{ .size = 12 }),
                ui.button(.{ .text = "Open", .size = .small }),
            } });
        }
        return ui.column(.{ .grow = 1, .align_items = .stretch, .children = &.{ui.column(.{ .key = "list", .grow = 1, .basis = 0, .scroll = true, .align_items = .stretch, .children = rows })} });
    }
};

const Stats = struct {
    samples: std.ArrayListUnmanaged(f64) = .empty,

    fn add(self: *Stats, gpa: std.mem.Allocator, ms: f64) void {
        self.samples.append(gpa, ms) catch {};
    }

    fn report(self: *Stats, name: []const u8) void {
        const s = self.samples.items;
        if (s.len == 0) return;
        std.mem.sort(f64, s, {}, std.sort.asc(f64));
        var sum: f64 = 0;
        for (s) |v| sum += v;
        const p95 = s[@min(s.len - 1, (s.len * 95) / 100)];
        std.debug.print("{s: <8} frames={d: >5}  avg={d: >7.2} ms  p95={d: >7.2} ms  max={d: >7.2} ms\n", .{ name, s.len, sum / @as(f64, @floatFromInt(s.len)), p95, s[s.len - 1] });
    }
};

const W: f32 = 900;
const H: f32 = 900;

fn frame(rt: *Runtime, r: *zant.renderer.Renderer, w: f32, h: f32) f64 {
    fake_now += 16;
    const t0 = zant.platform.monotonicUs();
    const canvas = r.begin(1.0).?;
    rt.renderFrame(canvas, w, h, 1.0);
    r.end();
    return @as(f64, @floatFromInt(zant.platform.monotonicUs() - t0)) / 1000.0;
}

fn newRuntime(gpa: std.mem.Allocator, ctx: *anyopaque, render: *const fn (*anyopaque) ui.Node) !*Runtime {
    const rt = try Runtime.init(gpa, ctx, render, .{ .window = .{ .width = W, .height = H } });
    rt.clock = fakeClock;
    return rt;
}

fn chatKeyBounds(rt: *Runtime, key: []const u8) ?zant.geometry.Rect {
    return rt.tree.boundsOfKey(key);
}

pub fn main(init: std.process.Init) !void {
    const gpa = std.heap.c_allocator;
    const args = try init.minimal.args.toSlice(init.arena.allocator());
    const n = if (init.environ_map.get("ZANT_BENCH_MESSAGES")) |v| std.fmt.parseInt(usize, v, 10) catch 30 else 30;
    zant.motion.enabled = false;
    bench_list = init.environ_map.get("ZANT_BENCH_LIST") != null;
    var renderer = try zant.renderer.Renderer.createRaster(1400, 1000);
    defer renderer.deinit();
    const want = struct {
        fn f(list: []const [:0]const u8, name: []const u8) bool {
            if (list.len <= 1) return true;
            for (list[1..]) |a| if (std.mem.eql(u8, a, name)) return true;
            return false;
        }
    }.f;
    std.debug.print("zant-bench: {d} messages, raster canvas {d}x{d}{s}\n", .{ n, W, H, if (bench_list) ", virtual list" else "" });

    if (want(args, "open")) {
        var st: Stats = .{};
        for (0..5) |_| {
            const chat = try Chat.init(gpa, n);
            const rt = try newRuntime(gpa, chat, Chat.render);
            st.add(gpa, frame(rt, &renderer, W, H));
            rt.deinit();
        }
        st.report("open");
    }
    if (want(args, "stream")) {
        var st: Stats = .{};
        const chat = try Chat.init(gpa, n);
        const rt = try newRuntime(gpa, chat, Chat.render);
        _ = frame(rt, &renderer, W, H);
        const reply = try std.fmt.allocPrint(gpa, message_template ++ message_template, .{ 99, 99, 98, 98 });
        var i: usize = 0;
        while (i < reply.len) {
            var end = @min(reply.len, i + 8);
            while (end < reply.len and (reply[end] & 0xC0) == 0x80) end += 1;
            chat.reply_text.appendSlice(gpa, reply[i..end]) catch {};
            i = end;
            rt.invalidate();
            ui.scrollToEnd("chat");
            st.add(gpa, frame(rt, &renderer, W, H));
        }
        rt.deinit();
        st.report("stream");
    }
    if (want(args, "typing")) {
        var st: Stats = .{};
        const chat = try Chat.init(gpa, n);
        chat.with_input = true;
        const rt = try newRuntime(gpa, chat, Chat.render);
        _ = frame(rt, &renderer, W, H);
        if (chatKeyBounds(rt, "input")) |b| {
            _ = rt.dispatch(.{ .pointer_down = .{ .x = b.x + 10, .y = b.y + b.height / 2 } });
            _ = rt.dispatch(.{ .pointer_up = .{ .x = b.x + 10, .y = b.y + b.height / 2 } });
        }
        _ = frame(rt, &renderer, W, H);
        for (0..200) |_| {
            _ = rt.dispatch(.{ .text_input = "a" });
            st.add(gpa, frame(rt, &renderer, W, H));
        }
        rt.deinit();
        st.report("typing");
    }
    if (want(args, "caret")) {
        var st: Stats = .{};
        const chat = try Chat.init(gpa, n);
        chat.with_input = true;
        const rt = try newRuntime(gpa, chat, Chat.render);
        _ = frame(rt, &renderer, W, H);
        if (chatKeyBounds(rt, "input")) |b| _ = rt.dispatch(.{ .pointer_down = .{ .x = b.x + 10, .y = b.y + b.height / 2 } });
        _ = frame(rt, &renderer, W, H);
        for (0..100) |_| {
            fake_now += 530; // one blink
            st.add(gpa, frame(rt, &renderer, W, H));
        }
        rt.deinit();
        st.report("caret");
    }
    if (want(args, "hover")) {
        var st: Stats = .{};
        var dummy: u8 = 0;
        const rt = try newRuntime(gpa, &dummy, Rows.render);
        _ = frame(rt, &renderer, W, H);
        var y: f32 = 10;
        for (0..300) |k| {
            const x: f32 = if (k % 2 == 0) 860 else 300;
            _ = rt.dispatch(.{ .pointer_move = .{ .x = x, .y = y } });
            y += 2.5;
            st.add(gpa, frame(rt, &renderer, W, H));
        }
        rt.deinit();
        st.report("hover");
    }
    if (want(args, "resize")) {
        var st: Stats = .{};
        const chat = try Chat.init(gpa, n);
        const rt = try newRuntime(gpa, chat, Chat.render);
        _ = frame(rt, &renderer, W, H);
        for (0..120) |k| {
            const w = 700 + @as(f32, @floatFromInt(k % 60)) * 8;
            st.add(gpa, frame(rt, &renderer, w, H));
        }
        rt.deinit();
        st.report("resize");
    }
    if (want(args, "scroll")) {
        var st: Stats = .{};
        const chat = try Chat.init(gpa, n);
        const rt = try newRuntime(gpa, chat, Chat.render);
        _ = frame(rt, &renderer, W, H);
        for (0..300) |_| {
            _ = rt.dispatch(.{ .scroll = .{ .x = 400, .y = 400, .dx = 0, .dy = -60, .modifiers = .{} } });
            st.add(gpa, frame(rt, &renderer, W, H));
        }
        rt.deinit();
        st.report("scroll");
    }
}
