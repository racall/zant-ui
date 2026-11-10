//! app.config.json → the app's configuration, and the `app_config.zig` the
//! app compiles against. Shared by the `zant` CLI (sync, packaging) and by
//! `zant-tsx` (TS apps generate it while building, for the target platform).
const std = @import("std");

/// The platform the config is for (platform sections override the window).
pub const Os = enum { macos, windows, other };

pub const WindowConfig = struct {
    style: []const u8 = "native",
    width: f32 = 560,
    height: f32 = 420,
    min_width: f32 = 480,
    min_height: f32 = 320,
    background: []const u8 = "#f5f5f5",
    resizable: bool = true,
    always_on_top: bool = false,
    movable_by_background: bool = false,
    center: bool = true,
    /// `hidden-titlebar` style: the height of the app's own title bar
    /// (where the system buttons sit); 0 for the system's default.
    titlebar_height: f32 = 0,
    /// Opened by the system at login: start hidden (a tray app).
    hidden_at_login: bool = false,
};

/// app.config.json. `title`, `bundle_id` (the app identifier: the macOS
/// bundle id, the data folder name on Windows), `version` and `window` are
/// shared by both platforms and written by `zant create`. Platform-only
/// settings live in their own section, added by `zant run` / `build` /
/// `package` on that platform the first time they are needed:
///   "macos": { "sandbox": false, "signing_identity": "none" }
/// (older projects keep `sandbox` / `signing_identity` at the top level).
pub const AppConfig = struct {
    title: []const u8 = "Zant App",
    bundle_id: []const u8 = "dev.zant.app",
    version: []const u8 = "0.1.0",
    /// A second launch brings the running app forward and exits (TS apps:
    /// `zant.tsx.app.run`; Zig apps call `zant.desktop.singleInstance`).
    single_instance: bool = false,
    sandbox: bool = false,
    signing_identity: []const u8 = "none",
    window: WindowConfig = .{},
    /// `macos.entitlements`: extra entitlements for the signed app, e.g.
    /// `{ "com.apple.security.network.client": true }` (values: true / false,
    /// a string or a list of strings). `sandbox` adds app-sandbox itself.
    entitlements: ?std.json.Value = null,
    /// set in the file (`macos` section or legacy top-level keys)
    has_sandbox: bool = false,
    has_signing: bool = false,
    /// read from the legacy top-level keys (moved into `macos` when written)
    legacy_sandbox: bool = false,
    legacy_signing: bool = false,
};

const RawWindow = struct {
    style: ?[]const u8 = null,
    width: ?f32 = null,
    height: ?f32 = null,
    min_width: ?f32 = null,
    min_height: ?f32 = null,
    background: ?[]const u8 = null,
    resizable: ?bool = null,
    always_on_top: ?bool = null,
    movable_by_background: ?bool = null,
    center: ?bool = null,
    titlebar_height: ?f32 = null,
    hidden_at_login: ?bool = null,
};

const RawMacos = struct {
    sandbox: ?bool = null,
    signing_identity: ?[]const u8 = null,
    entitlements: ?std.json.Value = null,
    /// Overrides `window.titlebar_height` on macOS.
    titlebar_height: ?f32 = null,
};

/// Windows-only settings (so far the title bar height override).
const RawWindows = struct {
    titlebar_height: ?f32 = null,
};

const RawConfig = struct {
    title: ?[]const u8 = null,
    bundle_id: ?[]const u8 = null,
    version: ?[]const u8 = null,
    single_instance: ?bool = null,
    sandbox: ?bool = null,
    signing_identity: ?[]const u8 = null,
    window: ?RawWindow = null,
    macos: ?RawMacos = null,
    windows: ?RawWindows = null,
};

pub const ParseError = error{ OutOfMemory, InvalidJson, InvalidEntitlements, InvalidBundleId };

/// Parses app.config.json (`data` must outlive the result: strings point into
/// it). On an error `why` says what is wrong (JSON error, bundle id rule).
pub fn parse(allocator: std.mem.Allocator, data: []const u8, os: Os, why: *[]const u8) ParseError!AppConfig {
    const parsed = std.json.parseFromSliceLeaky(RawConfig, allocator, data, .{ .ignore_unknown_fields = true }) catch |err| {
        why.* = @errorName(err);
        return if (err == error.OutOfMemory) error.OutOfMemory else error.InvalidJson;
    };
    var cfg = AppConfig{};
    if (parsed.title) |v| cfg.title = v;
    if (parsed.bundle_id) |v| cfg.bundle_id = v;
    if (parsed.version) |v| cfg.version = v;
    if (parsed.sandbox) |v| {
        cfg.sandbox = v;
        cfg.has_sandbox = true;
        cfg.legacy_sandbox = true;
    }
    if (parsed.signing_identity) |v| {
        cfg.signing_identity = v;
        cfg.has_signing = true;
        cfg.legacy_signing = true;
    }
    if (parsed.macos) |m| {
        if (m.sandbox) |v| {
            cfg.sandbox = v;
            cfg.has_sandbox = true;
            cfg.legacy_sandbox = false;
        }
        if (m.signing_identity) |v| {
            cfg.signing_identity = v;
            cfg.has_signing = true;
            cfg.legacy_signing = false;
        }
        if (m.entitlements) |v| {
            if (v != .object) return error.InvalidEntitlements;
            cfg.entitlements = v;
        }
    }
    if (parsed.single_instance) |v| cfg.single_instance = v;
    // the platform's own title bar height (macOS 52 / Windows 32 in ClipHop)
    const platform_titlebar = if (os == .windows)
        (if (parsed.windows) |w| w.titlebar_height else null)
    else
        (if (parsed.macos) |m| m.titlebar_height else null);
    if (parsed.window) |w| {
        if (w.style) |v| cfg.window.style = v;
        if (w.width) |v| cfg.window.width = v;
        if (w.height) |v| cfg.window.height = v;
        if (w.min_width) |v| cfg.window.min_width = v;
        if (w.min_height) |v| cfg.window.min_height = v;
        if (w.background) |v| cfg.window.background = v;
        if (w.resizable) |v| cfg.window.resizable = v;
        if (w.always_on_top) |v| cfg.window.always_on_top = v;
        if (w.movable_by_background) |v| cfg.window.movable_by_background = v;
        if (w.center) |v| cfg.window.center = v;
        if (w.titlebar_height) |v| cfg.window.titlebar_height = v;
        if (w.hidden_at_login) |v| cfg.window.hidden_at_login = v;
    }
    if (platform_titlebar) |v| cfg.window.titlebar_height = v;
    if (bundleError(cfg.bundle_id)) |reason| {
        why.* = reason;
        return error.InvalidBundleId;
    }
    if (cfg.window.min_width > cfg.window.width) cfg.window.min_width = cfg.window.width;
    if (cfg.window.min_height > cfg.window.height) cfg.window.min_height = cfg.window.height;
    return cfg;
}

/// `app_config.zig`: what the app reads at compile time (title, bundle id,
/// window options).
pub fn source(allocator: std.mem.Allocator, cfg: *const AppConfig) ![]u8 {
    const title = try escapeZig(allocator, cfg.title);
    defer allocator.free(title);
    const bundle = try escapeZig(allocator, cfg.bundle_id);
    defer allocator.free(bundle);
    const version = try escapeZig(allocator, cfg.version);
    defer allocator.free(version);
    const identity = try escapeZig(allocator, cfg.signing_identity);
    defer allocator.free(identity);
    const background = parseColor(cfg.window.background) catch 0xfff5f5f5;
    return std.fmt.allocPrint(allocator,
        \\const zant = @import("zant");
        \\
        \\// Generated by `zant` from app.config.json. Do not edit by hand.
        \\pub const title: [:0]const u8 = "{s}";
        \\pub const bundle_id: []const u8 = "{s}";
        \\pub const version: []const u8 = "{s}";
        \\pub const single_instance: bool = {};
        \\pub const sandbox: bool = {};
        \\pub const signing_identity: []const u8 = "{s}";
        \\
        \\pub const window: zant.WindowOptions = .{{
        \\    .style = .{s},
        \\    .width = {d},
        \\    .height = {d},
        \\    .min_width = {d},
        \\    .min_height = {d},
        \\    .background = 0x{x:0>8},
        \\    .resizable = {},
        \\    .always_on_top = {},
        \\    .movable_by_background = {},
        \\    .center = {},
        \\    .titlebar_height = {d},
        \\    .hidden_at_login = {},
        \\}};
        \\
    , .{
        title,
        bundle,
        version,
        cfg.single_instance,
        cfg.sandbox,
        identity,
        styleZig(cfg.window.style),
        cfg.window.width,
        cfg.window.height,
        cfg.window.min_width,
        cfg.window.min_height,
        background,
        cfg.window.resizable,
        cfg.window.always_on_top,
        cfg.window.movable_by_background,
        cfg.window.center,
        cfg.window.titlebar_height,
        cfg.window.hidden_at_login,
    });
}

fn parseColor(text: []const u8) !u32 {
    var value = text;
    if (value.len > 0 and value[0] == '#') value = value[1..];
    if (value.len == 6) {
        const rgb = try std.fmt.parseInt(u32, value, 16);
        return 0xff000000 | rgb;
    }
    if (value.len == 8) return try std.fmt.parseInt(u32, value, 16);
    return error.InvalidColor;
}

fn styleZig(style: []const u8) []const u8 {
    if (std.mem.eql(u8, style, "hidden-titlebar") or std.mem.eql(u8, style, "hidden_titlebar")) return "hidden_titlebar";
    if (std.mem.eql(u8, style, "borderless")) return "borderless";
    return "native";
}

fn escapeZig(allocator: std.mem.Allocator, text: []const u8) ![]u8 {
    var out: std.ArrayListUnmanaged(u8) = .empty;
    for (text) |ch| {
        switch (ch) {
            '\\' => try out.appendSlice(allocator, "\\\\"),
            '"' => try out.appendSlice(allocator, "\\\""),
            '\n' => try out.appendSlice(allocator, "\\n"),
            '\r' => try out.appendSlice(allocator, "\\r"),
            '\t' => try out.appendSlice(allocator, "\\t"),
            else => try out.append(allocator, ch),
        }
    }
    return out.toOwnedSlice(allocator);
}

/// Why `id` is not a valid app identifier, or null. The identifier is the
/// macOS CFBundleIdentifier and, on Windows, the data folder name (and fits
/// an MSIX package name), so it follows both:
/// - Apple: only A–Z a–z 0–9 `-` `.`, reverse-DNS ("com.xx.test"); the
///   `com.apple.` prefix is reserved.
/// - Microsoft (MSIX Identity Name): 3–50 characters, no trailing `.`; a part
///   may not be a reserved device name (CON, NUL, COM1…), which Windows also
///   refuses as a folder name.
/// Plus: at least two parts, none empty, each starting with a letter or
/// digit and not ending with `-`, the first starting with a letter.
pub fn bundleError(id: []const u8) ?[]const u8 {
    if (id.len < 3 or id.len > 50) return "长度要在 3～50 个字符之间（微软 MSIX 包名规范）";
    for (id) |ch| {
        if (!(std.ascii.isAlphanumeric(ch) or ch == '-' or ch == '.'))
            return "只能包含英文字母、数字、- 和 .（苹果 Bundle ID 规范；不能有空格、下划线、中文）";
    }
    if (std.mem.indexOfScalar(u8, id, '.') == null) return "要用反向域名格式，至少两段，如 com.xx.test";
    if (!std.ascii.isAlphabetic(id[0])) return "要以英文字母开头";
    if (std.ascii.startsWithIgnoreCase(id, "com.apple.") or std.ascii.eqlIgnoreCase(id, "com.apple"))
        return "com.apple 开头的标识是苹果保留的";
    var parts = std.mem.splitScalar(u8, id, '.');
    while (parts.next()) |part| {
        if (part.len == 0) return "不能以 . 开头或结尾，也不能有连续的 ..";
        if (part[0] == '-' or part[part.len - 1] == '-') return "每一段不能以 - 开头或结尾";
        if (reservedWindowsName(part)) return "不能有 CON、PRN、AUX、NUL、COM1～9、LPT1～9 这样的段（Windows 保留名）";
    }
    return null;
}

fn reservedWindowsName(part: []const u8) bool {
    for ([_][]const u8{ "con", "prn", "aux", "nul" }) |name| {
        if (std.ascii.eqlIgnoreCase(part, name)) return true;
    }
    if (part.len == 4 and part[3] >= '1' and part[3] <= '9' and
        (std.ascii.startsWithIgnoreCase(part, "com") or std.ascii.startsWithIgnoreCase(part, "lpt"))) return true;
    return false;
}

test "app identifier follows Apple and Microsoft rules" {
    for ([_][]const u8{ "com.xx.test", "dev.zant.demo-app", "io.github.user.App2", "Com.Example.MyApp" }) |id| {
        try std.testing.expect(bundleError(id) == null);
    }
    for ([_][]const u8{ "bad", "ab", "com.xx_test", "com.xx test", "com.剪贴板", "1com.xx", "com..xx", "com.xx.", ".com.xx", "com.-xx", "com.xx-", "com.apple.notes", "com.con.app", "com.xx.LPT1", "a.xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx" }) |id| {
        try std.testing.expect(bundleError(id) != null);
    }
}

test "platform title bar overrides and the generated source" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    const json =
        \\{ "title": "Clip", "bundle_id": "io.x.clip", "single_instance": true,
        \\  "window": { "style": "hidden-titlebar", "titlebar_height": 40 },
        \\  "macos": { "titlebar_height": 52 }, "windows": { "titlebar_height": 32 } }
    ;
    var why: []const u8 = "";
    const mac = try parse(a, json, .macos, &why);
    const win = try parse(a, json, .windows, &why);
    try std.testing.expectEqual(@as(f32, 52), mac.window.titlebar_height);
    try std.testing.expectEqual(@as(f32, 32), win.window.titlebar_height);
    const code = try source(a, &win);
    try std.testing.expect(std.mem.indexOf(u8, code, ".titlebar_height = 32,") != null);
    try std.testing.expect(std.mem.indexOf(u8, code, "pub const single_instance: bool = true;") != null);
    try std.testing.expectError(error.InvalidBundleId, parse(a, "{ \"bundle_id\": \"bad\" }", .macos, &why));
}
