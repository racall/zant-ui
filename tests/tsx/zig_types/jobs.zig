//! A native module with a background thread (tests/tsx_wake_test.zig):
//! `start` runs work on a thread that posts a wake; `take` returns the result
//! on the UI thread.
const std = @import("std");
const zant = @import("zant");

var done = std.atomic.Value(u32).init(0);
var result = std.atomic.Value(u32).init(0);

pub fn start(n: u32) void {
    const thread = std.Thread.spawn(.{}, work, .{n}) catch return;
    thread.detach();
}

fn work(n: u32) void {
    result.store(n * 2, .release);
    done.store(1, .release);
    zant.ui.postWake();
}

/// The result once the thread finished, else 0.
pub fn take() u32 {
    if (done.swap(0, .acq_rel) == 0) return 0;
    return result.load(.acquire);
}
