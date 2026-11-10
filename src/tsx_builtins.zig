//! JavaScript numeric built-ins. Returned strings belong to the current UI frame.
const std = @import("std");
const tsx = @import("tsx.zig");
const ui = @import("ui.zig");
const unicode = @import("core/unicode.zig");
const nan = std.math.nan(f64);
const inf = std.math.inf(f64);

/// Unary Math operations supported by the TSX compiler.
pub const Unary = enum { floor, ceil, round, trunc, abs, sign, sqrt, cbrt, exp, log, log2, log10, sin, cos, tan, atan };

/// Integer-preserving operations never round-trip through floating point.
pub fn unary(comptime op: Unary, x: anytype) if (@typeInfo(@TypeOf(x)) == .int) @TypeOf(x) else f64 {
    if (@typeInfo(@TypeOf(x)) == .int) return switch (op) {
        .floor, .ceil, .round, .trunc => x,
        .abs => if (x < 0) -x else x, // Signed minimum overflows, just like other native integer arithmetic.
        else => @compileError("this Math operation requires f64"),
    };
    return switch (op) {
        .floor => @floor(x),
        .ceil => @ceil(x),
        .trunc => @trunc(x),
        .abs => @abs(x),
        .round => if (!std.math.isFinite(x) or x == 0) x else blk: {
            const lo = @floor(x);
            const n = if (x - lo < 0.5) lo else lo + 1;
            break :blk if (n == 0) std.math.copysign(@as(f64, 0), x) else n;
        },
        .sign => if (std.math.isNan(x) or x == 0) x else std.math.copysign(@as(f64, 1), x),
        .sqrt => @sqrt(x),
        .cbrt => std.math.cbrt(x),
        .exp => @exp(x),
        .log => @log(x),
        .log2 => @log2(x),
        .log10 => @log10(x),
        .sin => @sin(x),
        .cos => @cos(x),
        .tan => @tan(x),
        .atan => std.math.atan(x),
    };
}

/// JS exponentiation differs from libm for NaN and unit bases with infinite exponents.
pub fn pow(x: f64, y: f64) f64 {
    if (y == 0) return 1;
    if (std.math.isNan(y) or (@abs(x) == 1 and std.math.isInf(y))) return nan;
    return std.math.pow(f64, x, y);
}

/// atan2 preserves quadrant and signed-zero semantics.
pub fn atan2(y: f64, x: f64) f64 {
    return std.math.atan2(y, x);
}

/// Variadic hypot avoids overflow/underflow and lets infinity take precedence over NaN.
pub fn hypot(values: []const f64) f64 {
    var result: f64 = 0;
    for (values) |x| result = std.math.hypot(result, x);
    return result;
}

/// Variadic extrema propagate NaN and choose the JS sign when zeros compare equal.
pub fn extrema(comptime T: type, comptime minimum: bool, values: []const T) T {
    var result: T = if (@typeInfo(T) == .int) values[0] else if (minimum) inf else -inf;
    for (values) |x| {
        if (@typeInfo(T) == .float) {
            if (std.math.isNan(x)) return x;
            if (x == 0 and result == 0) {
                if (std.math.signbit(x) == minimum) result = x;
                continue;
            }
        }
        if (if (minimum) x < result else x > result) result = x;
    }
    return result;
}

/// Number predicates do not coerce strings or booleans.
pub fn predicate(comptime kind: enum { isNaN, isFinite, isInteger }, x: anytype) bool {
    const info = @typeInfo(@TypeOf(x));
    if (info == .int) return kind != .isNaN;
    if (info != .float) return false;
    return switch (kind) {
        .isNaN => std.math.isNan(x),
        .isFinite => std.math.isFinite(x),
        .isInteger => std.math.isFinite(x) and @trunc(x) == x,
    };
}

fn whitespace(cp: u21) bool {
    return switch (cp) {
        0x09...0x0d, 0x20, 0xa0, 0x1680, 0x2000...0x200a, 0x2028, 0x2029, 0x202f, 0x205f, 0x3000, 0xfeff => true,
        else => false,
    };
}

fn trim(s: []const u8, end: bool) []const u8 {
    var lo: usize = 0;
    var hi = s.len;
    while (lo < hi) {
        const d = unicode.decodeAt(s, lo);
        if (!whitespace(d.codepoint)) break;
        lo += d.len;
    }
    if (end) while (hi > lo) {
        const prev = unicode.prevCodepoint(s, hi);
        if (!whitespace(unicode.decodeAt(s, prev).codepoint)) break;
        hi = prev;
    };
    return s[lo..hi];
}

fn digit(c: u8) u8 {
    if (std.ascii.isDigit(c)) return c - '0';
    return if (std.ascii.isAlphabetic(c)) std.ascii.toLower(c) - 'a' + 10 else 255;
}

fn integer(s: []const u8, radix: u8, full: bool) f64 {
    var n: u1024 = 0;
    var count: usize = 0;
    var overflow = false;
    for (s) |c| {
        const d = digit(c);
        if (d >= radix) break;
        const product = @mulWithOverflow(n, radix);
        const sum = @addWithOverflow(product[0], d);
        n = sum[0];
        overflow = overflow or product[1] != 0 or sum[1] != 0;
        count += 1;
    }
    if (count == 0 or (full and count != s.len)) return nan;
    if (overflow) return inf;
    // Zig/LLVM cannot convert u1024 directly. Round to 53 bits before widening.
    const bits = 1024 - @clz(n);
    if (bits <= 53) return @floatFromInt(@as(u64, @intCast(n)));
    const shift: u10 = @intCast(bits - 53);
    var head: u64 = @intCast(n >> shift);
    const half = @as(u1024, 1) << (shift - 1);
    const tail = n & ((half << 1) - 1);
    head += @intFromBool(tail > half or (tail == half and head & 1 != 0));
    return std.math.ldexp(@as(f64, @floatFromInt(head)), shift);
}

// Return the longest decimal prefix; a dangling exponent is excluded.
fn decimalEnd(s: []const u8) usize {
    var i: usize = 0;
    if (i < s.len and (s[i] == '+' or s[i] == '-')) i += 1;
    if (std.mem.startsWith(u8, s[i..], "Infinity")) return i + 8;
    var digits: usize = 0;
    while (i < s.len and std.ascii.isDigit(s[i])) : (i += 1) digits += 1;
    if (i < s.len and s[i] == '.') {
        i += 1;
        while (i < s.len and std.ascii.isDigit(s[i])) : (i += 1) digits += 1;
    }
    if (digits == 0) return 0;
    const end = i;
    if (i < s.len and (s[i] == 'e' or s[i] == 'E')) {
        i += 1;
        if (i < s.len and (s[i] == '+' or s[i] == '-')) i += 1;
        const start = i;
        while (i < s.len and std.ascii.isDigit(s[i])) : (i += 1) {}
        if (i == start) return end;
    }
    return i;
}

/// Parse a decimal prefix, including signed Infinity; no digits produces NaN.
pub fn parseFloat(input: []const u8) f64 {
    const s = trim(input, false);
    const end = decimalEnd(s);
    if (end == 0) return nan;
    return std.fmt.parseFloat(f64, s[0..end]) catch nan;
}

/// Parse a signed integer prefix, with ToInt32 radix and optional hexadecimal prefix.
pub fn parseInt(input: []const u8, radix: f64) f64 {
    var s = trim(input, false);
    const negative = s.len > 0 and s[0] == '-';
    if (s.len > 0 and (s[0] == '+' or s[0] == '-')) s = s[1..];
    const r = if (!std.math.isFinite(radix)) 0 else @mod(@trunc(radix), 4294967296);
    if (r != 0 and (r < 2 or r > 36)) return nan;
    var base: u8 = if (r == 0) 10 else @intFromFloat(r);
    if ((r == 0 or base == 16) and s.len >= 2 and s[0] == '0' and std.ascii.toLower(s[1]) == 'x') {
        base = 16;
        s = s[2..];
    }
    const value = integer(s, base, false);
    return if (negative) -value else value;
}

/// Full JS Number conversion for strings, booleans and numeric values.
pub fn number(x: anytype) f64 {
    return switch (@typeInfo(@TypeOf(x))) {
        .int, .comptime_int => @floatFromInt(x),
        .float, .comptime_float => @floatCast(x),
        .bool => if (x) 1 else 0,
        else => blk: {
            const s = trim(x, true);
            if (s.len == 0) break :blk 0;
            if (s.len >= 2 and s[0] == '0') {
                const base: u8 = switch (s[1]) {
                    'x', 'X' => 16,
                    'b', 'B' => 2,
                    'o', 'O' => 8,
                    else => 0,
                };
                if (base != 0) break :blk integer(s[2..], base, true);
            }
            if (decimalEnd(s) != s.len) break :blk nan;
            break :blk parseFloat(s);
        },
    };
}

/// Shortest round-trip decimal, using JS's fixed/scientific thresholds and special-value spelling.
pub fn string(x: anytype) []const u8 {
    switch (@typeInfo(@TypeOf(x))) {
        .int, .comptime_int => return std.fmt.allocPrint(ui.frameAllocator(), "{d}", .{x}) catch @panic("TSX format"),
        .bool => return if (x) "true" else "false",
        .float, .comptime_float => {},
        else => return x,
    }
    const n: f64 = @floatCast(x);
    if (std.math.isNan(n)) return "NaN";
    if (std.math.isInf(n)) return if (n < 0) "-Infinity" else "Infinity";
    if (n == 0) return "0";
    var buf: [400]u8 = undefined;
    if (@abs(n) >= 1e-6 and @abs(n) < 1e21) {
        const s = std.fmt.float.render(&buf, n, .{ .mode = .decimal }) catch unreachable;
        return dupe(s);
    }
    const s = std.fmt.float.render(&buf, n, .{ .mode = .scientific }) catch unreachable;
    const e = std.mem.indexOfScalar(u8, s, 'e').?;
    const exponent = std.fmt.parseInt(i32, s[e + 1 ..], 10) catch unreachable;
    return std.fmt.allocPrint(ui.frameAllocator(), "{s}e{s}{d}", .{
        s[0..e], if (exponent >= 0) "+" else "", exponent,
    }) catch @panic("TSX format");
}

fn dupe(s: []const u8) []const u8 {
    return ui.frameAllocator().dupe(u8, s) catch @panic("TSX format");
}

/// Round the exact binary value to 0..20 decimal places, ties away from zero (JS toFixed).
pub fn toFixed(x: f64, digits: f64) []const u8 {
    const d = if (std.math.isNan(digits)) 0 else @trunc(digits);
    if (d < 0 or d > 20 or !std.math.isFinite(d)) @panic("TSX toFixed digits must be 0..20");
    if (!std.math.isFinite(x) or @abs(x) >= 1e21) return string(x);
    const places: usize = @intFromFloat(d);
    const bits: u64 = @bitCast(@abs(x));
    const exponent: i32 = @as(i32, @intCast((bits >> 52) & 0x7ff)) - 1075;
    var scaled: u256 = bits & 0xfffffffffffff;
    if ((bits >> 52) != 0) scaled |= @as(u256, 1) << 52;
    for (0..places) |_| scaled *= 5;
    const shift = @max(-1074, exponent) + @as(i32, @intCast(places));
    if (shift >= 0) {
        scaled <<= @intCast(shift);
    } else if (shift <= -256) {
        scaled = 0;
    } else {
        const right: u8 = @intCast(-shift);
        const half: u256 = @as(u256, 1) << (right - 1);
        scaled = (scaled >> right) + @intFromBool(scaled & ((half << 1) - 1) >= half);
    }
    var buf: [100]u8 = undefined;
    const raw = std.fmt.bufPrint(&buf, "{d}", .{scaled}) catch unreachable;
    const len = @max(raw.len, places + 1);
    const sign: usize = @intFromBool(x < 0);
    const out = tsx.alloc(u8, sign + len + @intFromBool(places > 0));
    if (sign != 0) out[0] = '-';
    const integer_len = len - places;
    for (0..len) |i| {
        if (places > 0 and i == integer_len) out[sign + i] = '.';
        out[sign + i + @intFromBool(places > 0 and i >= integer_len)] =
            if (i < len - raw.len) '0' else raw[i - (len - raw.len)];
    }
    return out;
}

/// Wall-clock milliseconds since the Unix epoch, independent of UI frame time.
pub fn dateNow() f64 {
    return @floatFromInt(std.Io.Clock.real.now(std.Io.Threaded.global_single_threaded.io()).toMilliseconds());
}

/// Monotonic milliseconds with submillisecond precision, independent of UI frame time.
pub fn performanceNow() f64 {
    return @as(f64, @floatFromInt(@import("platform/platform.zig").monotonicUs())) / 1000;
}

var random_lock: @import("core/sync.zig").Mutex = .{};
var prng: ?std.Random.DefaultPrng = null;

/// Process-wide noncryptographic PRNG, seeded once and serialized across callers.
pub fn random() f64 {
    random_lock.lock();
    defer random_lock.unlock();
    if (prng == null) {
        const seed = std.Io.Clock.real.now(std.Io.Threaded.global_single_threaded.io()).toNanoseconds();
        prng = .init(@truncate(@as(u96, @bitCast(seed))));
    }
    return prng.?.random().float(f64);
}

/// Template interpolation shares the same numeric spelling as String(number).
pub const format = @import("tsx_builtins_format.zig").format;

test "Math unary operations, integer precision and IEEE edge cases" {
    const eq = std.testing.expectEqual;
    inline for (.{ .floor, .ceil, .round, .trunc, .abs }) |op| {
        try eq(@as(u64, 9007199254740993), unary(op, @as(u64, 9007199254740993)));
    }
    try eq(@as(i32, 9), unary(.abs, @as(i32, -9)));
    inline for (.{
        .{ Unary.floor, -2.5, -3.0 }, .{ Unary.ceil, -2.5, -2.0 }, .{ Unary.round, -2.5, -2.0 },
        .{ Unary.trunc, -2.5, -2.0 }, .{ Unary.abs, -2.5, 2.5 },   .{ Unary.sign, -2.5, -1.0 },
        .{ Unary.sqrt, 9.0, 3.0 },    .{ Unary.cbrt, -8.0, -2.0 }, .{ Unary.exp, 0.0, 1.0 },
        .{ Unary.log, 1.0, 0.0 },     .{ Unary.log2, 8.0, 3.0 },   .{ Unary.log10, 100.0, 2.0 },
        .{ Unary.sin, 0.0, 0.0 },     .{ Unary.cos, 0.0, 1.0 },    .{ Unary.tan, 0.0, 0.0 },
        .{ Unary.atan, 0.0, 0.0 },
    }) |case| try std.testing.expectApproxEqAbs(@as(f64, case[2]), unary(case[0], @as(f64, case[1])), 1e-14);
    try eq(@as(f64, 3), unary(.round, @as(f64, 2.5)));
    try eq(@as(f64, 0), unary(.round, @as(f64, 0.49999999999999994)));
    try std.testing.expect(std.math.signbit(unary(.round, @as(f64, -0.5))));
    inline for (.{ .floor, .ceil, .round, .trunc, .sign }) |op| {
        try std.testing.expect(std.math.signbit(unary(op, @as(f64, -0.0))));
        try eq(if (op == .sign) @as(f64, 1) else inf, unary(op, inf));
        try std.testing.expect(std.math.isNan(unary(op, nan)));
    }
    try eq(@as(f64, 8), pow(2, 3));
    try eq(@as(f64, 1), pow(nan, 0));
    try std.testing.expect(std.math.isNan(pow(1, nan)) and std.math.isNan(pow(-1, inf)));
    try std.testing.expect(std.math.isNan(pow(-2, 0.5)));
    try eq(-inf, pow(-0.0, -3));
    try eq(@as(f64, std.math.pi), atan2(0, -1));
    try std.testing.expect(std.math.signbit(atan2(-0.0, 1)));
    try eq(@as(f64, 5), hypot(&.{ 3, 4 }));
    try eq(@as(f64, 0), hypot(&.{}));
    try eq(inf, hypot(&.{ nan, inf }));
    try std.testing.expect(std.math.isFinite(hypot(&.{ 1e308, 1e308 })));
    try eq(inf, extrema(f64, true, &.{}));
    try eq(-inf, extrema(f64, false, &.{}));
    try eq(@as(i32, -9), extrema(i32, true, &.{ 7, -9 }));
    try std.testing.expect(std.math.isNan(extrema(f64, false, &.{ 1, nan, 3 })));
    try std.testing.expect(std.math.signbit(extrema(f64, true, &.{ 0.0, -0.0 })));
    try std.testing.expect(!std.math.signbit(extrema(f64, false, &.{ -0.0, 0.0 })));
}

test "Number conversion, predicates and JS prefix parsers" {
    const eq = std.testing.expectEqual;
    inline for (.{
        .{ "", 0.0 },           .{ " \t\u{feff}\u{a0}", 0.0 },
        .{ " 0x1f ", 31.0 },    .{ "0b101", 5.0 },
        .{ "0o17", 15.0 },      .{ " +1.25e2 ", 125.0 },
        .{ "-Infinity", -inf },
    }) |case| {
        try eq(@as(f64, case[1]), number(@as([]const u8, case[0])));
    }
    inline for (.{ "1tail", "0x", "-0x1f", "0b12", "1e", "nan", "inf", "1_0", "\u{85}1" }) |s| {
        try std.testing.expect(std.math.isNan(number(@as([]const u8, s))));
    }
    try eq(@as(f64, 1), number(true));
    try eq(@as(f64, 0), number(false));
    try eq(@as(f64, 42), number(@as(u32, 42)));
    try eq(@as(f64, 1.5), number(@as(f32, 1.5)));
    try std.testing.expect(predicate(.isNaN, nan) and !predicate(.isNaN, "NaN"));
    try std.testing.expect(!predicate(.isFinite, inf) and !predicate(.isFinite, true));
    try std.testing.expect(predicate(.isInteger, @as(u64, 9007199254740993)));
    try std.testing.expect(predicate(.isInteger, @as(f64, -0.0)) and !predicate(.isInteger, @as(f64, 1.5)));
    try eq(@as(f64, -31), parseInt(" \u{2028}-0x1fz", 0));
    try eq(@as(f64, 5), parseInt("+1012", 2));
    try eq(@as(f64, 35), parseInt("z!", 36));
    try eq(@as(f64, 31), parseInt("0x1f", 16));
    try eq(@as(f64, 0), parseInt("0x1f", 10));
    try eq(@as(f64, 12), parseInt("12", 4294967306));
    try std.testing.expect(std.math.isNan(parseInt("a", 10)) and std.math.isNan(parseInt("1", 1)));
    try std.testing.expect(std.math.isNan(parseInt("0x", 0)) and std.math.signbit(parseInt("-0", 0)));
    try eq(@as(f64, 9007199254740992), parseInt("9007199254740993", 10));
    try eq(@as(f64, 125), parseFloat(" \u{3000}+1.25e2tail"));
    try eq(@as(f64, 1), parseFloat("1e+oops"));
    try eq(@as(f64, 0), parseFloat("0x1f"));
    try eq(-inf, parseFloat("-Infinity!"));
    try eq(inf, parseFloat("1e999"));
    try std.testing.expect(std.math.isNan(parseFloat(".")) and std.math.isNan(parseFloat("inf")));
    try std.testing.expect(std.math.signbit(parseFloat("-0")));
}

test "JS number formatting, decimal carries, templates and clocks" {
    ui.beginFrame();
    defer ui.endFrame();
    const eq = std.testing.expectEqualStrings;
    inline for (.{
        .{ 1.0, "1" },                                     .{ -0.0, "0" },
        .{ nan, "NaN" },                                   .{ inf, "Infinity" },
        .{ -inf, "-Infinity" },                            .{ 1e20, "100000000000000000000" },
        .{ 1e21, "1e+21" },                                .{ 1e-6, "0.000001" },
        .{ 1e-7, "1e-7" },                                 .{ 5e-324, "5e-324" },
        .{ 1e308, "1e+308" },                              .{ 1e23, "1e+23" },
        .{ 1000000000000000128.0, "1000000000000000100" },
    }) |case| {
        try eq(case[1], string(@as(f64, case[0])));
    }
    try eq("9007199254740993", string(@as(u64, 9007199254740993)));
    try eq("1000000000000000128", toFixed(1000000000000000128.0, 0));
    try eq("100000000000000000000.00", toFixed(1e20, 2));
    try eq("0.61", toFixed(0.615, 2));
    try eq("1.00", toFixed(1.005, 2));
    try eq("true", string(true));
    try eq("hello", string("hello"));
    inline for (.{
        .{ 0.995, 2.0, "0.99" },                     .{ 9.999, 2.0, "10.00" },
        .{ 1.25, 1.0, "1.3" },                       .{ -1.25, 1.0, "-1.3" },
        .{ -0.0, 2.0, "0.00" },                      .{ -0.001, 2.0, "-0.00" },
        .{ 1e21, 2.0, "1e+21" },                     .{ inf, 2.0, "Infinity" },
        .{ nan, 2.0, "NaN" },                        .{ 0.3, 20.0, "0.29999999999999998890" },
        .{ 5e-324, 20.0, "0.00000000000000000000" },
    }) |case| {
        try eq(case[2], toFixed(case[0], case[1]));
    }
    try eq("{1e+21} 0 NaN Infinity 42", tsx.fmt("{{{d}}} {d} {d} {d} {d}", .{
        @as(f64, 1e21), @as(f64, -0.0), nan, inf, @as(u64, 42),
    }));
    try std.testing.expect(dateNow() > 1700000000000);
    const before = performanceNow();
    try std.testing.expect(performanceNow() >= before);
    for (0..100) |_| {
        const r = random();
        try std.testing.expect(r >= 0 and r < 1);
    }
}
