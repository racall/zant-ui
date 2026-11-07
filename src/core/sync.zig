//! Mutex and condition variable for worker threads (render workers,
//! background decoding), on the OS primitives: pthreads on macOS, SRW locks
//! and condition variables on Windows. Zero-initialised values are ready.
const std = @import("std");
const builtin = @import("builtin");

const is_windows = builtin.os.tag == .windows;

const win = struct {
    const SRWLOCK = extern struct { ptr: ?*anyopaque = null };
    const CONDITION_VARIABLE = extern struct { ptr: ?*anyopaque = null };
    extern "kernel32" fn AcquireSRWLockExclusive(lock: *SRWLOCK) callconv(.winapi) void;
    extern "kernel32" fn ReleaseSRWLockExclusive(lock: *SRWLOCK) callconv(.winapi) void;
    extern "kernel32" fn SleepConditionVariableSRW(cv: *CONDITION_VARIABLE, lock: *SRWLOCK, ms: u32, flags: u32) callconv(.winapi) i32;
    extern "kernel32" fn WakeConditionVariable(cv: *CONDITION_VARIABLE) callconv(.winapi) void;
    extern "kernel32" fn WakeAllConditionVariable(cv: *CONDITION_VARIABLE) callconv(.winapi) void;
};

pub const Mutex = struct {
    impl: if (is_windows) win.SRWLOCK else std.c.pthread_mutex_t = .{},

    pub fn lock(self: *Mutex) void {
        if (is_windows) win.AcquireSRWLockExclusive(&self.impl) else _ = std.c.pthread_mutex_lock(&self.impl);
    }

    pub fn unlock(self: *Mutex) void {
        if (is_windows) win.ReleaseSRWLockExclusive(&self.impl) else _ = std.c.pthread_mutex_unlock(&self.impl);
    }
};

pub const Condition = struct {
    impl: if (is_windows) win.CONDITION_VARIABLE else std.c.pthread_cond_t = .{},

    /// Atomically unlocks `mutex`, waits for a signal, and locks it again.
    /// Spurious wake-ups happen: wait in a loop on the condition.
    pub fn wait(self: *Condition, mutex: *Mutex) void {
        if (is_windows) {
            _ = win.SleepConditionVariableSRW(&self.impl, &mutex.impl, 0xFFFFFFFF, 0);
        } else _ = std.c.pthread_cond_wait(&self.impl, &mutex.impl);
    }

    pub fn signal(self: *Condition) void {
        if (is_windows) win.WakeConditionVariable(&self.impl) else _ = std.c.pthread_cond_signal(&self.impl);
    }

    pub fn broadcast(self: *Condition) void {
        if (is_windows) win.WakeAllConditionVariable(&self.impl) else _ = std.c.pthread_cond_broadcast(&self.impl);
    }
};
