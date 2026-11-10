//! `setTimeout` / `clearTimeout` for TSX (docs/tsx.md, "定时器"):
//!
//!     const id = setTimeout(() => expire(row), 8000);
//!     clearTimeout(id);
//!
//! A timer keeps its callback's context by value (a capture struct of
//! numbers / booleans / enums and the component pointer, or the component
//! pointer alone), so it outlives the frame that set it. A sleeper thread
//! posts a wake at the due time; the app shell (`zant.tsx.app.Shell.onWake`)
//! then runs the due timers on the UI thread and every window rebuilds.
const std = @import("std");
const ui = @import("ui.zig");
const platform = @import("platform/platform.zig");

const gpa = std.heap.c_allocator;

const Timer = struct {
    id: u64,
    due_ms: i64,
    run: *const fn (*Timer) void,
    free: *const fn (*Timer) void,
};

var timers: std.ArrayList(*Timer) = .empty;
var next_id: u64 = 1;

/// Runs `f(ctx)` (or `f(&ctx)` for a capture struct) on the UI thread after
/// `ms` milliseconds; returns the id for `clearTimeout`.
pub fn set(ms: f64, ctx: anytype, comptime f: anytype) f64 {
    const Ctx = @TypeOf(ctx);
    const Node = struct {
        base: Timer,
        ctx: Ctx,

        fn run(t: *Timer) void {
            const node: *@This() = @fieldParentPtr("base", t);
            if (@typeInfo(Ctx) == .pointer) f(node.ctx) else f(&node.ctx);
        }

        fn free(t: *Timer) void {
            const node: *@This() = @fieldParentPtr("base", t);
            gpa.destroy(node);
        }
    };
    const delay: i64 = if (ms > 0) @intFromFloat(@min(ms, 1e12)) else 0;
    const node = gpa.create(Node) catch return 0;
    const id = next_id;
    next_id += 1;
    node.* = .{ .base = .{
        .id = id,
        .due_ms = platform.monotonicMs() + delay,
        .run = Node.run,
        .free = Node.free,
    }, .ctx = ctx };
    timers.append(gpa, &node.base) catch {
        gpa.destroy(node);
        return 0;
    };
    wakeIn(delay);
    return @floatFromInt(id);
}

/// Cancels a timer that has not run yet (ignores unknown ids).
pub fn clear(id: f64) void {
    if (!(id >= 1)) return;
    const want: u64 = @intFromFloat(id);
    for (timers.items, 0..) |t, i| if (t.id == want) {
        _ = timers.orderedRemove(i);
        t.free(t);
        return;
    };
}

/// Runs the timers due by `now_ms` (in the order they were set); true when
/// one ran. Timers set by a callback wait for a later call.
pub fn fire(now_ms: i64) bool {
    var ran = false;
    var i: usize = 0;
    const count = timers.items.len;
    var seen: usize = 0;
    while (i < timers.items.len and seen < count) : (seen += 1) {
        const t = timers.items[i];
        if (t.due_ms > now_ms) {
            i += 1;
            continue;
        }
        _ = timers.orderedRemove(i);
        t.run(t);
        t.free(t);
        ran = true;
    }
    return ran;
}

/// The app shell's wake: runs what is due, then makes sure a wake comes for
/// the next timer (a sleep may end a little early, e.g. on Windows).
pub fn fireDue() bool {
    const now = platform.monotonicMs();
    // a wake a few ms early still runs the timer it was for
    const ran = fire(now + early_ms);
    // every timer has its own sleeper (`set`); a wake that came just before
    // a timer was due gets another one for it
    var next: ?i64 = null;
    for (timers.items) |t| next = if (next) |n| @min(n, t.due_ms) else t.due_ms;
    if (next) |due| if (due - now < 100) wakeIn(@max(due - now, 1));
    return ran;
}

/// How early a wake may come and still run its timer.
const early_ms = 4;

/// Timers still waiting (tests).
pub fn pending() usize {
    return timers.items.len;
}

/// A short-lived thread sleeps until the due time and wakes the UI thread
/// (`ui.postWake`); a cleared timer just causes one extra rebuild.
fn wakeIn(delay_ms: i64) void {
    // tests fire timers themselves; outside a TS app nothing would run them
    const io = @import("tsx_app.zig").ioIfStarted() orelse return;
    const Sleeper = struct {
        fn run(i: std.Io, ms: i64) void {
            i.sleep(.fromMilliseconds(@max(ms, 0) + 2), .awake) catch {};
            ui.postWake();
        }
    };
    const thread = std.Thread.spawn(.{}, Sleeper.run, .{ io, delay_ms }) catch return;
    thread.detach();
}

test "timers run when due, in order, and can be cleared" {
    const Probe = struct {
        var log: [8]u8 = undefined;
        var len: usize = 0;
        const Cap = struct { tag: u8 };
        fn hit(cap: *Cap) void {
            log[len] = cap.tag;
            len += 1;
        }
    };
    const a = set(0, Probe.Cap{ .tag = 'a' }, Probe.hit);
    const b = set(50, Probe.Cap{ .tag = 'b' }, Probe.hit);
    const c = set(0, Probe.Cap{ .tag = 'c' }, Probe.hit);
    try std.testing.expect(a > 0 and b > a and c > b);
    clear(c);
    try std.testing.expect(fire(platform.monotonicMs()));
    try std.testing.expectEqualStrings("a", Probe.log[0..Probe.len]);
    try std.testing.expectEqual(@as(usize, 1), pending());
    try std.testing.expect(!fire(platform.monotonicMs()));
    try std.testing.expect(fire(platform.monotonicMs() + 100));
    try std.testing.expectEqualStrings("ab", Probe.log[0..Probe.len]);
    clear(b); // already ran: ignored
    clear(-1);
    try std.testing.expectEqual(@as(usize, 0), pending());
}
