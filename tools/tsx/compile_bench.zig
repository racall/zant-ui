//! The Zig side of the TSX compile benchmark (WP-5.10). A standalone tool,
//! not part of the `zant-tsx` compiler itself.
//!
//! The shipped Zig `zant-tsx-bench` binary (`examples/tsx/bench.zig`) measures
//! *frames*, not compile time, so it cannot be the baseline for the Rust
//! in-process compile benchmark (`crates/zant-examples/src/bin/zant-tsx-bench.rs`).
//! This harness calls the very same `tools/tsx/compile.zig` the Zig compiler
//! uses, in process, on the same corpus with the same warm-up / iteration
//! counts and prints the same table, so the two numbers are comparable.
//!
//! Build and run (also done by `scripts/tsx/examples-check.sh`):
//!
//!     ZIG_GLOBAL_CACHE_DIR=$PWD/.zig-global-cache zig build-exe -OReleaseFast \
//!         -lc --name tsx-compile-bench-zig tools/tsx/compile_bench.zig \
//!         -femit-bin=target/tsx-bench/zig-bench
//!     ./target/tsx-bench/zig-bench tools/tsx/testdata 10 100
//!
//! Usage: `tsx-compile-bench-zig <corpus dir> [warmup] [iterations]`
const std = @import("std");
const compile = @import("compile.zig");

const Stats = struct {
    samples: std.ArrayList(f64) = .empty,

    fn add(self: *Stats, arena: std.mem.Allocator, ms: f64) void {
        self.samples.append(arena, ms) catch {};
    }

    /// The `bench_harness.zig` report line (`examples/tsx/bench_harness.zig`).
    fn report(self: *Stats, name: []const u8) void {
        const s = self.samples.items;
        if (s.len == 0) return;
        std.mem.sort(f64, s, {}, std.sort.asc(f64));
        var sum: f64 = 0;
        for (s) |v| sum += v;
        const p95 = s[@min(s.len - 1, (s.len * 95) / 100)];
        const avg = sum / @as(f64, @floatFromInt(s.len));
        std.debug.print(
            "{s: <20} frames={d: >5}  avg={d: >6.3} ms  p95={d: >6.3} ms  max={d: >6.3} ms  ({d:.0} fps at avg)\n",
            .{ name, s.len, avg, p95, s[s.len - 1], 1000.0 / avg },
        );
    }
};

/// Monotonic microseconds (`zant.platform.monotonicUs` on macOS / Linux).
fn nowUs() i64 {
    var ts: std.c.timespec = undefined;
    _ = std.c.clock_gettime(.MONOTONIC, &ts);
    return @as(i64, @intCast(ts.sec)) * 1_000_000 + @divTrunc(@as(i64, @intCast(ts.nsec)), 1000);
}

pub fn main(init: std.process.Init) !void {
    const io = init.io;
    var arena_state = std.heap.ArenaAllocator.init(init.gpa);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const args = try init.minimal.args.toSlice(arena);
    const corpus = if (args.len >= 2) args[1] else "tools/tsx/testdata";
    const warmup = if (args.len >= 3) try std.fmt.parseInt(usize, args[2], 10) else 10;
    const iterations = if (args.len >= 4) try std.fmt.parseInt(usize, args[3], 10) else 100;

    var paths: std.ArrayList([]const u8) = .empty;
    var dir = try std.Io.Dir.cwd().openDir(io, corpus, .{ .iterate = true });
    defer dir.close(io);
    var it = dir.iterate();
    while (try it.next(io)) |entry| {
        if (entry.kind != .file or entry.name.len == 0 or entry.name[0] == '.') continue;
        if (!std.mem.endsWith(u8, entry.name, ".tsx")) continue;
        try paths.append(arena, try std.fs.path.join(arena, &.{ corpus, entry.name }));
    }
    std.mem.sort([]const u8, paths.items, {}, struct {
        fn lessThan(_: void, a: []const u8, b: []const u8) bool {
            return std.mem.lessThan(u8, a, b);
        }
    }.lessThan);

    std.debug.print(
        "zant-tsx-bench: {} corpus files in {s}, {} warm-up rounds, {} iterations\n",
        .{ paths.items.len, corpus, warmup, iterations },
    );

    var total: Stats = .{};
    var failed: usize = 0;
    // one arena per file, reset between iterations: same allocation behaviour
    // for every run, like the Rust benchmark's per-iteration allocations
    var file_arena = std.heap.ArenaAllocator.init(init.gpa);
    defer file_arena.deinit();
    for (paths.items) |path| {
        const src = try arena.dupe(u8, std.Io.Dir.cwd().readFileAlloc(io, path, arena, .limited(16 << 20)) catch |err| {
            std.debug.print("{s}: cannot read ({s})\n", .{ path, @errorName(err) });
            std.process.exit(1);
        });
        const file_name = std.fs.path.basename(path);
        const name = file_name[0 .. file_name.len - ".tsx".len];
        var stats: Stats = .{};
        var i: usize = 0;
        const runs = warmup + iterations;
        while (i < runs) : (i += 1) {
            const a = file_arena.allocator();
            const t0 = nowUs();
            const result = try compile.compile(a, src, name);
            const us = nowUs() - t0;
            _ = file_arena.reset(.retain_capacity);
            switch (result) {
                .ok => {},
                .err => |diag| {
                    std.debug.print("{s}: compile error at {}: {s}\n", .{ path, diag.loc, diag.msg });
                    failed += 1;
                    break;
                },
            }
            if (i >= warmup) {
                const ms = @as(f64, @floatFromInt(us)) / 1000.0;
                stats.add(arena, ms);
                total.add(arena, ms);
            }
        }
        if (stats.samples.items.len > 0) stats.report(name);
    }
    total.report("total");
    if (failed > 0) {
        std.debug.print("{} corpus files failed to compile\n", .{failed});
        std.process.exit(1);
    }
}
