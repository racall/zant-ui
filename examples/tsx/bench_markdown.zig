//! The same streamed text and full raster frames through TSX and a hand-held
//! Streamdown cache. Source assignment is outside the timed render in both.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const md = zant.markdown;
const harness = @import("bench_harness.zig");
const Tsx = @import("markdown_bench_tsx").MarkdownBench;

const Manual = struct {
    source: zant.tsx.String = .{},
    cache: md.Cache = md.Cache.init(std.heap.c_allocator),

    pub fn render(self: *@This()) ui.Node {
        return md.streamdown(&self.cache, self.source.get(), .{
            .mode = .streaming,
            .render = .{
                .key_prefix = comptime std.fmt.comptimePrint("md-{x}", .{zant.tsx.siteId(0, "Markdown", 1)}),
                .font_size = 15,
                .content_width = 600,
                .is_animating = true,
                .caret = .block,
            },
        });
    }

    fn deinit(self: *@This()) void {
        self.cache.deinit();
        self.source.deinit();
    }
};

/// Streams a multi-block reply, alternating order to reduce warmup bias.
pub fn run(gpa: std.mem.Allocator, renderer: *zant.renderer.Renderer) !void {
    var input: std.ArrayList(u8) = .empty;
    defer input.deinit(gpa);
    const paragraph = "## A streamed reply\n\nStable **paragraphs** stay cached while the tail grows. " ++
        "Inline `code` and [links](https://zant.dev) keep their nodes.\n\n" ++
        "```zig\nconst answer = 42;\n```\n\n";
    for (0..48) |_| try input.appendSlice(gpa, paragraph);
    var tsx_stats: harness.Stats = .{};
    defer tsx_stats.samples.deinit(gpa);
    var manual_stats: harness.Stats = .{};
    defer manual_stats.samples.deinit(gpa);
    for (0..4) |round| {
        if (round % 2 == 0) {
            try generated(gpa, renderer, input.items, &tsx_stats);
            try handwritten(gpa, renderer, input.items, &manual_stats);
        } else {
            try handwritten(gpa, renderer, input.items, &manual_stats);
            try generated(gpa, renderer, input.items, &tsx_stats);
        }
    }
    const tsx_avg = average(tsx_stats.samples.items);
    const manual_avg = average(manual_stats.samples.items);
    tsx_stats.report("markdown-tsx");
    manual_stats.report("markdown-manual");
    std.debug.print("markdown delta: {d:.2}% ({d} bytes, 24 bytes/frame, same Streamdown options)\n", .{
        (tsx_avg / manual_avg - 1) * 100, input.items.len,
    });
}

fn generated(gpa: std.mem.Allocator, renderer: *zant.renderer.Renderer, input: []const u8, st: *harness.Stats) !void {
    var app = Tsx{};
    defer app.source.deinit();
    try stream(gpa, renderer, &app, input, st);
}

fn handwritten(gpa: std.mem.Allocator, renderer: *zant.renderer.Renderer, input: []const u8, st: *harness.Stats) !void {
    var app = Manual{};
    defer app.deinit();
    try stream(gpa, renderer, &app, input, st);
}

fn stream(
    gpa: std.mem.Allocator,
    renderer: *zant.renderer.Renderer,
    app: anytype,
    input: []const u8,
    st: *harness.Stats,
) !void {
    var h = try harness.Harness.init(gpa, renderer, app);
    defer h.deinit();
    var end: usize = 24;
    while (end < input.len + 24) : (end += 24) {
        app.source.set(input[0..@min(end, input.len)]);
        h.rt.needs_build = true;
        st.add(gpa, h.frame());
    }
}

fn average(samples: []const f64) f64 {
    var sum: f64 = 0;
    for (samples) |value| sum += value;
    return sum / @as(f64, @floatFromInt(samples.len));
}
