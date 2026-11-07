const std = @import("std");
const build_options = @import("build_options");
const Io = std.Io;
const Dir = Io.Dir;
pub const is_windows = @import("builtin").os.tag == .windows;
const png = @import("png");
const distribute = @import("distribute.zig");

pub var child_environ: ?*std.process.Environ.Map = null;
/// Run before `fail` exits: `zant build` removes temporary key files and
/// keychains there.
pub var on_fail: ?*const fn () void = null;

pub const Color = struct {
    pub const cyan = "\x1b[36m";
    pub const green = "\x1b[32m";
    pub const red = "\x1b[31m";
    pub const dim = "\x1b[2m";
    pub const reset = "\x1b[0m";
};

const WindowConfig = struct {
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
};

const RawMacos = struct {
    sandbox: ?bool = null,
    signing_identity: ?[]const u8 = null,
    entitlements: ?std.json.Value = null,
};

const RawConfig = struct {
    title: ?[]const u8 = null,
    bundle_id: ?[]const u8 = null,
    version: ?[]const u8 = null,
    sandbox: ?bool = null,
    signing_identity: ?[]const u8 = null,
    window: ?RawWindow = null,
    macos: ?RawMacos = null,
};

pub fn step(comptime fmt: []const u8, args: anytype) void {
    std.debug.print("  " ++ Color.cyan ++ "→" ++ Color.reset ++ " " ++ fmt ++ "\n", args);
}

pub fn ok(comptime fmt: []const u8, args: anytype) void {
    std.debug.print("  " ++ Color.green ++ "✓" ++ Color.reset ++ " " ++ fmt ++ "\n", args);
}

pub fn fail(comptime fmt: []const u8, args: anytype) noreturn {
    if (on_fail) |cleanup| {
        on_fail = null;
        cleanup();
    }
    std.debug.print("\n  " ++ Color.red ++ "✗" ++ Color.reset ++ " " ++ fmt ++ "\n\n", args);
    console.deinit();
    std.process.exit(1);
}

pub fn writeFile(allocator: std.mem.Allocator, io: Io, path: []const u8, data: []const u8) !void {
    if (std.fs.path.dirname(path)) |parent| {
        try Dir.cwd().createDirPath(io, parent);
    }
    try Dir.cwd().writeFile(io, .{ .sub_path = path, .data = data });
    _ = allocator;
}

pub fn readFile(allocator: std.mem.Allocator, io: Io, path: []const u8) ![]u8 {
    return Dir.cwd().readFileAlloc(io, path, allocator, .limited(8 * 1024 * 1024));
}

pub fn runInherit(allocator: std.mem.Allocator, io: Io, argv: []const []const u8, cwd: []const u8) !void {
    _ = allocator;
    var child = std.process.spawn(io, .{
        .argv = argv,
        .cwd = .{ .path = cwd },
        .stdin = .inherit,
        .stdout = .inherit,
        .stderr = .inherit,
        .create_no_window = false,
        .environ_map = child_environ,
    }) catch |err| fail("无法启动 {s}: {s}", .{ argv[0], @errorName(err) });
    const term = child.wait(io) catch |err| fail("等待 {s} 失败: {s}", .{ argv[0], @errorName(err) });
    if (!term.success()) fail("{s} 执行失败", .{argv[0]});
}

pub fn runCapture(allocator: std.mem.Allocator, io: Io, argv: []const []const u8) !std.process.RunResult {
    return std.process.run(allocator, io, .{
        .argv = argv,
        .stdout_limit = .limited(1024 * 1024),
        .stderr_limit = .limited(1024 * 1024),
        .reserve_amount = 4096,
        .environ_map = child_environ,
    });
}

/// Windows console: UTF-8 output (the console default is the OEM code page,
/// e.g. 936 on Chinese systems, which shows UTF-8 text as mojibake) and ANSI
/// colors (off by default in conhost). Restored on exit: the code page belongs
/// to the console window, not to this process.
pub const console = struct {
    const win = struct {
        extern "kernel32" fn GetStdHandle(id: u32) callconv(.winapi) ?*anyopaque;
        extern "kernel32" fn GetConsoleOutputCP() callconv(.winapi) u32;
        extern "kernel32" fn SetConsoleOutputCP(cp: u32) callconv(.winapi) c_int;
        extern "kernel32" fn GetConsoleMode(h: ?*anyopaque, mode: *u32) callconv(.winapi) c_int;
        extern "kernel32" fn SetConsoleMode(h: ?*anyopaque, mode: u32) callconv(.winapi) c_int;
        extern "kernel32" fn ReadConsoleW(h: ?*anyopaque, buf: [*]u16, len: u32, read: *u32, control: ?*anyopaque) callconv(.winapi) c_int;
        extern "kernel32" fn SetConsoleCtrlHandler(handler: ?*const fn (u32) callconv(.winapi) c_int, add: c_int) callconv(.winapi) c_int;
        extern "kernel32" fn ReadFile(file: ?*anyopaque, buffer: [*]u8, len: u32, read: *u32, overlapped: ?*anyopaque) callconv(.winapi) c_int;
    };
    const STD_INPUT_HANDLE: u32 = @bitCast(@as(i32, -10));
    const STD_OUTPUT_HANDLE: u32 = @bitCast(@as(i32, -11));
    const STD_ERROR_HANDLE: u32 = @bitCast(@as(i32, -12));
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
    var saved_cp: u32 = 0;

    fn init() void {
        if (!is_windows) return;
        saved_cp = win.GetConsoleOutputCP();
        if (saved_cp != 0 and saved_cp != 65001) _ = win.SetConsoleOutputCP(65001);
        for ([_]u32{ STD_OUTPUT_HANDLE, STD_ERROR_HANDLE }) |id| {
            const h = win.GetStdHandle(id);
            var mode: u32 = 0;
            if (win.GetConsoleMode(h, &mode) != 0) _ = win.SetConsoleMode(h, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
        }
        // Ctrl+C (`zant run --watch` ends that way) skips deferred cleanup
        _ = win.SetConsoleCtrlHandler(onCtrl, 1);
    }

    fn onCtrl(_: u32) callconv(.winapi) c_int {
        deinit();
        return 0; // default handling (exit) continues
    }

    fn deinit() void {
        if (!is_windows) return;
        if (saved_cp != 0 and saved_cp != 65001) _ = win.SetConsoleOutputCP(saved_cp);
    }

    /// One line of input (without the line break) as UTF-8. A console is read
    /// as UTF-16 (ReadFile returns no bytes for non-ASCII input under code
    /// page 65001 and OEM bytes otherwise); pipes are read as UTF-8 bytes.
    pub fn readLine(buf: []u8) []const u8 {
        if (is_windows) {
            const h = win.GetStdHandle(STD_INPUT_HANDLE);
            var mode: u32 = 0;
            if (win.GetConsoleMode(h, &mode) != 0) {
                var wide: [512]u16 = undefined;
                var read: u32 = 0;
                if (win.ReadConsoleW(h, &wide, wide.len, &read, null) == 0) return buf[0..0];
                var line: []const u16 = wide[0..read];
                while (line.len > 0 and (line[line.len - 1] == '\n' or line[line.len - 1] == '\r')) line = line[0 .. line.len - 1];
                const n = std.unicode.utf16LeToUtf8(buf, line) catch 0;
                return buf[0..n];
            }
        }
        var n: usize = 0;
        while (n < buf.len) {
            if (readByte(buf[n .. n + 1]) == 0 or buf[n] == '\n') break;
            n += 1;
        }
        return std.mem.trimEnd(u8, buf[0..n], "\r");
    }

    /// A line typed without echo (a password); used for this run only.
    pub fn readSecret(buf: []u8) []const u8 {
        if (is_windows) {
            const h = win.GetStdHandle(STD_INPUT_HANDLE);
            var mode: u32 = 0;
            const tty = win.GetConsoleMode(h, &mode) != 0;
            if (tty) _ = win.SetConsoleMode(h, mode & ~@as(u32, 0x0004)); // ENABLE_ECHO_INPUT
            defer if (tty) {
                _ = win.SetConsoleMode(h, mode);
            };
            const line = readLine(buf);
            std.debug.print("\n", .{});
            return line;
        }
        const saved = std.posix.tcgetattr(std.posix.STDIN_FILENO) catch return readLine(buf);
        var quiet = saved;
        quiet.lflag.ECHO = false;
        std.posix.tcsetattr(std.posix.STDIN_FILENO, .NOW, quiet) catch {};
        defer std.posix.tcsetattr(std.posix.STDIN_FILENO, .NOW, saved) catch {};
        const line = readLine(buf);
        std.debug.print("\n", .{});
        return line;
    }

    fn readByte(buf: []u8) usize {
        if (is_windows) {
            var read: u32 = 0;
            if (win.ReadFile(win.GetStdHandle(STD_INPUT_HANDLE), buf.ptr, @intCast(buf.len), &read, null) == 0) return 0;
            return read;
        }
        return std.posix.read(std.posix.STDIN_FILENO, buf) catch 0;
    }
};

pub fn prompt(allocator: std.mem.Allocator, label: []const u8, default: []const u8) ![]u8 {
    std.debug.print("{s} [{s}]: ", .{ label, default });
    var buf: [2048]u8 = undefined;
    const value = std.mem.trim(u8, console.readLine(&buf), " ");
    return allocator.dupe(u8, if (value.len == 0) default else value);
}

/// Asks for the app identifier until it follows the rules; three invalid
/// answers end `zant create` before anything is written. The suggested
/// default (from the project name) is offered only when it is valid itself.
fn askBundle(allocator: std.mem.Allocator, suggested: []const u8) ![]const u8 {
    const default: []const u8 = if (bundleError(suggested) == null) suggested else "";
    var attempts: usize = 0;
    while (attempts < 3) : (attempts += 1) {
        std.debug.print("\n", .{});
        const value = if (default.len > 0)
            try prompt(allocator, "应用标识（反向域名，如 com.xx.test）", default)
        else blk: {
            std.debug.print("应用标识（反向域名，如 com.xx.test）: ", .{});
            var buf: [256]u8 = undefined;
            break :blk try allocator.dupe(u8, std.mem.trim(u8, console.readLine(&buf), " "));
        };
        if (value.len == 0) {
            std.debug.print("  " ++ Color.red ++ "✗" ++ Color.reset ++ " 应用标识不能为空，请输入（如 com.xx.test）\n", .{});
            continue;
        }
        if (bundleError(value)) |why| {
            std.debug.print("  " ++ Color.red ++ "✗" ++ Color.reset ++ " “{s}”不符合规范：{s}，请重新输入\n", .{ value, why });
            continue;
        }
        return value;
    }
    fail("应用标识连续 3 次不符合规范，未创建项目", .{});
}

pub fn promptYesNo(allocator: std.mem.Allocator, label: []const u8, default: bool) !bool {
    const def = if (default) "Y/n" else "y/N";
    while (true) {
        std.debug.print("{s} [{s}]: ", .{ label, def });
        var buf: [64]u8 = undefined;
        const v = std.mem.trim(u8, console.readLine(&buf), " \r\n");
        if (v.len == 0) return default;
        if (std.ascii.eqlIgnoreCase(v, "y") or std.ascii.eqlIgnoreCase(v, "yes")) return true;
        if (std.ascii.eqlIgnoreCase(v, "n") or std.ascii.eqlIgnoreCase(v, "no")) return false;
        _ = allocator;
    }
}

pub fn listIdentities(allocator: std.mem.Allocator, io: Io) ![]const []const u8 {
    if (is_windows) return &.{}; // code signing identities are a macOS concept
    const result = runCapture(allocator, io, &.{ "security", "find-identity", "-v", "-p", "codesigning" }) catch return &.{};
    defer allocator.free(result.stdout);
    defer allocator.free(result.stderr);
    var list: std.ArrayListUnmanaged([]const u8) = .empty;
    var lines = std.mem.splitScalar(u8, result.stdout, '\n');
    while (lines.next()) |line| {
        if (std.mem.indexOfScalar(u8, line, '"')) |start| {
            if (std.mem.indexOfScalarPos(u8, line, start + 1, '"')) |end| {
                const identity = try allocator.dupe(u8, line[start + 1 .. end]);
                try list.append(allocator, identity);
            }
        }
    }
    return list.toOwnedSlice(allocator);
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

pub fn escapeXml(allocator: std.mem.Allocator, text: []const u8) ![]u8 {
    var out: std.ArrayListUnmanaged(u8) = .empty;
    for (text) |ch| {
        switch (ch) {
            '&' => try out.appendSlice(allocator, "&amp;"),
            '<' => try out.appendSlice(allocator, "&lt;"),
            '>' => try out.appendSlice(allocator, "&gt;"),
            '"' => try out.appendSlice(allocator, "&quot;"),
            else => try out.append(allocator, ch),
        }
    }
    return out.toOwnedSlice(allocator);
}

pub fn loadConfig(allocator: std.mem.Allocator, io: Io, root: []const u8) !AppConfig {
    const path = try std.fs.path.join(allocator, &.{ root, "app.config.json" });
    defer allocator.free(path);
    const data = readFile(allocator, io, path) catch fail("找不到 app.config.json（请先运行 zant create）", .{});
    // Keep data alive: strings may point into it. All CLI allocations use the
    // process-lifetime arena below.
    const parsed = std.json.parseFromSliceLeaky(RawConfig, allocator, data, .{ .ignore_unknown_fields = true }) catch |err|
        fail("app.config.json 解析失败: {s}", .{@errorName(err)});
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
            if (v != .object) fail("app.config.json 的 macos.entitlements 要是对象，如 {{ \"com.apple.security.network.client\": true }}", .{});
            cfg.entitlements = v;
        }
    }
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
    }
    if (bundleError(cfg.bundle_id)) |why| fail("app.config.json 的 bundle_id \"{s}\" 不符合规范：{s}（如 com.xx.test）", .{ cfg.bundle_id, why });
    if (cfg.window.min_width > cfg.window.width) cfg.window.min_width = cfg.window.width;
    if (cfg.window.min_height > cfg.window.height) cfg.window.min_height = cfg.window.height;
    return cfg;
}

fn generateConfigSource(allocator: std.mem.Allocator, cfg: *const AppConfig) ![]u8 {
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
        \\}};
        \\
    , .{
        title,
        bundle,
        version,
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
    });
}

fn generateInfoPlist(allocator: std.mem.Allocator, cfg: *const AppConfig) ![]u8 {
    return generateInfoPlistFor(allocator, cfg, .{});
}

pub const PlistExtras = struct {
    icon: bool = false,
    /// CFBundleVersion (App Store Connect needs a new one for every upload of
    /// the same version); defaults to `version`
    build_number: ?[]const u8 = null,
    /// LSApplicationCategoryType, e.g. public.app-category.utilities
    category: ?[]const u8 = null,
};

fn generateInfoPlistFor(allocator: std.mem.Allocator, cfg: *const AppConfig, extras: PlistExtras) ![]u8 {
    const title = try escapeXml(allocator, cfg.title);
    defer allocator.free(title);
    const bundle = try escapeXml(allocator, cfg.bundle_id);
    defer allocator.free(bundle);
    const build = try escapeXml(allocator, extras.build_number orelse cfg.version);
    const category = if (extras.category) |c|
        try std.fmt.allocPrint(allocator, "\n<key>LSApplicationCategoryType</key><string>{s}</string>", .{try escapeXml(allocator, c)})
    else
        "";
    return std.fmt.allocPrint(allocator,
        \\<?xml version="1.0" encoding="UTF-8"?>
        \\<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
        \\<plist version="1.0"><dict>
        \\<key>CFBundleDevelopmentRegion</key><string>zh_CN</string>
        \\<key>CFBundleDisplayName</key><string>{s}</string>
        \\<key>CFBundleExecutable</key><string>app</string>{s}
        \\<key>CFBundleIdentifier</key><string>{s}</string>
        \\<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
        \\<key>CFBundleName</key><string>{s}</string>
        \\<key>CFBundlePackageType</key><string>APPL</string>
        \\<key>CFBundleShortVersionString</key><string>{s}</string>
        \\<key>CFBundleVersion</key><string>{s}</string>{s}
        \\<key>LSMinimumSystemVersion</key><string>13.0</string>
        \\<key>NSHighResolutionCapable</key><true/>
        \\<key>NSPrincipalClass</key><string>NSApplication</string>
        \\</dict></plist>
        \\
    , .{ title, if (extras.icon) "\n<key>CFBundleIconFile</key><string>AppIcon</string>" else "", bundle, title, cfg.version, build, category });
}

/// src/app_config.zig (both platforms).
fn writeConfigSource(allocator: std.mem.Allocator, io: Io, root: []const u8, cfg: *const AppConfig) !void {
    const config_source = try generateConfigSource(allocator, cfg);
    defer allocator.free(config_source);
    const config_path = try std.fs.path.join(allocator, &.{ root, "src", "app_config.zig" });
    defer allocator.free(config_path);
    try writeFile(allocator, io, config_path, config_source);
}

/// Generated files for building on this platform: src/app_config.zig, plus
/// Info.plist and app.entitlements on macOS.
pub fn syncProject(allocator: std.mem.Allocator, io: Io, root: []const u8, cfg: *const AppConfig) !void {
    try writeConfigSource(allocator, io, root, cfg);
    if (is_windows) return; // Info.plist and entitlements are macOS bundle files

    const plist = try generateInfoPlist(allocator, cfg);
    defer allocator.free(plist);
    const plist_path = try std.fs.path.join(allocator, &.{ root, "Info.plist" });
    defer allocator.free(plist_path);
    try writeFile(allocator, io, plist_path, plist);

    if (cfg.sandbox or cfg.entitlements != null) {
        const ent_path = try std.fs.path.join(allocator, &.{ root, "app.entitlements" });
        try writeFile(allocator, io, ent_path, try entitlementsPlist(allocator, cfg, null));
    }
}

/// The app's entitlements: app-sandbox when `macos.sandbox` is on, the
/// `macos.entitlements` from app.config.json, and for the Mac App Store the
/// application and team identifiers (`store` = team id; sandbox is forced).
pub fn entitlementsPlist(allocator: std.mem.Allocator, cfg: *const AppConfig, store_team: ?[]const u8) ![]u8 {
    var out: std.ArrayListUnmanaged(u8) = .empty;
    try out.appendSlice(allocator,
        \\<?xml version="1.0" encoding="UTF-8"?>
        \\<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
        \\<plist version="1.0"><dict>
        \\
    );
    if (cfg.sandbox or store_team != null) try out.appendSlice(allocator, "<key>com.apple.security.app-sandbox</key><true/>\n");
    if (store_team) |team| {
        const t = try escapeXml(allocator, team);
        const id = try escapeXml(allocator, cfg.bundle_id);
        try out.print(allocator, "<key>com.apple.application-identifier</key><string>{s}.{s}</string>\n", .{ t, id });
        try out.print(allocator, "<key>com.apple.developer.team-identifier</key><string>{s}</string>\n", .{t});
    }
    if (cfg.entitlements) |ents| {
        var it = ents.object.iterator();
        while (it.next()) |entry| {
            const key = entry.key_ptr.*;
            if (std.mem.eql(u8, key, "com.apple.security.app-sandbox") and (cfg.sandbox or store_team != null)) continue;
            try out.print(allocator, "<key>{s}</key>", .{try escapeXml(allocator, key)});
            switch (entry.value_ptr.*) {
                .bool => |v| try out.appendSlice(allocator, if (v) "<true/>" else "<false/>"),
                .string => |v| try out.print(allocator, "<string>{s}</string>", .{try escapeXml(allocator, v)}),
                .array => |list| {
                    try out.appendSlice(allocator, "<array>");
                    for (list.items) |item| {
                        if (item != .string) fail("macos.entitlements 的 {s}：列表里只能是字符串", .{key});
                        try out.print(allocator, "<string>{s}</string>", .{try escapeXml(allocator, item.string)});
                    }
                    try out.appendSlice(allocator, "</array>");
                },
                else => fail("macos.entitlements 的 {s}：值只能是 true / false、字符串或字符串列表", .{key}),
            }
            try out.append(allocator, '\n');
        }
    }
    try out.appendSlice(allocator, "</dict></plist>\n");
    return out.toOwnedSlice(allocator);
}

const Need = enum { run, package };

/// Adds this platform's settings to app.config.json when they are missing
/// (projects are created with the shared settings only). On macOS: `sandbox`
/// when running or building, the signing identity when packaging (asked in
/// an interactive terminal, ad-hoc otherwise). Windows needs nothing yet.
pub fn completePlatformConfig(allocator: std.mem.Allocator, io: Io, root: []const u8, cfg: *AppConfig, need: Need) !void {
    if (is_windows) return;
    const want_signing = need == .package and !cfg.has_signing;
    const want_sandbox = !cfg.has_sandbox;
    if (!want_sandbox and !want_signing) return;
    if (want_signing) {
        cfg.signing_identity = if (try std.Io.File.stdin().isTty(io)) try askSigningIdentity(allocator, io) else "none";
        cfg.has_signing = true;
    }
    cfg.has_sandbox = true;
    try writeMacosSection(allocator, io, root, cfg, want_sandbox, want_signing);
    if (want_signing) {
        ok("已在 app.config.json 补充 macOS 打包账号：{s}", .{if (std.mem.eql(u8, cfg.signing_identity, "none")) "Ad-hoc" else cfg.signing_identity});
    } else {
        ok("已在 app.config.json 补充 macOS 设置（macos.sandbox = {}，打包时再选择签名账号）", .{cfg.sandbox});
    }
}

fn askSigningIdentity(allocator: std.mem.Allocator, io: Io) ![]const u8 {
    const identities = try listIdentities(allocator, io);
    std.debug.print("\n打包开发者账号（macOS）:\n  0) Ad-hoc（无开发者账号）\n", .{});
    for (identities, 0..) |id, idx| std.debug.print("  {d}) {s}\n", .{ idx + 1, id });
    const value = try prompt(allocator, "选择", "0");
    if (std.mem.eql(u8, value, "0") or value.len == 0) return "none";
    if (std.fmt.parseInt(usize, value, 10)) |idx| {
        if (idx >= 1 and idx <= identities.len) return identities[idx - 1];
        return "none";
    } else |_| return value;
}

/// Adds the missing `macos` keys: into the existing section (its other keys,
/// such as `entitlements`, stay as they are) or as a new section before the
/// closing brace. Legacy top-level `sandbox` / `signing_identity` move into
/// the section.
fn writeMacosSection(allocator: std.mem.Allocator, io: Io, root: []const u8, cfg: *const AppConfig, add_sandbox: bool, add_signing: bool) !void {
    const path = try std.fs.path.join(allocator, &.{ root, "app.config.json" });
    const text = try readFile(allocator, io, path);
    var keys: std.ArrayListUnmanaged([]const u8) = .empty;
    if (add_sandbox or cfg.legacy_sandbox) try keys.append(allocator, try std.fmt.allocPrint(allocator, "\"sandbox\": {}", .{cfg.sandbox}));
    if (add_signing or cfg.legacy_signing) try keys.append(allocator, try std.fmt.allocPrint(allocator, "\"signing_identity\": \"{s}\"", .{try escapeJson(allocator, cfg.signing_identity)}));
    if (keys.items.len == 0) return;
    var out: []u8 = undefined;
    if (std.mem.indexOf(u8, text, "\"macos\"")) |start| {
        const open = std.mem.indexOfScalarPos(u8, text, start, '{') orelse fail("app.config.json 的 macos 段格式不对", .{});
        const rest = std.mem.trimStart(u8, text[open + 1 ..], " \t\r\n");
        const empty = rest.len > 0 and rest[0] == '}';
        var insert: std.ArrayListUnmanaged(u8) = .empty;
        for (keys.items, 0..) |key, idx| {
            try insert.appendSlice(allocator, "\n    ");
            try insert.appendSlice(allocator, key);
            if (!(empty and idx == keys.items.len - 1)) try insert.append(allocator, ',');
        }
        if (empty) {
            out = try std.mem.concat(allocator, u8, &.{ text[0 .. open + 1], insert.items, "\n  ", rest });
        } else {
            out = try std.mem.concat(allocator, u8, &.{ text[0 .. open + 1], insert.items, text[open + 1 ..] });
        }
    } else {
        const end = std.mem.lastIndexOfScalar(u8, text, '}') orelse fail("app.config.json 格式不对", .{});
        const body = std.mem.trimEnd(u8, text[0..end], " \t\r\n");
        const joined = try std.mem.join(allocator, ",\n    ", keys.items);
        out = try std.mem.concat(allocator, u8, &.{ body, ",\n  \"macos\": {\n    ", joined, "\n  }\n}\n" });
    }
    if (cfg.legacy_sandbox) out = try dropTopLevelKey(allocator, out, "sandbox");
    if (cfg.legacy_signing) out = try dropTopLevelKey(allocator, out, "signing_identity");
    try writeFile(allocator, io, path, out);
}

/// Removes a `"key": value,` line written by older `zant create` (the value
/// is a literal on one line); leaves the text unchanged if it is not there.
fn dropTopLevelKey(allocator: std.mem.Allocator, text: []u8, key: []const u8) ![]u8 {
    const needle = try std.fmt.allocPrint(allocator, "\n  \"{s}\":", .{key});
    const start = std.mem.indexOf(u8, text, needle) orelse return text;
    const line_end = std.mem.indexOfScalarPos(u8, text, start + 1, '\n') orelse return text;
    const line = std.mem.trim(u8, text[start..line_end], " \t\r\n");
    if (line.len == 0 or line[line.len - 1] != ',') return text;
    return std.mem.concat(allocator, u8, &.{ text[0..start], text[line_end..] });
}

pub fn escapeJson(allocator: std.mem.Allocator, text: []const u8) ![]u8 {
    var out: std.ArrayListUnmanaged(u8) = .empty;
    for (text) |c| switch (c) {
        '"' => try out.appendSlice(allocator, "\\\""),
        '\\' => try out.appendSlice(allocator, "\\\\"),
        else => try out.append(allocator, c),
    };
    return out.toOwnedSlice(allocator);
}

fn packageApp(allocator: std.mem.Allocator, io: Io, root: []const u8, cfg: *const AppConfig, release: bool) ![]u8 {
    try compileApp(allocator, io, root, release);
    if (is_windows) return packageWindows(allocator, io, root, cfg);
    const ent: ?[]const u8 = if (cfg.sandbox or cfg.entitlements != null)
        try std.fmt.allocPrint(allocator, "{s}/app.entitlements", .{root})
    else
        null;
    return buildMacBundle(allocator, io, root, cfg, .{ .identity = cfg.signing_identity, .entitlements = ent });
}

pub fn compileApp(allocator: std.mem.Allocator, io: Io, root: []const u8, release: bool) !void {
    const args = if (release)
        &[_][]const u8{ zigExe(), "build", "-Doptimize=ReleaseFast" }
    else
        &[_][]const u8{ zigExe(), "build" };
    step("{s}", .{if (release) "正在编译 ReleaseFast" else "正在编译"});
    try runInherit(allocator, io, args, root);
    ok("编译完成", .{});
}

pub const BundleOptions = struct {
    /// codesign identity; "none" = ad-hoc
    identity: []const u8 = "none",
    entitlements: ?[]const u8 = null,
    /// Contents/embedded.provisionprofile (Mac App Store)
    provisioning_profile: ?[]const u8 = null,
    /// a keychain to find the identity in (`zant build` with a CI certificate)
    keychain: ?[]const u8 = null,
    plist: PlistExtras = .{},
};

/// zig-out/<title>.app from the compiled zig-out/bin/app: icon, Info.plist,
/// signature (hardened runtime and a secure timestamp with a real identity).
pub fn buildMacBundle(allocator: std.mem.Allocator, io: Io, root: []const u8, cfg: *const AppConfig, opts: BundleOptions) ![]u8 {
    const app_name = cfg.title;
    if (std.mem.indexOfScalar(u8, app_name, '/') != null) fail("应用标题不能包含 '/'", .{});
    const app_path = try std.fmt.allocPrint(allocator, "{s}/zig-out/{s}.app", .{ root, app_name });
    Dir.cwd().deleteTree(io, app_path) catch {};
    const macos_path = try std.fmt.allocPrint(allocator, "{s}/Contents/MacOS", .{app_path});
    try Dir.cwd().createDirPath(io, macos_path);

    const src_exe = try std.fmt.allocPrint(allocator, "{s}/zig-out/bin/app", .{root});
    const dst_exe = try std.fmt.allocPrint(allocator, "{s}/Contents/MacOS/app", .{app_path});
    try Dir.cwd().copyFile(src_exe, std.Io.Dir.cwd(), dst_exe, io, .{ .replace = true, .make_path = true });

    var extras = opts.plist;
    extras.icon = try writeMacIcon(allocator, io, root, app_path);
    const plist = try generateInfoPlistFor(allocator, cfg, extras);
    try writeFile(allocator, io, try std.fmt.allocPrint(allocator, "{s}/Contents/Info.plist", .{app_path}), plist);
    if (opts.provisioning_profile) |profile| {
        const dst = try std.fmt.allocPrint(allocator, "{s}/Contents/embedded.provisionprofile", .{app_path});
        Dir.cwd().copyFile(profile, Dir.cwd(), dst, io, .{ .replace = true }) catch |err|
            fail("无法复制描述文件 {s}：{s}", .{ profile, @errorName(err) });
    }

    const identity = if (opts.identity.len == 0) "none" else opts.identity;
    var sign_argv: std.ArrayListUnmanaged([]const u8) = .empty;
    try sign_argv.appendSlice(allocator, &.{ "codesign", "--force", "--deep" });
    if (opts.entitlements) |ent| try sign_argv.appendSlice(allocator, &.{ "--entitlements", ent });
    if (opts.keychain) |kc| try sign_argv.appendSlice(allocator, &.{ "--keychain", kc });
    if (std.mem.eql(u8, identity, "none")) {
        try sign_argv.appendSlice(allocator, &.{ "--sign", "-" });
    } else {
        try sign_argv.appendSlice(allocator, &.{ "--options", "runtime", "--timestamp", "--sign", identity });
    }
    try sign_argv.append(allocator, app_path);
    step("正在签名（{s}）", .{if (std.mem.eql(u8, identity, "none")) "Ad-hoc" else identity});
    try runInherit(allocator, io, sign_argv.items, root);
    ok("已生成 {s}", .{app_path});
    return app_path;
}

/// `zig-out/bin/app` (`app.exe` on Windows).
/// Contents/Resources/AppIcon.icns from assets/icons/app-macos.png (1024×1024,
/// Apple icon grid): the ten iconset sizes are resized here, iconutil packs
/// them. Returns false when the project has no icon.
fn writeMacIcon(allocator: std.mem.Allocator, io: Io, root: []const u8, app_path: []const u8) !bool {
    const source = try std.fs.path.join(allocator, &.{ root, "assets", "icons", "app-macos.png" });
    const data = readFile(allocator, io, source) catch return false;
    const img = png.decode(allocator, data) catch |err| fail("{s} 无法读取（{s}）：需要 8 位、非隔行扫描的 PNG", .{ source, @errorName(err) });
    if (img.width != 1024 or img.height != 1024) fail("{s} 是 {d}×{d}；macOS 应用图标需要 1024×1024（苹果图标网格：主体 824×824 居中）", .{ source, img.width, img.height });
    step("生成应用图标 AppIcon.icns", .{});
    const iconset = try std.fmt.allocPrint(allocator, "{s}/zig-out/AppIcon.iconset", .{root});
    Dir.cwd().deleteTree(io, iconset) catch {};
    try Dir.cwd().createDirPath(io, iconset);
    for ([_]u32{ 16, 32, 128, 256, 512 }) |size| {
        for ([_]u32{ 1, 2 }) |scale| {
            const px = size * scale;
            const resized = if (px == img.width) img else try png.resize(allocator, img, px, px);
            const name = if (scale == 1)
                try std.fmt.allocPrint(allocator, "{s}/icon_{d}x{d}.png", .{ iconset, size, size })
            else
                try std.fmt.allocPrint(allocator, "{s}/icon_{d}x{d}@2x.png", .{ iconset, size, size });
            try writeFile(allocator, io, name, try png.encode(allocator, resized));
        }
    }
    const resources = try std.fmt.allocPrint(allocator, "{s}/Contents/Resources", .{app_path});
    try Dir.cwd().createDirPath(io, resources);
    const icns = try std.fmt.allocPrint(allocator, "{s}/AppIcon.icns", .{resources});
    try runInherit(allocator, io, &.{ "iconutil", "-c", "icns", "-o", icns, iconset }, root);
    Dir.cwd().deleteTree(io, iconset) catch {};
    ok("应用图标已生成", .{});
    return true;
}

fn appExePath(allocator: std.mem.Allocator, root: []const u8) ![]u8 {
    return std.fmt.allocPrint(allocator, "{s}/zig-out/bin/app{s}", .{ root, if (is_windows) ".exe" else "" });
}

/// Windows: a folder with the exe, zant_skia.dll, pdfium.dll and the MSVC
/// runtime the DLL needs, ready to zip or run anywhere on Windows 10+.
pub fn packageWindows(allocator: std.mem.Allocator, io: Io, root: []const u8, cfg: *const AppConfig) ![]u8 {
    for ([_]u8{ '/', '\\', ':', '*', '?', '"', '<', '>', '|' }) |bad| {
        if (std.mem.indexOfScalar(u8, cfg.title, bad) != null) fail("应用标题不能包含 {c}", .{bad});
    }
    const out = try std.fmt.allocPrint(allocator, "{s}/zig-out/{s}", .{ root, cfg.title });
    errdefer allocator.free(out);
    Dir.cwd().deleteTree(io, out) catch {};
    try Dir.cwd().createDirPath(io, out);
    const exe_name = try std.fmt.allocPrint(allocator, "{s}.exe", .{cfg.title});
    defer allocator.free(exe_name);
    const files = [_]struct { src: []const u8, dst: []const u8 }{
        .{ .src = "app.exe", .dst = exe_name },
        .{ .src = "zant_skia.dll", .dst = "zant_skia.dll" },
        .{ .src = "pdfium.dll", .dst = "pdfium.dll" },
    };
    for (files) |f| {
        const src = try std.fmt.allocPrint(allocator, "{s}/zig-out/bin/{s}", .{ root, f.src });
        defer allocator.free(src);
        const dst = try std.fs.path.join(allocator, &.{ out, f.dst });
        defer allocator.free(dst);
        Dir.cwd().copyFile(src, Dir.cwd(), dst, io, .{ .replace = true }) catch |err| {
            if (!std.mem.eql(u8, f.src, "pdfium.dll")) return err;
        };
    }
    // zant_skia.dll is built with /MD: ship the MSVC runtime next to it (the
    // copy installed with a prebuilt bridge, else the system's)
    const system_root = (if (child_environ) |env| env.get("SystemRoot") else null) orelse "C:\\Windows";
    for ([_][]const u8{ "vcruntime140.dll", "vcruntime140_1.dll", "msvcp140.dll" }) |name| {
        const local = try std.fmt.allocPrint(allocator, "{s}/zig-out/bin/{s}", .{ root, name });
        const src = if (fileExists(io, local)) local else try std.fmt.allocPrint(allocator, "{s}\\System32\\{s}", .{ system_root, name });
        defer allocator.free(src);
        const dst = try std.fs.path.join(allocator, &.{ out, name });
        defer allocator.free(dst);
        Dir.cwd().copyFile(src, Dir.cwd(), dst, io, .{ .replace = true }) catch {};
    }
    ok("已生成 {s}", .{out});
    return out;
}

fn validName(name: []const u8) bool {
    if (name.len == 0 or !std.ascii.isAlphabetic(name[0])) return false;
    for (name) |ch| {
        if (!(std.ascii.isAlphanumeric(ch) or ch == '-' or ch == '_')) return false;
    }
    return true;
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
fn bundleError(id: []const u8) ?[]const u8 {
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

pub fn pathExists(io: Io, path: []const u8) bool {
    if (Dir.cwd().openDir(io, path, .{})) |dir| {
        dir.close(io);
        return true;
    } else |_| return false;
}

fn cmdCreate(allocator: std.mem.Allocator, io: Io, args: []const [:0]const u8) !void {
    if (args.len < 3) fail("用法: zant create <项目名> [--yes] [--title ...] [--style ...] [--bundle-id ...] [--sandbox true|false] [--sign ...]", .{});
    const name = args[2];
    var assume_yes = false;
    var title_arg: ?[]const u8 = null;
    var style_arg: ?[]const u8 = null;
    var bundle_arg: ?[]const u8 = null;
    var sandbox_arg: ?bool = null;
    var sign_arg: ?[]const u8 = null;
    var force = false;
    var i: usize = 3;
    while (i < args.len) : (i += 1) {
        if (std.mem.eql(u8, args[i], "--yes")) assume_yes = true else if (std.mem.eql(u8, args[i], "--force")) force = true else if (std.mem.eql(u8, args[i], "--title") and i + 1 < args.len) {
            i += 1;
            title_arg = args[i];
        } else if (std.mem.eql(u8, args[i], "--style") and i + 1 < args.len) {
            i += 1;
            style_arg = args[i];
        } else if (std.mem.eql(u8, args[i], "--bundle-id") and i + 1 < args.len) {
            i += 1;
            bundle_arg = args[i];
        } else if (std.mem.eql(u8, args[i], "--sandbox") and i + 1 < args.len) {
            i += 1;
            sandbox_arg = std.mem.eql(u8, args[i], "true") or std.mem.eql(u8, args[i], "yes");
        } else if (std.mem.eql(u8, args[i], "--sign") and i + 1 < args.len) {
            i += 1;
            sign_arg = args[i];
        } else fail("未知参数: {s}", .{args[i]});
    }
    if (!validName(name)) fail("项目名只能以字母开头，并包含字母、数字、-、_", .{});

    const cwd = try std.process.currentPathAlloc(io, allocator);
    defer allocator.free(cwd);
    const target = try std.fs.path.join(allocator, &.{ cwd, name });
    defer allocator.free(target);
    if (pathExists(io, target) and !force) fail("目录已存在: {s}（使用 --force 覆盖）", .{target});

    const default_title = try titleFromName(allocator, name);
    defer allocator.free(default_title);
    const slug = try slugFromName(allocator, name);
    defer allocator.free(slug);
    const zig_name = try zigNameFromName(allocator, name);
    defer allocator.free(zig_name);
    const default_bundle = try std.fmt.allocPrint(allocator, "dev.zant.{s}", .{slug});
    defer allocator.free(default_bundle);

    var title: []const u8 = title_arg orelse default_title;
    var style: []const u8 = style_arg orelse "native";
    // app identifier (bundle_id): the macOS bundle id and the Windows data
    // folder name. macOS-only settings (sandbox, signing) are added by
    // `zant run` / `zant package` on a Mac, see completePlatformConfig.
    var bundle: []const u8 = bundle_arg orelse default_bundle;

    if (!assume_yes) {
        std.debug.print("\nZant UI 项目生成器\n\n", .{});
        if (title_arg == null) {
            const value = try prompt(allocator, "应用标题", title);
            title = value;
        }
        if (style_arg == null) {
            std.debug.print("\n窗口风格:\n  1) 原生标题栏\n  2) 隐藏标题栏（全尺寸内容 + 系统窗口按钮：macOS 左上角红黄绿，Windows 右上角最小化 / 最大化 / 关闭）\n  3) 无边框（movable_by_background 时可拖动背景移动；macOS Cmd+Q、Windows Alt+F4 退出）\n", .{});
            while (true) {
                const value = try prompt(allocator, "选择", "1");
                if (std.mem.eql(u8, value, "1") or std.mem.eql(u8, value, "native")) {
                    style = "native";
                    break;
                }
                if (std.mem.eql(u8, value, "2") or std.mem.eql(u8, value, "hidden-titlebar")) {
                    style = "hidden-titlebar";
                    break;
                }
                if (std.mem.eql(u8, value, "3") or std.mem.eql(u8, value, "borderless")) {
                    style = "borderless";
                    break;
                }
            }
        }
        if (bundle_arg == null) bundle = try askBundle(allocator, bundle);
    }
    if (std.mem.eql(u8, style, "2")) style = "hidden-titlebar";
    if (std.mem.eql(u8, style, "3")) style = "borderless";
    if (!std.mem.eql(u8, style, "native") and !std.mem.eql(u8, style, "hidden-titlebar") and !std.mem.eql(u8, style, "borderless"))
        fail("窗口风格必须是 native、hidden-titlebar 或 borderless", .{});
    if (title.len == 0) fail("应用标题不能为空", .{});
    if (bundleError(bundle)) |why| fail("应用标识 {s} 不符合规范：{s}", .{ bundle, why });

    // everything is validated: only now replace an existing folder (--force)
    if (pathExists(io, target)) Dir.cwd().deleteTree(io, target) catch {};
    step("创建 {s}", .{target});
    try Dir.cwd().createDirPath(io, target);
    ok("目录已创建", .{});
    try Dir.cwd().createDirPath(io, try std.fs.path.join(allocator, &.{ target, "src", "components" }));

    step("写入模板、配置和自定义组件", .{});
    const zroot = zant_root;
    const build_zig = try readFile(allocator, io, try std.fs.path.join(allocator, &.{ zroot, "scaffold/template/build.zig" }));
    defer allocator.free(build_zig);
    const build_zon = try readFile(allocator, io, try std.fs.path.join(allocator, &.{ zroot, "scaffold/template/build.zig.zon" }));
    defer allocator.free(build_zon);
    const main_zig = try readFile(allocator, io, try std.fs.path.join(allocator, &.{ zroot, "scaffold/template/src/main.zig" }));
    defer allocator.free(main_zig);
    const state_zig = try readFile(allocator, io, try std.fs.path.join(allocator, &.{ zroot, "scaffold/template/src/state.zig" }));
    defer allocator.free(state_zig);
    const stat_card = try readFile(allocator, io, try std.fs.path.join(allocator, &.{ zroot, "scaffold/template/src/components/stat_card.zig" }));
    defer allocator.free(stat_card);
    const counter_card = try readFile(allocator, io, try std.fs.path.join(allocator, &.{ zroot, "scaffold/template/src/components/counter_card.zig" }));
    defer allocator.free(counter_card);
    const dashboard = try readFile(allocator, io, try std.fs.path.join(allocator, &.{ zroot, "scaffold/template/src/components/dashboard.zig" }));
    defer allocator.free(dashboard);
    const readme = try readFile(allocator, io, try std.fs.path.join(allocator, &.{ zroot, "scaffold/template/README.md" }));
    defer allocator.free(readme);
    const agents = try readFile(allocator, io, try std.fs.path.join(allocator, &.{ zroot, "scaffold/template/AGENTS.md" }));
    defer allocator.free(agents);
    const claude = try readFile(allocator, io, try std.fs.path.join(allocator, &.{ zroot, "scaffold/template/CLAUDE.md" }));
    defer allocator.free(claude);
    const design = try readFile(allocator, io, try std.fs.path.join(allocator, &.{ zroot, "scaffold/template/DESIGN.md" }));
    defer allocator.free(design);

    const zant_rel = try zantDependencyPath(allocator, io, cwd, target, zroot);
    defer allocator.free(zant_rel);
    // a Zig string literal in build.zig.zon: "\\" would be an escape
    std.mem.replaceScalar(u8, zant_rel, '\\', '/');
    const fingerprint = (@as(u64, std.hash.Crc32.hash(zig_name)) << 32) | 0x9e3779b9;
    const zon_path = try std.fs.path.join(allocator, &.{ target, "build.zig.zon" });
    defer allocator.free(zon_path);
    const zon_text = try std.fmt.allocPrint(allocator, "{s}", .{build_zon});
    defer allocator.free(zon_text);
    var zon_owned = try allocator.dupe(u8, zon_text);
    defer allocator.free(zon_owned);
    zon_owned = try replace(allocator, zon_owned, "__APP_NAME_SNAKE__", zig_name);
    zon_owned = try replace(allocator, zon_owned, "__APP_FINGERPRINT__", try std.fmt.allocPrint(allocator, "0x{x:0>16}", .{fingerprint}));
    zon_owned = try replace(allocator, zon_owned, "__ZANT_PATH__", zant_rel);
    try writeFile(allocator, io, zon_path, zon_owned);

    const build_path = try std.fs.path.join(allocator, &.{ target, "build.zig" });
    defer allocator.free(build_path);
    try writeFile(allocator, io, build_path, build_zig);
    const main_path = try std.fs.path.join(allocator, &.{ target, "src", "main.zig" });
    defer allocator.free(main_path);
    const top_padding = if (std.mem.eql(u8, style, "hidden-titlebar")) "56" else "24";
    const main_text = try replace(allocator, try allocator.dupe(u8, main_zig), "__TOP_PADDING__", top_padding);
    try writeFile(allocator, io, main_path, main_text);
    const state_path = try std.fs.path.join(allocator, &.{ target, "src", "state.zig" });
    defer allocator.free(state_path);
    try writeFile(allocator, io, state_path, state_zig);
    const stat_path = try std.fs.path.join(allocator, &.{ target, "src", "components", "stat_card.zig" });
    defer allocator.free(stat_path);
    try writeFile(allocator, io, stat_path, stat_card);
    const counter_path = try std.fs.path.join(allocator, &.{ target, "src", "components", "counter_card.zig" });
    defer allocator.free(counter_path);
    try writeFile(allocator, io, counter_path, counter_card);
    const dashboard_path = try std.fs.path.join(allocator, &.{ target, "src", "components", "dashboard.zig" });
    defer allocator.free(dashboard_path);
    try writeFile(allocator, io, dashboard_path, dashboard);
    var agents_text = try allocator.dupe(u8, agents);
    agents_text = try replace(allocator, agents_text, "__APP_TITLE__", title);
    agents_text = try replace(allocator, agents_text, "__BUNDLE_ID__", bundle);
    const agents_path = try std.fs.path.join(allocator, &.{ target, "AGENTS.md" });
    defer allocator.free(agents_path);
    try writeFile(allocator, io, agents_path, agents_text);
    var claude_text = try allocator.dupe(u8, claude);
    claude_text = try replace(allocator, claude_text, "__APP_TITLE__", title);
    claude_text = try replace(allocator, claude_text, "__BUNDLE_ID__", bundle);
    const claude_path = try std.fs.path.join(allocator, &.{ target, "CLAUDE.md" });
    defer allocator.free(claude_path);
    try writeFile(allocator, io, claude_path, claude_text);
    try writeFile(allocator, io, try std.fs.path.join(allocator, &.{ target, ".gitignore" }),
        \\# build output and caches
        \\zig-out*/
        \\.zig-cache/
        \\# Zant UI link for projects on another drive (Windows)
        \\.zant/
        \\# generated by `zant` on macOS from app.config.json
        \\Info.plist
        \\app.entitlements
        \\
    );
    // default icons (both platforms' guidelines); replace the PNGs, the SVGs are editable sources
    try Dir.cwd().createDirPath(io, try std.fs.path.join(allocator, &.{ target, "assets", "icons" }));
    for ([_][]const u8{ "app-macos", "app-windows", "tray-dark", "tray-light", "statusbar" }) |icon| {
        for ([_][]const u8{ ".png", ".svg" }) |ext| {
            const file = try std.mem.concat(allocator, u8, &.{ icon, ext });
            const data = try readFile(allocator, io, try std.fs.path.join(allocator, &.{ zroot, "scaffold/template/assets/icons", file }));
            try writeFile(allocator, io, try std.fs.path.join(allocator, &.{ target, "assets", "icons", file }), data);
        }
    }
    // UI/UX design guide (highest-priority rules for AI agents)
    const design_path = try std.fs.path.join(allocator, &.{ target, "DESIGN.md" });
    defer allocator.free(design_path);
    try writeFile(allocator, io, design_path, design);

    const readme_text = try std.fmt.allocPrint(allocator, "{s}", .{readme});
    defer allocator.free(readme_text);
    var readme_owned = try allocator.dupe(u8, readme_text);
    defer allocator.free(readme_owned);
    readme_owned = try replace(allocator, readme_owned, "__APP_TITLE__", title);
    readme_owned = try replace(allocator, readme_owned, "__BUNDLE_ID__", bundle);
    const readme_path = try std.fs.path.join(allocator, &.{ target, "README.md" });
    defer allocator.free(readme_path);
    try writeFile(allocator, io, readme_path, readme_owned);

    // optional macOS section when --sandbox / --sign were given
    const macos_section = if (sandbox_arg != null or sign_arg != null)
        try std.fmt.allocPrint(allocator, ",\n  \"macos\": {{\n    \"sandbox\": {}{s}\n  }}", .{
            sandbox_arg orelse false,
            if (sign_arg) |id| try std.fmt.allocPrint(allocator, ",\n    \"signing_identity\": \"{s}\"", .{try escapeJson(allocator, id)}) else "",
        })
    else
        "";
    const config_json = try std.fmt.allocPrint(allocator,
        \\{{
        \\  "title": "{s}",
        \\  "bundle_id": "{s}",
        \\  "version": "0.1.0",
        \\  "window": {{
        \\    "style": "{s}",
        \\    "width": 680,
        \\    "height": 700,
        \\    "min_width": 600,
        \\    "min_height": 620,
        \\    "background": "#f5f5f5",
        \\    "resizable": true,
        \\    "always_on_top": false,
        \\    "movable_by_background": false,
        \\    "center": true
        \\  }}{s}
        \\}}
        \\
    , .{ try escapeJson(allocator, title), bundle, style, macos_section });
    defer allocator.free(config_json);
    const config_path = try std.fs.path.join(allocator, &.{ target, "app.config.json" });
    defer allocator.free(config_path);
    try writeFile(allocator, io, config_path, config_json);

    // platform files (Info.plist, entitlements) are generated by `zant run`
    const cfg = try loadConfig(allocator, io, target);
    try writeConfigSource(allocator, io, target, &cfg);
    ok("app.config.json / src/app_config.zig 已生成", .{});
    ok("项目已生成: {s}", .{target});
    std.debug.print("  " ++ Color.dim ++ "macOS 专属设置（沙盒、签名）在 Mac 上首次 zant run / zant package 时补充到 app.config.json" ++ Color.reset ++ "\n", .{});
    if (release_home != null) {
        std.debug.print("\n下一步：\n  cd {s}\n  zant run\n\n", .{name});
    } else {
        const sep = if (is_windows) "\\" else "/";
        std.debug.print("\n下一步：\n  cd {s}\n  {s}{s}zig-out{s}bin{s}zant run\n\n", .{ name, zroot, sep, sep, sep });
    }
}

/// Path of Zant UI for build.zig.zon, which only accepts paths relative to
/// the project. On Windows a project on another drive than Zant UI has no
/// relative path: link Zant UI into the project with a directory junction
/// (no admin rights needed) and refer to that.
fn zantDependencyPath(allocator: std.mem.Allocator, io: Io, cwd: []const u8, target: []const u8, zroot: []const u8) ![]u8 {
    const rel = try std.fs.path.relativeAlloc(allocator, cwd, null, target, zroot);
    if (!std.fs.path.isAbsolute(rel)) return rel;
    allocator.free(rel);
    const dir = try std.fs.path.join(allocator, &.{ target, ".zant" });
    try Dir.cwd().createDirPath(io, dir);
    const link = try std.fs.path.join(allocator, &.{ dir, "zant-ui" });
    const result = runCapture(allocator, io, &.{ "cmd", "/c", "mklink", "/J", link, zroot }) catch |err|
        fail("项目和 Zant UI 不在同一个盘，创建目录联接失败（{s}）；请把项目建在 {s} 所在的盘", .{ @errorName(err), zroot });
    if (result.term != .exited or result.term.exited != 0)
        fail("项目和 Zant UI 不在同一个盘，创建目录联接失败：{s}", .{result.stderr});
    ok("项目和 Zant UI 不在同一个盘：已用目录联接 .zant/zant-ui 引用 Zant UI", .{});
    return allocator.dupe(u8, ".zant/zant-ui");
}

fn replace(allocator: std.mem.Allocator, input: []u8, needle: []const u8, value: []const u8) ![]u8 {
    const index = std.mem.indexOf(u8, input, needle) orelse return input;
    var out: std.ArrayListUnmanaged(u8) = .empty;
    try out.appendSlice(allocator, input[0..index]);
    try out.appendSlice(allocator, value);
    try out.appendSlice(allocator, input[index + needle.len ..]);
    allocator.free(input);
    return out.toOwnedSlice(allocator);
}

fn titleFromName(allocator: std.mem.Allocator, name: []const u8) ![]u8 {
    const out = try allocator.dupe(u8, name);
    for (out) |*ch| {
        if (ch.* == '-' or ch.* == '_') ch.* = ' ';
    }
    var upper = true;
    for (out) |*ch| {
        if (ch.* == ' ') {
            upper = true;
            continue;
        }
        if (upper) {
            ch.* = std.ascii.toUpper(ch.*);
            upper = false;
        }
    }
    return out;
}

fn slugFromName(allocator: std.mem.Allocator, name: []const u8) ![]u8 {
    const out = try allocator.dupe(u8, name);
    for (out) |*ch| {
        ch.* = switch (ch.*) {
            'A'...'Z' => std.ascii.toLower(ch.*),
            '_' => '-',
            else => ch.*,
        };
    }
    return out;
}

fn zigNameFromName(allocator: std.mem.Allocator, name: []const u8) ![]u8 {
    const out = try allocator.dupe(u8, name);
    for (out) |*ch| {
        ch.* = switch (ch.*) {
            'A'...'Z' => std.ascii.toLower(ch.*),
            '-' => '_',
            else => ch.*,
        };
    }
    return out;
}

fn cmdSync(allocator: std.mem.Allocator, io: Io, root: []const u8) !void {
    var config = try loadConfig(allocator, io, root);
    try completePlatformConfig(allocator, io, root, &config, .run);
    try syncProject(allocator, io, root, &config);
}

fn cmdBuild(allocator: std.mem.Allocator, io: Io, release: bool) !void {
    const root = try std.process.currentPathAlloc(io, allocator);
    defer allocator.free(root);
    try cmdSync(allocator, io, root);
    const args = if (release) &[_][]const u8{ zigExe(), "build", "-Doptimize=ReleaseFast" } else &[_][]const u8{ zigExe(), "build" };
    try runInherit(allocator, io, args, root);
}

fn cmdRun(allocator: std.mem.Allocator, io: Io, release: bool) !void {
    const root = try std.process.currentPathAlloc(io, allocator);
    defer allocator.free(root);
    var config = try loadConfig(allocator, io, root);
    try completePlatformConfig(allocator, io, root, &config, .run);
    try syncProject(allocator, io, root, &config);
    if (config.sandbox and !is_windows) {
        const app_path = try packageApp(allocator, io, root, &config, true);
        defer allocator.free(app_path);
        // App Sandbox apps must be launched through LaunchServices, not by
        // executing the Mach-O inside the bundle directly.
        try runInherit(allocator, io, &.{ "open", "-n", "-W", app_path }, root);
    } else {
        const args = if (release) &[_][]const u8{ zigExe(), "build", "-Doptimize=ReleaseFast" } else &[_][]const u8{ zigExe(), "build" };
        try runInherit(allocator, io, args, root);
        const exe = try appExePath(allocator, root);
        defer allocator.free(exe);
        if (rawLogs() or is_windows) {
            try runInherit(allocator, io, &.{exe}, root);
        } else {
            const argv = appArgv(exe);
            try runInherit(allocator, io, &argv, root);
        }
    }
}

fn buildOnce(allocator: std.mem.Allocator, io: Io, root: []const u8, release: bool) !bool {
    _ = allocator;
    const args = if (release) &[_][]const u8{ zigExe(), "build", "-Doptimize=ReleaseFast" } else &[_][]const u8{ zigExe(), "build" };
    var child = std.process.spawn(io, .{
        .argv = args,
        .cwd = .{ .path = root },
        .stdin = .inherit,
        .stdout = .inherit,
        .stderr = .inherit,
        .create_no_window = false,
        .environ_map = child_environ,
    }) catch return false;
    const term = child.wait(io) catch return false;
    return term.success();
}

fn hashFileStamp(hash: *std.hash.Wyhash, io: Io, path: []const u8) void {
    const st = Dir.cwd().statFile(io, path, .{}) catch return;
    hash.update(path);
    hash.update(std.mem.asBytes(&st.mtime.nanoseconds));
    hash.update(std.mem.asBytes(&st.size));
}

fn hashTree(allocator: std.mem.Allocator, io: Io, hash: *std.hash.Wyhash, dir_path: []const u8, depth: usize) void {
    if (depth > 8) return;
    var dir = Dir.cwd().openDir(io, dir_path, .{ .iterate = true }) catch return;
    defer dir.close(io);
    var it = dir.iterate();
    while (it.next(io) catch null) |entry| {
        const path = std.fs.path.join(allocator, &.{ dir_path, entry.name }) catch continue;
        defer allocator.free(path);
        switch (entry.kind) {
            .directory => hashTree(allocator, io, hash, path, depth + 1),
            .file => if (std.mem.endsWith(u8, entry.name, ".zig")) hashFileStamp(hash, io, path),
            else => {},
        }
    }
}

/// Fingerprint of everything that affects the app build: the config files
/// and every `.zig` file under `src/` (any depth).
fn sourceStamp(allocator: std.mem.Allocator, io: Io, root: []const u8) !u64 {
    var hash = std.hash.Wyhash.init(0);
    const files = [_][]const u8{ "app.config.json", "build.zig", "build.zig.zon" };
    for (files) |rel| {
        const path = try std.fs.path.join(allocator, &.{ root, rel });
        defer allocator.free(path);
        hashFileStamp(&hash, io, path);
    }
    const src = try std.fs.path.join(allocator, &.{ root, "src" });
    defer allocator.free(src);
    hashTree(allocator, io, &hash, src, 0);
    return hash.final();
}

/// Harmless macOS framework log lines (InputMethodKit / Text Services
/// Manager) that unbundled apps launched from a terminal print when an input
/// method attaches. They do not affect input; `zant run` hides them unless
/// ZANT_RAW_LOGS=1.
const benign_log_filter =
    "exec \"$0\" \"$@\" 2> >(grep --line-buffered -v " ++
    "-e 'IMKCFRunLoopWakeUpReliable' -e 'IMKClient Stall detected' " ++
    "-e 'TSM AdjustCapsLockLEDForKeyTransitionHandling' >&2)";

/// argv that runs `exe` with benign system log lines filtered from stderr.
/// `exec` keeps the app's pid, so waiting / killing the child still works.
fn appArgv(exe: []const u8) [4][]const u8 {
    return .{ "/bin/bash", "-c", benign_log_filter, exe };
}

pub fn rawLogs() bool {
    const env = child_environ orelse return false;
    const value = env.get("ZANT_RAW_LOGS") orelse return false;
    return value.len > 0 and value[0] == '1';
}

fn spawnApp(io: Io, exe: []const u8, root: []const u8) !std.process.Child {
    const filtered = appArgv(exe);
    const plain = [_][]const u8{exe};
    return std.process.spawn(io, .{
        .argv = if (rawLogs() or is_windows) &plain else &filtered,
        .cwd = .{ .path = root },
        .stdin = .inherit,
        .stdout = .inherit,
        .stderr = .inherit,
        .create_no_window = false,
        .environ_map = child_environ,
    });
}

fn cmdWatch(allocator: std.mem.Allocator, io: Io, release: bool) !void {
    const root = try std.process.currentPathAlloc(io, allocator);
    defer allocator.free(root);
    var config = try loadConfig(allocator, io, root);
    try completePlatformConfig(allocator, io, root, &config, .run);
    if (config.sandbox and !is_windows) fail("App Sandbox 下暂不支持热更新；请使用 zant run", .{});
    try syncProject(allocator, io, root, &config);
    step("首次编译", .{});
    if (!try buildOnce(allocator, io, root, release)) fail("编译失败", .{});
    ok("编译完成", .{});

    var last = try sourceStamp(allocator, io, root);
    const exe_path = try appExePath(allocator, root);
    defer allocator.free(exe_path);
    var child = try spawnApp(io, exe_path, root);
    std.debug.print("\n热更新已开启：监听 src/、app.config.json；Ctrl+C 退出\n", .{});

    while (true) {
        std.Io.sleep(io, .fromMilliseconds(400), .awake) catch {};
        const now = try sourceStamp(allocator, io, root);
        if (now == last) continue;
        last = now;
        std.debug.print("\n检测到改动，重新编译...\n", .{});
        if (is_windows) {
            // a running .exe cannot be overwritten: stop it before linking,
            // and bring the last good build back if this one fails
            child.kill(io);
            const built = try buildOnce(allocator, io, root, release);
            if (!built) std.debug.print("编译失败，重新打开上一次的版本，等待下一次改动\n", .{});
            try syncProject(allocator, io, root, &config);
            child = try spawnApp(io, exe_path, root);
            if (built) std.debug.print("已重启应用\n", .{});
            continue;
        }
        if (!try buildOnce(allocator, io, root, release)) {
            std.debug.print("编译失败，保持当前窗口，等待下一次改动\n", .{});
            continue;
        }
        child.kill(io);
        try syncProject(allocator, io, root, &config);
        child = try spawnApp(io, exe_path, root);
        std.debug.print("已重启应用\n", .{});
    }
}

fn cmdPackage(allocator: std.mem.Allocator, io: Io) !void {
    const root = try std.process.currentPathAlloc(io, allocator);
    defer allocator.free(root);
    var config = try loadConfig(allocator, io, root);
    try completePlatformConfig(allocator, io, root, &config, .package);
    try syncProject(allocator, io, root, &config);
    const app_path = try packageApp(allocator, io, root, &config, true);
    defer allocator.free(app_path);
    std.debug.print("\n输出: {s}\n", .{app_path});
}

/// Zant UI's folder. An installed release (install.sh / install.ps1) keeps
/// it as `zant-ui` next to `bin/zant`; a zant built from a checkout uses
/// that checkout.
pub var zant_root: []const u8 = build_options.zant_root;
/// The install folder (`~/.zant`, `%LOCALAPPDATA%\zant`) of a release install.
pub var release_home: ?[]const u8 = null;

fn findRoot(allocator: std.mem.Allocator, io: Io) void {
    const dir = std.process.executableDirPathAlloc(io, allocator) catch return;
    const home = std.fs.path.dirname(dir) orelse return;
    const root = std.fs.path.join(allocator, &.{ home, "zant-ui" }) catch return;
    const zon = std.fs.path.join(allocator, &.{ root, "build.zig.zon" }) catch return;
    if (!fileExists(io, zon)) return;
    zant_root = root;
    release_home = home;
}

/// The Zig to build apps with: $ZANT_ZIG; the Zig that built this zant;
/// the one the installer downloaded (`<home>/zig/<version>`); else `zig`
/// from PATH.
pub fn zigExe() []const u8 {
    const io = io_global orelse return "zig";
    if (child_environ) |env| if (env.get("ZANT_ZIG")) |z| if (z.len > 0) return z;
    if (std.fs.path.isAbsolute(build_options.zig_exe) and fileExists(io, build_options.zig_exe)) return build_options.zig_exe;
    if (release_home) |home| {
        const a = std.heap.page_allocator;
        const zon = Dir.cwd().readFileAlloc(io, std.fs.path.join(a, &.{ zant_root, "build.zig.zon" }) catch return "zig", a, .limited(64 * 1024)) catch return "zig";
        if (zonString(zon, ".minimum_zig_version")) |version| {
            const exe = std.fs.path.join(a, &.{ home, "zig", version, if (is_windows) "zig.exe" else "zig" }) catch return "zig";
            if (fileExists(io, exe)) return exe;
        }
    }
    return "zig";
}
var io_global: ?Io = null;

/// `zant update`: pulls Zant UI (a clean checkout that tracks a remote,
/// fast-forward only), fetches the pinned dependencies again when they
/// changed, checks the Zig version and rebuilds the CLI.
fn cmdUpdate(allocator: std.mem.Allocator, io: Io) !void {
    if (release_home) |home| return updateRelease(allocator, io, home);
    const root = zant_root;
    std.debug.print("\n" ++ Color.cyan ++ "更新 Zant UI" ++ Color.reset ++ "：{s}\n", .{root});

    const git_dir = try std.fs.path.join(allocator, &.{ root, ".git" });
    if (pathExists(io, git_dir) or fileExists(io, git_dir)) {
        const before = gitOutput(allocator, io, root, &.{ "rev-parse", "--short", "HEAD" });
        const dirty = gitOutput(allocator, io, root, &.{ "status", "--porcelain", "--untracked-files=no" });
        const upstream = gitOutput(allocator, io, root, &.{ "rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}" });
        if (dirty.len > 0) {
            std.debug.print("  " ++ Color.dim ++ "本地有未提交的修改，不拉取新代码，只重新编译" ++ Color.reset ++ "\n", .{});
        } else if (upstream.len == 0) {
            std.debug.print("  " ++ Color.dim ++ "当前分支没有对应的远程分支，不拉取新代码，只重新编译" ++ Color.reset ++ "\n", .{});
        } else {
            step("拉取最新代码（{s}）", .{upstream});
            const pulled = runOk(io, &.{ "git", "-C", root, "pull", "--ff-only", "--quiet" }, root);
            const after = gitOutput(allocator, io, root, &.{ "rev-parse", "--short", "HEAD" });
            if (!pulled) {
                std.debug.print("  " ++ Color.dim ++ "拉取失败（本地有远程没有的提交，或网络问题），用现有代码重新编译" ++ Color.reset ++ "\n", .{});
            } else if (std.mem.eql(u8, before, after)) {
                ok("已经是最新（{s}）", .{after});
            } else {
                ok("已更新：{s} → {s}", .{ before, after });
            }
        }
    } else {
        std.debug.print("  " ++ Color.dim ++ "不是 git 仓库，不拉取新代码，只重新编译" ++ Color.reset ++ "\n", .{});
    }

    // Zig version the framework needs (build.zig.zon) vs the one zant uses
    const zon = readFile(allocator, io, try std.fs.path.join(allocator, &.{ root, "build.zig.zon" })) catch "";
    const needed = zonString(zon, ".minimum_zig_version") orelse "";
    const zig = zigExe();
    const have = blk: {
        const res = runCapture(allocator, io, &.{ zig, "version" }) catch break :blk "";
        break :blk std.mem.trim(u8, res.stdout, " \r\n");
    };
    if (needed.len > 0 and !std.mem.eql(u8, needed, have)) {
        fail("Zant UI 现在需要 Zig {s}（zant 用的是 {s}）：重新运行安装脚本即可（见 Zant UI README「安装」），它会下载对应版本的 Zig", .{ needed, if (have.len > 0) have else "找不到的 Zig" });
    }

    // pinned Skia / PDFium / fonts: the scripts download only what changed
    step("检查依赖（Skia 等）", .{});
    if (is_windows) {
        try runInherit(allocator, io, &.{ "powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", try std.fs.path.join(allocator, &.{ root, "scripts", "fetch_skia_windows.ps1" }) }, root);
    } else {
        try runInherit(allocator, io, &.{ "/bin/bash", try std.fs.path.join(allocator, &.{ root, "scripts", "fetch_skia.sh" }) }, root);
    }

    step("重新编译 zant", .{});
    const exe = try std.fmt.allocPrint(allocator, "{s}/zig-out/bin/zant{s}", .{ root, if (is_windows) ".exe" else "" });
    const old = try std.fmt.allocPrint(allocator, "{s}/zig-out/bin/zant.old.exe", .{root});
    if (is_windows) {
        // a running .exe cannot be overwritten, but it can be renamed
        Dir.cwd().deleteFile(io, old) catch {};
        Dir.cwd().rename(exe, Dir.cwd(), old, io) catch {};
    }
    if (!runOk(io, &.{ zig, "build", "cli" }, root)) {
        if (is_windows) Dir.cwd().rename(old, Dir.cwd(), exe, io) catch {};
        fail("编译失败，zant 保持原来的版本", .{});
    }
    ok("已更新：{s}", .{exe});
}

/// An installed release updates by running the install script of the
/// latest release again (it checks the version, Zig and replaces
/// `zant-ui/` and `bin/zant`); ZANT_DOWNLOAD points it elsewhere.
fn updateRelease(allocator: std.mem.Allocator, io: Io, home: []const u8) !void {
    const base = (if (child_environ) |env| env.get("ZANT_DOWNLOAD") else null) orelse release_download;
    try child_environ.?.put("ZANT_HOME", home);
    const script = if (is_windows)
        try std.fmt.allocPrint(allocator, "irm '{s}/install.ps1' | iex", .{base})
    else
        try std.fmt.allocPrint(allocator, "curl -fsSL '{s}/install.sh' | bash", .{base});
    const argv: []const []const u8 = if (is_windows)
        &.{ "powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script }
    else
        &.{ "/bin/bash", "-c", script };
    if (!runOk(io, argv, home)) fail("更新没有完成（见上面的说明）", .{});
}

/// Where release archives and the install scripts are published.
pub const release_download = "https://github.com/racall/zant-ui/releases/latest/download";

/// Runs a command with the terminal; false when it fails (no exit).
fn runOk(io: Io, argv: []const []const u8, cwd: []const u8) bool {
    var child = std.process.spawn(io, .{
        .argv = argv,
        .cwd = .{ .path = cwd },
        .stdin = .inherit,
        .stdout = .inherit,
        .stderr = .inherit,
        .create_no_window = false,
        .environ_map = child_environ,
    }) catch return false;
    const term = child.wait(io) catch return false;
    return term.success();
}

/// Trimmed stdout of a git command in `root`, "" on failure.
fn gitOutput(allocator: std.mem.Allocator, io: Io, root: []const u8, args: []const []const u8) []const u8 {
    var argv: std.ArrayListUnmanaged([]const u8) = .empty;
    argv.appendSlice(allocator, &.{ "git", "-C", root }) catch return "";
    argv.appendSlice(allocator, args) catch return "";
    const res = runCapture(allocator, io, argv.items) catch return "";
    if (res.term != .exited or res.term.exited != 0) return "";
    return std.mem.trim(u8, res.stdout, " \r\n");
}

fn fileExists(io: Io, path: []const u8) bool {
    Dir.cwd().access(io, path, .{}) catch return false;
    return true;
}

/// The string value of `.field = "value"` in a .zon file.
fn zonString(text: []const u8, field: []const u8) ?[]const u8 {
    const at = std.mem.indexOf(u8, text, field) orelse return null;
    const open = std.mem.indexOfScalarPos(u8, text, at, '"') orelse return null;
    const close = std.mem.indexOfScalarPos(u8, text, open + 1, '"') orelse return null;
    return text[open + 1 .. close];
}

test "zon string field" {
    try std.testing.expectEqualStrings("0.17.1", zonString(".{ .name = .x, .minimum_zig_version = \"0.17.1\", }", ".minimum_zig_version").?);
    try std.testing.expect(zonString(".{}", ".minimum_zig_version") == null);
}

fn hasArg(args: []const [:0]const u8, needle: []const u8) bool {
    for (args) |arg| if (std.mem.eql(u8, arg, needle)) return true;
    return false;
}

pub fn main(init: std.process.Init) !void {
    var arena = std.heap.ArenaAllocator.init(init.gpa);
    defer arena.deinit();
    const allocator = arena.allocator();
    const io = init.io;
    io_global = io;
    findRoot(allocator, io);
    console.init();
    defer console.deinit();
    child_environ = init.environ_map;
    if (init.environ_map.get("ZIG_GLOBAL_CACHE_DIR") == null) {
        const cache = if (release_home) |home|
            try std.fs.path.join(allocator, &.{ home, "cache" })
        else
            try std.fmt.allocPrint(allocator, "{s}/.zig-global-cache", .{zant_root});
        try init.environ_map.put("ZIG_GLOBAL_CACHE_DIR", cache);
    }
    const args = try init.minimal.args.toSlice(allocator);
    if (args.len < 2) {
        std.debug.print(
            \\用法: zant <命令>
            \\  create <项目名>       生成项目
            \\  run [--watch]         编译并运行（--watch 热更新）
            \\  compile [--release]   只编译
            \\  package               生成本机测试用的 .app / 程序目录
            \\  build [配置文件]      发布构建：签名、公证、DMG / 安装包，或提交商店
            \\                        （默认读 build.config.json；--yes 不提问，CI 用）
            \\  sync                  重新生成配置代码
            \\  update                更新 Zant UI（git pull、依赖）并重新编译 zant
            \\
        , .{});
        return;
    }
    if (std.mem.eql(u8, args[1], "create")) {
        try cmdCreate(allocator, io, args);
    } else if (std.mem.eql(u8, args[1], "run")) {
        if (hasArg(args, "--watch")) {
            try cmdWatch(allocator, io, hasArg(args, "--release"));
        } else {
            try cmdRun(allocator, io, hasArg(args, "--release"));
        }
    } else if (std.mem.eql(u8, args[1], "build")) {
        try distribute.cmdBuild(allocator, io, args);
    } else if (std.mem.eql(u8, args[1], "compile")) {
        try cmdBuild(allocator, io, hasArg(args, "--release"));
    } else if (std.mem.eql(u8, args[1], "package")) {
        try cmdPackage(allocator, io);
    } else if (std.mem.eql(u8, args[1], "sync")) {
        const root = try std.process.currentPathAlloc(io, allocator);
        defer allocator.free(root);
        try cmdSync(allocator, io, root);
    } else if (std.mem.eql(u8, args[1], "update")) {
        try cmdUpdate(allocator, io);
    } else {
        std.debug.print("未知命令: {s}\n", .{args[1]});
    }
}

test {
    _ = distribute;
}
