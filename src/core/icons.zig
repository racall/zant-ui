//! Zant icon set: line icons on a 24×24 grid with a uniform stroke (in the
//! spirit of Lucide), drawn as vector paths so every icon shares the same
//! optical size and weight at any scale.
const std = @import("std");
const vector = @import("vector.zig");

pub const Icon = enum {
    none,
    // navigation / chrome
    menu,
    list,
    search,
    eye,
    eye_off,
    more,
    more_vertical,
    close,
    plus,
    minus,
    check,
    chevron_down,
    chevron_up,
    chevron_left,
    chevron_right,
    arrow_down,
    arrow_up,
    // files / actions
    file,
    file_text,
    folder,
    save,
    copy,
    download,
    upload,
    link,
    external_link,
    pencil,
    trash,
    undo,
    redo,
    settings,
    // view
    sun,
    moon,
    columns,
    keyboard,
    book_open,
    maximize,
    minimize,
    focus,
    type_size,
    // formatting
    heading,
    bold,
    italic,
    strikethrough,
    code,
    code_block,
    quote,
    list_bullet,
    list_ordered,
    list_checks,
    table,
    sigma,
    image,
    minus_rule,
    // Windows caption buttons (drawn at 10px with a 1px stroke)
    window_minimize,
    window_maximize,
    window_restore,
    window_close,
    // status
    info,
    alert,
    lightbulb,
};

/// Receives one stroked polyline in 24×24 icon space.
pub const Sink = struct {
    ctx: *anyopaque,
    stroke: *const fn (ctx: *anyopaque, points: []const vector.Point, closed: bool) void,
    dot: *const fn (ctx: *anyopaque, x: f32, y: f32, r: f32) void,
};

const Pen = struct {
    sink: Sink,

    fn line(p: Pen, pts: []const vector.Point) void {
        p.sink.stroke(p.sink.ctx, pts, false);
    }

    fn closed(p: Pen, pts: []const vector.Point) void {
        p.sink.stroke(p.sink.ctx, pts, true);
    }

    fn dot(p: Pen, x: f32, y: f32, r: f32) void {
        p.sink.dot(p.sink.ctx, x, y, r);
    }

    fn arc(p: Pen, cx: f32, cy: f32, r: f32, a0_deg: f32, a1_deg: f32) void {
        var pts: [33]vector.Point = undefined;
        const steps: usize = 32;
        for (0..steps + 1) |i| {
            const t = @as(f32, @floatFromInt(i)) / @as(f32, @floatFromInt(steps));
            const a = std.math.degreesToRadians(a0_deg + (a1_deg - a0_deg) * t);
            pts[i] = .{ cx + @cos(a) * r, cy + @sin(a) * r };
        }
        p.line(&pts);
    }

    fn circle(p: Pen, cx: f32, cy: f32, r: f32) void {
        var pts: [32]vector.Point = undefined;
        for (0..pts.len) |i| {
            const a = @as(f32, @floatFromInt(i)) / @as(f32, @floatFromInt(pts.len)) * std.math.tau;
            pts[i] = .{ cx + @cos(a) * r, cy + @sin(a) * r };
        }
        p.closed(&pts);
    }

    /// Rounded rectangle outline.
    fn rect(p: Pen, x: f32, y: f32, w: f32, h: f32, r: f32) void {
        var pts: [28]vector.Point = undefined;
        var n: usize = 0;
        const corners = [_][3]f32{ .{ x + w - r, y + r, -90 }, .{ x + w - r, y + h - r, 0 }, .{ x + r, y + h - r, 90 }, .{ x + r, y + r, 180 } };
        for (corners) |c| {
            for (0..7) |k| {
                const a = std.math.degreesToRadians(c[2] + @as(f32, @floatFromInt(k)) * 15);
                pts[n] = .{ c[0] + @cos(a) * r, c[1] + @sin(a) * r };
                n += 1;
            }
        }
        p.closed(pts[0..n]);
    }
};

/// Emits `icon` in 24×24 space.
pub fn trace(icon: Icon, sink: Sink) void {
    const p: Pen = .{ .sink = sink };
    switch (icon) {
        .none => {},
        .menu => {
            p.line(&.{ .{ 4, 6 }, .{ 20, 6 } });
            p.line(&.{ .{ 4, 12 }, .{ 20, 12 } });
            p.line(&.{ .{ 4, 18 }, .{ 20, 18 } });
        },
        .list => {
            p.line(&.{ .{ 9, 6 }, .{ 20, 6 } });
            p.line(&.{ .{ 9, 12 }, .{ 20, 12 } });
            p.line(&.{ .{ 9, 18 }, .{ 20, 18 } });
            p.dot(4.5, 6, 1.1);
            p.dot(4.5, 12, 1.1);
            p.dot(4.5, 18, 1.1);
        },
        .search => {
            p.circle(10.5, 10.5, 6.5);
            p.line(&.{ .{ 15.5, 15.5 }, .{ 20.5, 20.5 } });
        },
        .eye => {
            p.closed(&.{ .{ 2, 12 }, .{ 4.5, 8.6 }, .{ 8, 6.2 }, .{ 12, 5.4 }, .{ 16, 6.2 }, .{ 19.5, 8.6 }, .{ 22, 12 }, .{ 19.5, 15.4 }, .{ 16, 17.8 }, .{ 12, 18.6 }, .{ 8, 17.8 }, .{ 4.5, 15.4 } });
            p.circle(12, 12, 3);
        },
        .eye_off => {
            p.line(&.{ .{ 4, 8.8 }, .{ 2, 12 }, .{ 4.5, 15.4 }, .{ 8, 17.8 }, .{ 12, 18.6 }, .{ 15, 18.1 } });
            p.line(&.{ .{ 9.5, 5.8 }, .{ 12, 5.4 }, .{ 16, 6.2 }, .{ 19.5, 8.6 }, .{ 22, 12 }, .{ 20.3, 14.4 } });
            p.line(&.{ .{ 3, 3 }, .{ 21, 21 } });
        },
        .more => {
            p.dot(5.5, 12, 1.4);
            p.dot(12, 12, 1.4);
            p.dot(18.5, 12, 1.4);
        },
        .more_vertical => {
            p.dot(12, 5.5, 1.4);
            p.dot(12, 12, 1.4);
            p.dot(12, 18.5, 1.4);
        },
        .close => {
            p.line(&.{ .{ 6, 6 }, .{ 18, 18 } });
            p.line(&.{ .{ 18, 6 }, .{ 6, 18 } });
        },
        .plus => {
            p.line(&.{ .{ 12, 5 }, .{ 12, 19 } });
            p.line(&.{ .{ 5, 12 }, .{ 19, 12 } });
        },
        .minus => p.line(&.{ .{ 5, 12 }, .{ 19, 12 } }),
        .check => p.line(&.{ .{ 5, 12.5 }, .{ 10, 17.5 }, .{ 19.5, 7 } }),
        .chevron_down => p.line(&.{ .{ 6, 9 }, .{ 12, 15 }, .{ 18, 9 } }),
        .chevron_up => p.line(&.{ .{ 6, 15 }, .{ 12, 9 }, .{ 18, 15 } }),
        .chevron_left => p.line(&.{ .{ 15, 6 }, .{ 9, 12 }, .{ 15, 18 } }),
        .chevron_right => p.line(&.{ .{ 9, 6 }, .{ 15, 12 }, .{ 9, 18 } }),
        .arrow_down => {
            p.line(&.{ .{ 12, 5 }, .{ 12, 19 } });
            p.line(&.{ .{ 6, 13 }, .{ 12, 19 }, .{ 18, 13 } });
        },
        .arrow_up => {
            p.line(&.{ .{ 12, 19 }, .{ 12, 5 } });
            p.line(&.{ .{ 6, 11 }, .{ 12, 5 }, .{ 18, 11 } });
        },
        .file => {
            p.closed(&.{ .{ 6, 2.5 }, .{ 14, 2.5 }, .{ 19, 7.5 }, .{ 19, 21.5 }, .{ 6, 21.5 } });
            p.line(&.{ .{ 14, 2.5 }, .{ 14, 7.5 }, .{ 19, 7.5 } });
        },
        .file_text => {
            p.closed(&.{ .{ 6, 2.5 }, .{ 14, 2.5 }, .{ 19, 7.5 }, .{ 19, 21.5 }, .{ 6, 21.5 } });
            p.line(&.{ .{ 14, 2.5 }, .{ 14, 7.5 }, .{ 19, 7.5 } });
            p.line(&.{ .{ 9, 13 }, .{ 16, 13 } });
            p.line(&.{ .{ 9, 17 }, .{ 16, 17 } });
        },
        .folder => p.closed(&.{ .{ 3, 6 }, .{ 9, 6 }, .{ 11, 8.5 }, .{ 21, 8.5 }, .{ 21, 19 }, .{ 3, 19 } }),
        .save => {
            p.closed(&.{ .{ 4, 4 }, .{ 16.5, 4 }, .{ 20, 7.5 }, .{ 20, 20 }, .{ 4, 20 } });
            p.line(&.{ .{ 7.5, 4 }, .{ 7.5, 9 }, .{ 15, 9 }, .{ 15, 4 } });
            p.line(&.{ .{ 7.5, 20 }, .{ 7.5, 14 }, .{ 16.5, 14 }, .{ 16.5, 20 } });
        },
        .copy => {
            p.rect(8.5, 8.5, 12, 12, 2);
            p.line(&.{ .{ 4.5, 15.5 }, .{ 3.5, 15.5 }, .{ 3.5, 3.5 }, .{ 15.5, 3.5 }, .{ 15.5, 4.5 } });
        },
        .download => {
            p.line(&.{ .{ 12, 3.5 }, .{ 12, 15 } });
            p.line(&.{ .{ 7, 10 }, .{ 12, 15 }, .{ 17, 10 } });
            p.line(&.{ .{ 4, 15 }, .{ 4, 20 }, .{ 20, 20 }, .{ 20, 15 } });
        },
        .upload => {
            p.line(&.{ .{ 12, 15 }, .{ 12, 3.5 } });
            p.line(&.{ .{ 7, 8.5 }, .{ 12, 3.5 }, .{ 17, 8.5 } });
            p.line(&.{ .{ 4, 15 }, .{ 4, 20 }, .{ 20, 20 }, .{ 20, 15 } });
        },
        .link => {
            p.line(&.{ .{ 10.5, 13.5 }, .{ 13.5, 10.5 } });
            p.arc(8, 16, 3.6, 45, 225);
            p.line(&.{ .{ 5.5, 13.5 }, .{ 8.5, 10.5 } });
            p.line(&.{ .{ 10.5, 18.5 }, .{ 13.5, 15.5 } });
            p.arc(16, 8, 3.6, 225, 405);
            p.line(&.{ .{ 13.5, 5.5 }, .{ 10.5, 8.5 } });
            p.line(&.{ .{ 18.5, 10.5 }, .{ 15.5, 13.5 } });
        },
        .external_link => {
            p.line(&.{ .{ 18, 13 }, .{ 18, 20 }, .{ 4, 20 }, .{ 4, 6 }, .{ 11, 6 } });
            p.line(&.{ .{ 14, 4 }, .{ 20, 4 }, .{ 20, 10 } });
            p.line(&.{ .{ 10, 14 }, .{ 20, 4 } });
        },
        .pencil => {
            p.closed(&.{ .{ 4, 20 }, .{ 5, 15.5 }, .{ 16, 4.5 }, .{ 19.5, 8 }, .{ 8.5, 19 } });
            p.line(&.{ .{ 13.5, 7 }, .{ 17, 10.5 } });
        },
        .trash => {
            p.line(&.{ .{ 3.5, 6.5 }, .{ 20.5, 6.5 } });
            p.line(&.{ .{ 9, 6.5 }, .{ 9, 3.5 }, .{ 15, 3.5 }, .{ 15, 6.5 } });
            p.line(&.{ .{ 5.5, 6.5 }, .{ 6.5, 20.5 }, .{ 17.5, 20.5 }, .{ 18.5, 6.5 } });
        },
        .undo => {
            p.line(&.{ .{ 8, 4.5 }, .{ 3.5, 9 }, .{ 8, 13.5 } });
            p.line(&.{ .{ 3.5, 9 }, .{ 14.5, 9 } });
            p.arc(14.5, 14, 5, -90, 90);
            p.line(&.{ .{ 14.5, 19 }, .{ 9, 19 } });
        },
        .redo => {
            p.line(&.{ .{ 16, 4.5 }, .{ 20.5, 9 }, .{ 16, 13.5 } });
            p.line(&.{ .{ 20.5, 9 }, .{ 9.5, 9 } });
            p.arc(9.5, 14, 5, 270, 90);
            p.line(&.{ .{ 9.5, 19 }, .{ 15, 19 } });
        },
        .settings => {
            p.circle(12, 12, 3);
            var pts: [16]vector.Point = undefined;
            for (0..16) |i| {
                const a = @as(f32, @floatFromInt(i)) / 16.0 * std.math.tau;
                const r: f32 = if (i % 2 == 0) 9 else 7;
                pts[i] = .{ 12 + @cos(a) * r, 12 + @sin(a) * r };
            }
            p.closed(&pts);
        },
        .sun => {
            p.circle(12, 12, 4);
            for (0..8) |i| {
                const a = @as(f32, @floatFromInt(i)) / 8.0 * std.math.tau;
                p.line(&.{ .{ 12 + @cos(a) * 7, 12 + @sin(a) * 7 }, .{ 12 + @cos(a) * 9.5, 12 + @sin(a) * 9.5 } });
            }
        },
        .moon => {
            // crescent: outer arc of r=9 around (12,12) from 0° to 270°,
            // back along an inner arc bulging towards the centre
            var pts: [50]vector.Point = undefined;
            var n: usize = 0;
            for (0..25) |i| {
                const a = std.math.degreesToRadians(@as(f32, @floatFromInt(i)) / 24.0 * 270.0);
                pts[n] = .{ 12 + @cos(a) * 9, 12 + @sin(a) * 9 };
                n += 1;
            }
            for (1..24) |i| {
                const a = std.math.degreesToRadians(225.0 - @as(f32, @floatFromInt(i)) / 24.0 * 180.0);
                pts[n] = .{ 16.5 + @cos(a) * 6.364, 7.5 + @sin(a) * 6.364 };
                n += 1;
            }
            p.closed(pts[0..n]);
        },
        .columns => {
            p.rect(3.5, 4, 17, 16, 2);
            p.line(&.{ .{ 12, 4 }, .{ 12, 20 } });
        },
        .keyboard => {
            p.rect(2.5, 6, 19, 12, 2);
            for ([_]f32{ 6.5, 10, 13.5, 17 }) |x| p.dot(x, 9.75, 0.9);
            for ([_]f32{ 8.25, 11.75, 15.25 }) |x| p.dot(x, 12.25, 0.9);
            p.line(&.{ .{ 8, 15 }, .{ 16, 15 } });
        },
        .book_open => {
            p.closed(&.{ .{ 2.5, 5 }, .{ 8.5, 5 }, .{ 12, 7.5 }, .{ 12, 20 }, .{ 8.5, 18 }, .{ 2.5, 18 } });
            p.closed(&.{ .{ 21.5, 5 }, .{ 15.5, 5 }, .{ 12, 7.5 }, .{ 12, 20 }, .{ 15.5, 18 }, .{ 21.5, 18 } });
        },
        .maximize => {
            p.line(&.{ .{ 4, 9 }, .{ 4, 4 }, .{ 9, 4 } });
            p.line(&.{ .{ 15, 4 }, .{ 20, 4 }, .{ 20, 9 } });
            p.line(&.{ .{ 20, 15 }, .{ 20, 20 }, .{ 15, 20 } });
            p.line(&.{ .{ 9, 20 }, .{ 4, 20 }, .{ 4, 15 } });
        },
        .minimize => {
            p.line(&.{ .{ 4, 9 }, .{ 9, 9 }, .{ 9, 4 } });
            p.line(&.{ .{ 15, 4 }, .{ 15, 9 }, .{ 20, 9 } });
            p.line(&.{ .{ 20, 15 }, .{ 15, 15 }, .{ 15, 20 } });
            p.line(&.{ .{ 9, 20 }, .{ 9, 15 }, .{ 4, 15 } });
        },
        .focus => {
            p.circle(12, 12, 3);
            p.line(&.{ .{ 4, 8 }, .{ 4, 4 }, .{ 8, 4 } });
            p.line(&.{ .{ 16, 4 }, .{ 20, 4 }, .{ 20, 8 } });
            p.line(&.{ .{ 20, 16 }, .{ 20, 20 }, .{ 16, 20 } });
            p.line(&.{ .{ 8, 20 }, .{ 4, 20 }, .{ 4, 16 } });
        },
        .type_size => {
            p.line(&.{ .{ 3, 7 }, .{ 3, 5 }, .{ 13, 5 }, .{ 13, 7 } });
            p.line(&.{ .{ 8, 5 }, .{ 8, 19 } });
            p.line(&.{ .{ 13, 12 }, .{ 13, 11 }, .{ 21, 11 }, .{ 21, 12 } });
            p.line(&.{ .{ 17, 11 }, .{ 17, 19 } });
        },
        .heading => {
            p.line(&.{ .{ 6, 4.5 }, .{ 6, 19.5 } });
            p.line(&.{ .{ 18, 4.5 }, .{ 18, 19.5 } });
            p.line(&.{ .{ 6, 12 }, .{ 18, 12 } });
        },
        .bold => {
            p.line(&.{ .{ 7, 12 }, .{ 13.5, 12 } });
            p.arc(13.5, 8.25, 3.75, -90, 90);
            p.line(&.{ .{ 13.5, 4.5 }, .{ 7, 4.5 }, .{ 7, 19.5 }, .{ 14.5, 19.5 } });
            p.arc(14.5, 15.75, 3.75, -90, 90);
        },
        .italic => {
            p.line(&.{ .{ 10, 4.5 }, .{ 19, 4.5 } });
            p.line(&.{ .{ 5, 19.5 }, .{ 14, 19.5 } });
            p.line(&.{ .{ 14.5, 4.5 }, .{ 9.5, 19.5 } });
        },
        .strikethrough => {
            p.line(&.{ .{ 4, 12 }, .{ 20, 12 } });
            p.arc(12, 8, 4, 160, 330);
            p.arc(12, 16, 4, -30, 160);
        },
        .code => {
            p.line(&.{ .{ 8.5, 6.5 }, .{ 3, 12 }, .{ 8.5, 17.5 } });
            p.line(&.{ .{ 15.5, 6.5 }, .{ 21, 12 }, .{ 15.5, 17.5 } });
        },
        .code_block => {
            p.rect(3, 3.5, 18, 17, 2.5);
            p.line(&.{ .{ 10, 9 }, .{ 7, 12 }, .{ 10, 15 } });
            p.line(&.{ .{ 14, 9 }, .{ 17, 12 }, .{ 14, 15 } });
        },
        .quote => {
            p.line(&.{ .{ 5, 5 }, .{ 5, 19 } });
            p.line(&.{ .{ 10, 8 }, .{ 20, 8 } });
            p.line(&.{ .{ 10, 12 }, .{ 20, 12 } });
            p.line(&.{ .{ 10, 16 }, .{ 16, 16 } });
        },
        .list_bullet => {
            p.line(&.{ .{ 9, 6 }, .{ 20, 6 } });
            p.line(&.{ .{ 9, 12 }, .{ 20, 12 } });
            p.line(&.{ .{ 9, 18 }, .{ 20, 18 } });
            p.dot(4.5, 6, 1.2);
            p.dot(4.5, 12, 1.2);
            p.dot(4.5, 18, 1.2);
        },
        .list_ordered => {
            p.line(&.{ .{ 10, 6 }, .{ 20, 6 } });
            p.line(&.{ .{ 10, 12 }, .{ 20, 12 } });
            p.line(&.{ .{ 10, 18 }, .{ 20, 18 } });
            p.line(&.{ .{ 4, 4.5 }, .{ 5.5, 3.5 }, .{ 5.5, 8 } });
            p.line(&.{ .{ 3.8, 10.8 }, .{ 5, 10 }, .{ 6.4, 10.6 }, .{ 3.8, 14 }, .{ 6.6, 14 } });
        },
        .list_checks => {
            p.line(&.{ .{ 3, 6.5 }, .{ 5, 8.5 }, .{ 8.5, 4.5 } });
            p.line(&.{ .{ 3, 15.5 }, .{ 5, 17.5 }, .{ 8.5, 13.5 } });
            p.line(&.{ .{ 12, 6.5 }, .{ 21, 6.5 } });
            p.line(&.{ .{ 12, 15.5 }, .{ 21, 15.5 } });
        },
        .table => {
            p.rect(3, 4, 18, 16, 2);
            p.line(&.{ .{ 3, 9.5 }, .{ 21, 9.5 } });
            p.line(&.{ .{ 3, 15 }, .{ 21, 15 } });
            p.line(&.{ .{ 10, 4 }, .{ 10, 20 } });
        },
        .sigma => p.line(&.{ .{ 18, 6.5 }, .{ 18, 4 }, .{ 6, 4 }, .{ 12.5, 12 }, .{ 6, 20 }, .{ 18, 20 }, .{ 18, 17.5 } }),
        .image => {
            p.rect(3, 4, 18, 16, 2.5);
            p.circle(8.5, 9.5, 1.8);
            p.line(&.{ .{ 21, 15.5 }, .{ 16, 10.5 }, .{ 5, 20 } });
        },
        .minus_rule => {
            p.line(&.{ .{ 3, 12 }, .{ 21, 12 } });
            p.line(&.{ .{ 3, 6 }, .{ 9, 6 } });
            p.line(&.{ .{ 3, 18 }, .{ 9, 18 } });
        },
        .info => {
            p.circle(12, 12, 9);
            p.line(&.{ .{ 12, 11 }, .{ 12, 16.5 } });
            p.dot(12, 7.8, 1.1);
        },
        .alert => {
            p.closed(&.{ .{ 12, 3.5 }, .{ 21.5, 20 }, .{ 2.5, 20 } });
            p.line(&.{ .{ 12, 9.5 }, .{ 12, 14 } });
            p.dot(12, 17, 1.1);
        },
        .lightbulb => {
            p.arc(12, 9.5, 6, 140, 400);
            p.line(&.{ .{ 7.4, 13.3 }, .{ 9, 16 }, .{ 15, 16 }, .{ 16.6, 13.3 } });
            p.line(&.{ .{ 9.5, 19.5 }, .{ 14.5, 19.5 } });
        },
        // 24 units = the 10px glyph box of Windows 11 caption buttons
        .window_minimize => p.line(&.{ .{ 0, 12 }, .{ 24, 12 } }),
        .window_maximize => p.rect(0, 0, 24, 24, 2.4),
        .window_restore => {
            p.rect(0, 4.8, 19.2, 19.2, 2.4);
            p.line(&.{ .{ 4.8, 4.8 }, .{ 4.8, 2.4 }, .{ 7.2, 0 }, .{ 21.6, 0 }, .{ 24, 2.4 }, .{ 24, 16.8 }, .{ 21.6, 19.2 }, .{ 19.2, 19.2 } });
        },
        .window_close => {
            p.line(&.{ .{ 0, 0 }, .{ 24, 24 } });
            p.line(&.{ .{ 24, 0 }, .{ 0, 24 } });
        },
    }
}

/// Stroke width in icon units for a given pixel size (≈1.5px at 16px).
pub fn defaultStroke(size: f32) f32 {
    return @max(1.2, size / 24.0 * 2.0);
}

// ── immediate painting (buttons) ──────────────────────────────────────────

extern fn zant_canvas_stroke_path(canvas: ?*anyopaque, xy: [*]const f32, count: c_int, color: u32, width: f32, closed: c_int) void;
extern fn zant_canvas_fill_round_rect(canvas: ?*anyopaque, x: f32, y: f32, width: f32, height: f32, radius: f32, color: u32) void;

const PaintCtx = struct { canvas: ?*anyopaque, x: f32, y: f32, scale: f32, color: u32, width: f32 };

fn paintStroke(raw: *anyopaque, points: []const vector.Point, closed: bool) void {
    const c: *PaintCtx = @ptrCast(@alignCast(raw));
    var buf: [256]f32 = undefined;
    const n = @min(points.len, buf.len / 2);
    for (points[0..n], 0..) |pt, i| {
        buf[i * 2] = c.x + pt[0] * c.scale;
        buf[i * 2 + 1] = c.y + pt[1] * c.scale;
    }
    zant_canvas_stroke_path(c.canvas, &buf, @intCast(n), c.color, c.width, @intFromBool(closed));
}

fn paintDot(raw: *anyopaque, x: f32, y: f32, r: f32) void {
    const c: *PaintCtx = @ptrCast(@alignCast(raw));
    const rr = r * c.scale + c.width * 0.25;
    zant_canvas_fill_round_rect(c.canvas, c.x + x * c.scale - rr, c.y + y * c.scale - rr, rr * 2, rr * 2, rr, c.color);
}

/// Paints `icon` centred at (cx, cy) with a `size`-px box.
pub fn paint(canvas: ?*anyopaque, icon: Icon, cx: f32, cy: f32, size: f32, color: u32, stroke: ?f32) void {
    var ctx: PaintCtx = .{
        .canvas = canvas,
        .x = cx - size / 2,
        .y = cy - size / 2,
        .scale = size / 24.0,
        .color = color,
        .width = stroke orelse defaultStroke(size),
    };
    trace(icon, .{ .ctx = &ctx, .stroke = paintStroke, .dot = paintDot });
}

// ── retained drawings (ui.icon) ───────────────────────────────────────────

const BuildCtx = struct {
    arena: std.mem.Allocator,
    cmds: std.ArrayListUnmanaged(vector.Command) = .empty,
    scale: f32,
    top: f32,
    color: u32,
    width: f32,
    failed: bool = false,
};

fn buildStroke(raw: *anyopaque, points: []const vector.Point, closed: bool) void {
    const c: *BuildCtx = @ptrCast(@alignCast(raw));
    const pts = c.arena.alloc(vector.Point, points.len) catch {
        c.failed = true;
        return;
    };
    for (points, 0..) |pt, i| pts[i] = .{ pt[0] * c.scale, pt[1] * c.scale - c.top };
    c.cmds.append(c.arena, .{ .path = .{ .points = pts, .color = c.color, .stroke_width = c.width, .closed = closed } }) catch {
        c.failed = true;
    };
}

fn buildDot(raw: *anyopaque, x: f32, y: f32, r: f32) void {
    const c: *BuildCtx = @ptrCast(@alignCast(raw));
    const rr = r * c.scale + c.width * 0.25;
    c.cmds.append(c.arena, .{ .rect = .{ .x = x * c.scale - rr, .y = y * c.scale - c.top - rr, .width = rr * 2, .height = rr * 2, .radius = rr, .color = c.color } }) catch {
        c.failed = true;
    };
}

/// A `size`×`size` drawing whose box sits entirely above the baseline.
pub fn drawing(arena: std.mem.Allocator, icon: Icon, size: f32, color: u32, stroke: ?f32) !vector.Drawing {
    var ctx: BuildCtx = .{ .arena = arena, .scale = size / 24.0, .top = size, .color = color, .width = stroke orelse defaultStroke(size) };
    trace(icon, .{ .ctx = &ctx, .stroke = buildStroke, .dot = buildDot });
    if (ctx.failed) return error.OutOfMemory;
    var h = std.hash.Wyhash.init(0x69636f6e);
    h.update(std.mem.asBytes(&icon));
    h.update(std.mem.asBytes(&size));
    h.update(std.mem.asBytes(&color));
    h.update(std.mem.asBytes(&ctx.width));
    return .{ .width = size, .ascent = size, .descent = 0, .commands = ctx.cmds.items, .hash = h.final() };
}

test "every icon traces" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    for (std.enums.values(Icon)) |icon| {
        const d = try drawing(arena.allocator(), icon, 18, 0xff000000, null);
        if (icon != .none) try std.testing.expect(d.commands.len > 0);
    }
}
