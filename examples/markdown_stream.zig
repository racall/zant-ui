//! Streaming Markdown benchmark: a chat with finished history messages and
//! one assistant reply that streams in, like an AI chat over SSE.
//!
//!   zig build run-md-stream                         # built-in long reply, simulated deltas
//!   zant-md-stream reply.md                         # stream a Markdown file
//!   curl -N ... /chat | zant-md-stream --sse        # real SSE from stdin (text_delta events)
//!   zant-md-stream --sse-replay recorded.sse        # recorded SSE, one event per ZANT_STREAM_MS
//!
//! Environment:
//!   ZANT_STREAM_CHARS  bytes per simulated delta (default 6)
//!   ZANT_STREAM_MS     ms between simulated deltas (default 16)
//!   ZANT_STREAM_HISTORY finished messages above the reply (default 8)
//!   ZANT_STREAM_EXIT=1 quit when the stream ends (prints a summary)
//!   ZANT_STREAM_WIDTH  window width (default 900)
//!   ZANT_FRAME_STATS=1 per-second frame timing (use a ReleaseFast build)
//!
//! The stream runs on a worker thread (like a network reader); it appends
//! to a shared buffer and calls `ui.postWake()`; each build renders the
//! whole buffer with `markdown.streamdown` (block cache + remend).
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const md = zant.markdown;

const Shared = struct {
    mutex: zant.sync.Mutex = .{},
    text: std.ArrayListUnmanaged(u8) = .empty,
    done: bool = false,
    deltas: usize = 0,
};

const State = struct {
    gpa: std.mem.Allocator,
    shared: *Shared,
    /// Copy of the shared text the UI renders (taken under the lock).
    view: std.ArrayListUnmanaged(u8) = .empty,
    reply: md.Cache,
    history: []md.Cache,
    history_text: []const []const u8,
    dark: bool = false,
    exit_when_done: bool = false,
    started_ms: i64 = 0,
    finished_ms: i64 = 0,
    builds: usize = 0,
    build_us_total: i64 = 0,
    build_us_max: i64 = 0,
    streaming: bool = true,
    exited: bool = false,
    done_now: bool = false,
    list: ui.ListState,

    pub fn theme(self: *State) zant.tokens.Theme {
        return if (self.dark) zant.tokens.antDark() else zant.tokens.antLight();
    }

    fn render(self: *State) ui.Node {
        const t0 = zant.platform.monotonicUs();
        defer {
            const us = zant.platform.monotonicUs() - t0;
            self.builds += 1;
            self.build_us_total += us;
            self.build_us_max = @max(self.build_us_max, us);
        }
        if (self.started_ms == 0) self.started_ms = ui.nowMs();
        var done = false;
        var deltas: usize = 0;
        {
            self.shared.mutex.lock();
            defer self.shared.mutex.unlock();
            if (self.shared.text.items.len != self.view.items.len) {
                self.view.clearRetainingCapacity();
                self.view.appendSlice(self.gpa, self.shared.text.items) catch {};
            }
            done = self.shared.done;
            deltas = self.shared.deltas;
        }
        if (done and self.streaming) {
            self.streaming = false;
            self.finished_ms = ui.nowMs();
            self.summary(deltas);
        }
        if (done and self.exit_when_done and !self.exited) {
            self.exited = true;
            std.process.exit(0);
        }
        const t = ui.currentTheme();
        self.done_now = done;
        return ui.column(.{
            .grow = 1,
            .align_items = .stretch,
            .background = t.color_bg_layout,
            .children = &.{
                ui.row(.{ .padding = ui.insetXY(20, 10), .gap = 12, .align_items = .center, .children = &.{
                    ui.text("Streaming Markdown", .{ .size = 14, .weight = .semibold }),
                    ui.textFmt("{d} bytes · {d} deltas{s}", .{ self.view.items.len, deltas, if (done) " · done" else "" }, .{ .size = 12, .color = t.color_text_secondary }),
                } }),
                // a virtual list: only the messages in view are built and laid
                // out; it follows the reply while it streams
                ui.list(&self.list, .{
                    .key = "chat",
                    .count = self.history.len + 1,
                    .item = ui.listItem(self, message),
                    .alignment = .bottom,
                    .follow_tail = true,
                    .gap = 16,
                    .padding = ui.insetXY(24, 16),
                    .selectable = true,
                }),
            },
        });
    }

    fn message(self: *State, i: usize) ui.Node {
        const t = ui.currentTheme();
        const md_theme = if (self.dark) md.theme.dark() else md.theme.light();
        if (i < self.history.len) {
            const key = std.fmt.allocPrint(ui.frameAllocator(), "h{d}", .{i}) catch "h";
            return bubble(key, i % 2 == 0, t, md.streamdown(&self.history[i], self.history_text[i], .{ .parse = .{ .latex_delimiters = true }, .render = .{ .theme = md_theme, .key_prefix = key, .content_width = 680, .font_size = 15, .selectable = false } }));
        }
        return bubble("reply", false, t, md.streamdown(&self.reply, self.view.items, .{
            .mode = .streaming,
            // Sia's replies (like most model output) use \( \) and \[ \]
            .parse = .{ .latex_delimiters = true },
            .render = .{ .theme = md_theme, .key_prefix = "reply", .content_width = 680, .font_size = 15, .is_animating = !self.done_now, .caret = .block, .selectable = false },
        }));
    }

    fn summary(self: *State, deltas: usize) void {
        const elapsed = self.finished_ms - self.started_ms;
        const avg = if (self.builds > 0) @as(f64, @floatFromInt(self.build_us_total)) / @as(f64, @floatFromInt(self.builds)) / 1000.0 else 0;
        std.debug.print("[stream] done: {d} bytes, {d} deltas, {d} ms, {d} builds, build avg {d:.2} ms, max {d:.2} ms\n", .{
            self.view.items.len, deltas, elapsed, self.builds, avg, @as(f64, @floatFromInt(self.build_us_max)) / 1000.0,
        });
    }
};

fn bubble(key: []const u8, user: bool, theme: zant.tokens.Theme, content: ui.Node) ui.Node {
    return ui.row(.{ .key = key, .width_percent = 100, .justify = if (user) .end else .start, .children = &.{
        ui.column(.{
            .max_width = 760,
            .grow = if (user) 0 else 1,
            .shrink = 1,
            .padding = ui.insetXY(16, 12),
            .radius = 12,
            .background = if (user) theme.color_primary_bg else theme.color_bg_container,
            .align_items = .stretch,
            .children = &.{content},
        }),
    } });
}

// ---- stream sources (worker thread) ---------------------------------------

const Source = union(enum) {
    /// Simulated deltas from a document.
    text: []const u8,
    /// Server-sent events on stdin: `data: {"type":"text_delta","content":"…"}`.
    sse,
    /// A recorded SSE stream, replayed one event per interval.
    sse_replay: []const u8,
};

fn appendDelta(shared: *Shared, bytes: []const u8) void {
    shared.mutex.lock();
    shared.text.appendSlice(std.heap.c_allocator, bytes) catch {};
    shared.deltas += 1;
    shared.mutex.unlock();
    ui.postWake();
}

fn finish(shared: *Shared) void {
    shared.mutex.lock();
    shared.done = true;
    shared.mutex.unlock();
    ui.postWake();
}

fn sleepMs(ms: u32) void {
    if (@import("builtin").os.tag == .windows) {
        const Sleep = struct {
            extern "kernel32" fn Sleep(ms: u32) callconv(.winapi) void;
        }.Sleep;
        Sleep(ms);
    } else {
        const ts = std.c.timespec{ .sec = @intCast(ms / 1000), .nsec = @intCast(@as(u64, ms % 1000) * 1_000_000) };
        _ = std.c.nanosleep(&ts, null);
    }
}

fn simulate(shared: *Shared, text: []const u8, chars: usize, ms: u32) void {
    // wait for the window
    sleepMs(400);
    var i: usize = 0;
    while (i < text.len) {
        var end = @min(text.len, i + chars);
        // never split a UTF-8 sequence (deltas are whole characters)
        while (end < text.len and (text[end] & 0xC0) == 0x80) end += 1;
        appendDelta(shared, text[i..end]);
        i = end;
        sleepMs(ms);
    }
    finish(shared);
}

fn readStdin(buf: []u8) isize {
    if (@import("builtin").os.tag == .windows) {
        const k = struct {
            extern "kernel32" fn GetStdHandle(id: u32) callconv(.winapi) ?*anyopaque;
            extern "kernel32" fn ReadFile(h: ?*anyopaque, buf: [*]u8, len: u32, read: *u32, overlapped: ?*anyopaque) callconv(.winapi) i32;
        };
        var got: u32 = 0;
        const std_input: u32 = @bitCast(@as(i32, -10));
        if (k.ReadFile(k.GetStdHandle(std_input), buf.ptr, @intCast(buf.len), &got, null) == 0) return -1;
        return @intCast(got);
    }
    return std.c.read(0, buf.ptr, buf.len);
}

/// Reads SSE from stdin and appends the `content` of `text_delta` events.
fn readSse(shared: *Shared) void {
    var line: std.ArrayListUnmanaged(u8) = .empty;
    defer line.deinit(std.heap.c_allocator);
    var buf: [4096]u8 = undefined;
    while (true) {
        const n = readStdin(&buf);
        if (n <= 0) break;
        for (buf[0..@intCast(n)]) |c| {
            if (c != '\n') {
                line.append(std.heap.c_allocator, c) catch {};
                continue;
            }
            handleSseLine(shared, std.mem.trim(u8, line.items, " \r"));
            line.clearRetainingCapacity();
        }
    }
    finish(shared);
}

fn replaySse(shared: *Shared, recorded: []const u8, ms: u32) void {
    sleepMs(400);
    var lines = std.mem.splitScalar(u8, recorded, '\n');
    while (lines.next()) |raw| {
        const line = std.mem.trim(u8, raw, " \r");
        if (!std.mem.startsWith(u8, line, "data:")) continue;
        handleSseLine(shared, line);
        sleepMs(ms);
    }
    finish(shared);
}

fn handleSseLine(shared: *Shared, line: []const u8) void {
    if (!std.mem.startsWith(u8, line, "data:")) return;
    const json = std.mem.trim(u8, line[5..], " ");
    const parsed = std.json.parseFromSlice(std.json.Value, std.heap.c_allocator, json, .{}) catch return;
    defer parsed.deinit();
    if (parsed.value != .object) return;
    const obj = parsed.value.object;
    const kind = obj.get("type") orelse return;
    if (kind != .string) return;
    if (std.mem.eql(u8, kind.string, "text_delta")) {
        const content = obj.get("content") orelse return;
        if (content == .string) appendDelta(shared, content.string);
    } else if (std.mem.eql(u8, kind.string, "turn_reset")) {
        shared.mutex.lock();
        shared.text.clearRetainingCapacity();
        shared.mutex.unlock();
        ui.postWake();
    }
}

fn readFile(gpa: std.mem.Allocator, path: []const u8) ![]u8 {
    const path_z = try gpa.dupeSentinel(u8, path, 0);
    defer gpa.free(path_z);
    const f = std.c.fopen(path_z.ptr, "rb") orelse return error.FileNotFound;
    defer _ = std.c.fclose(f);
    var out: std.ArrayListUnmanaged(u8) = .empty;
    var buf: [16 * 1024]u8 = undefined;
    while (true) {
        const n = std.c.fread(&buf, 1, buf.len, f);
        if (n == 0) break;
        try out.appendSlice(gpa, buf[0..n]);
    }
    return out.toOwnedSlice(gpa);
}

const builtin_reply =
    \\## 二次函数的最值问题
    \\
    \\设函数 $f(x) = ax^2 + bx + c$（其中 $a \neq 0$），我们来系统地分析它的**最大值与最小值**。首先回顾配方：
    \\
    \\$$
    \\f(x) = a\left(x + \frac{b}{2a}\right)^2 + \frac{4ac - b^2}{4a}
    \\$$
    \\
    \\由此可得顶点坐标为 $\left(-\frac{b}{2a}, \frac{4ac-b^2}{4a}\right)$。当 $a > 0$ 时抛物线开口向上，函数在顶点处取得**最小值**；当 $a < 0$ 时开口向下，取得**最大值**。
    \\
    \\### 例题
    \\
    \\> 已知 $f(x) = 2x^2 - 8x + 3$，求 $f(x)$ 在区间 $[0, 5]$ 上的最值。
    \\
    \\1. 对称轴 $x = -\frac{b}{2a} = 2$，在区间 $[0,5]$ 内；
    \\2. 最小值 $f(2) = 2 \cdot 4 - 16 + 3 = -5$；
    \\3. 比较端点：$f(0) = 3$，$f(5) = 50 - 40 + 3 = 13$，所以最大值是 $13$。
    \\
    \\| $x$ | 0 | 1 | 2 | 3 | 4 | 5 |
    \\| :-: | :-: | :-: | :-: | :-: | :-: | :-: |
    \\| $f(x)$ | 3 | -3 | -5 | -3 | 3 | 13 |
    \\
    \\下面用 Python 验证结果：
    \\
    \\```python
    \\import numpy as np
    \\
    \\def f(x):
    \\    return 2 * x**2 - 8 * x + 3
    \\
    \\xs = np.linspace(0, 5, 501)
    \\ys = f(xs)
    \\print("min:", ys.min(), "at", xs[ys.argmin()])
    \\print("max:", ys.max(), "at", xs[ys.argmax()])
    \\# 输出: min: -5.0 at 2.0, max: 13.0 at 5.0
    \\```
    \\
    \\### 一般结论
    \\
    \\对于闭区间 $[m, n]$ 上的二次函数，最值只可能出现在**顶点**或**端点**处。设对称轴为 $x_0 = -\frac{b}{2a}$，分三种情况讨论：
    \\
    \\- 若 $x_0 < m$：函数在区间上单调，最值在两个端点取得；
    \\- 若 $m \le x_0 \le n$：一个最值在顶点，另一个在离顶点较远的端点；
    \\- 若 $x_0 > n$：同样在端点取得。
    \\
    \\这个结论可以推广到**含参数的二次函数**：例如 $g(x) = x^2 - 2tx + 1$ 在 $[0, 1]$ 上的最小值 $h(t)$ 是一个分段函数：
    \\
    \\$$
    \\h(t) = \begin{cases} 1, & t < 0 \\ 1 - t^2, & 0 \le t \le 1 \\ 2 - 2t, & t > 1 \end{cases}
    \\$$
    \\
    \\掌握了“定轴动区间”与“动轴定区间”两类问题，就能解决绝大多数二次函数最值题。下一步可以练习几道综合题，巩固这种**分类讨论**的思路。
    \\
;

const history_samples = [_][]const u8{
    "帮我讲一下二次函数的最值问题，最好带例题。",
    "当然可以！二次函数 $y = ax^2+bx+c$ 的图像是抛物线，**开口方向**由 $a$ 决定，对称轴是 $x=-\\frac{b}{2a}$。\n\n- $a>0$：开口向上，有最小值\n- $a<0$：开口向下，有最大值\n\n我们先看一个例子：$y = x^2 - 4x + 3 = (x-2)^2 - 1$，最小值为 $-1$。",
    "那如果区间不包含顶点呢？",
    "这时函数在区间上是**单调**的，最值在端点取得。例如 $y=(x-2)^2-1$ 在 $[3,5]$ 上递增：\n\n| x | 3 | 4 | 5 |\n|---|---|---|---|\n| y | 0 | 3 | 8 |\n\n所以最小值 $0$，最大值 $8$。",
};

pub fn main(init: std.process.Init) !void {
    const gpa = init.gpa;
    const args = try init.minimal.args.toSlice(init.arena.allocator());
    const env = init.environ_map;
    const shared = try gpa.create(Shared);
    shared.* = .{};

    var source: Source = .{ .text = builtin_reply };
    var ai: usize = 1;
    while (ai < args.len) : (ai += 1) {
        const arg = args[ai];
        if (std.mem.eql(u8, arg, "--sse")) {
            source = .sse;
        } else if (std.mem.eql(u8, arg, "--sse-replay") and ai + 1 < args.len) {
            ai += 1;
            source = .{ .sse_replay = try readFile(gpa, args[ai]) };
        } else source = .{ .text = try readFile(gpa, arg) };
    }
    const chars = if (env.get("ZANT_STREAM_CHARS")) |v| std.fmt.parseInt(usize, v, 10) catch 6 else 6;
    const ms = if (env.get("ZANT_STREAM_MS")) |v| std.fmt.parseInt(u32, v, 10) catch 16 else 16;
    const history_n = if (env.get("ZANT_STREAM_HISTORY")) |v| std.fmt.parseInt(usize, v, 10) catch 8 else 8;

    const history = try gpa.alloc(md.Cache, history_n);
    const history_text = try gpa.alloc([]const u8, history_n);
    for (history, history_text, 0..) |*c, *t, i| {
        c.* = md.Cache.init(gpa);
        // long finished replies interleaved with short user messages
        t.* = if (i % 2 == 0) history_samples[(i / 2 * 2) % history_samples.len] else switch (source) {
            .text => |s| s,
            .sse, .sse_replay => builtin_reply,
        };
    }

    _ = switch (source) {
        .text => |text| try std.Thread.spawn(.{}, simulate, .{ shared, text, chars, ms }),
        .sse => try std.Thread.spawn(.{}, readSse, .{shared}),
        .sse_replay => |recorded| try std.Thread.spawn(.{}, replaySse, .{ shared, recorded, if (env.get("ZANT_STREAM_MS") != null) ms else 25 }),
    };
    try zant.launch.run(gpa, State{
        .gpa = gpa,
        .shared = shared,
        .reply = md.Cache.init(gpa),
        .history = history,
        .history_text = history_text,
        .list = .init(gpa),
        .dark = env.get("ZANT_DARK") != null,
        .exit_when_done = env.get("ZANT_STREAM_EXIT") != null,
    }, State.render, .{
        .title = "Zant streaming Markdown",
        .window = .{ .width = if (env.get("ZANT_STREAM_WIDTH")) |w| std.fmt.parseFloat(f32, w) catch 900 else 900, .height = 900, .min_width = 600, .min_height = 500 },
    });
}
