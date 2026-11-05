const std = @import("std");
const build_options = @import("build_options");
const Io = std.Io;
const Dir = Io.Dir;

var child_environ: ?*std.process.Environ.Map = null;

const Color = struct {
    const cyan = "\x1b[36m";
    const green = "\x1b[32m";
    const red = "\x1b[31m";
    const dim = "\x1b[2m";
    const reset = "\x1b[0m";
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

const AppConfig = struct {
    title: []const u8 = "Zant App",
    bundle_id: []const u8 = "dev.zant.app",
    version: []const u8 = "0.1.0",
    sandbox: bool = false,
    signing_identity: []const u8 = "none",
    window: WindowConfig = .{},
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

const RawConfig = struct {
    title: ?[]const u8 = null,
    bundle_id: ?[]const u8 = null,
    version: ?[]const u8 = null,
    sandbox: ?bool = null,
    signing_identity: ?[]const u8 = null,
    window: ?RawWindow = null,
};

fn step(comptime fmt: []const u8, args: anytype) void {
    std.debug.print("  " ++ Color.cyan ++ "→" ++ Color.reset ++ " " ++ fmt ++ "\n", args);
}

fn ok(comptime fmt: []const u8, args: anytype) void {
    std.debug.print("  " ++ Color.green ++ "✓" ++ Color.reset ++ " " ++ fmt ++ "\n", args);
}

fn fail(comptime fmt: []const u8, args: anytype) noreturn {
    std.debug.print("\n  " ++ Color.red ++ "✗" ++ Color.reset ++ " " ++ fmt ++ "\n\n", args);
    std.process.exit(1);
}

fn writeFile(allocator: std.mem.Allocator, io: Io, path: []const u8, data: []const u8) !void {
    if (std.fs.path.dirname(path)) |parent| {
        try Dir.cwd().createDirPath(io, parent);
    }
    try Dir.cwd().writeFile(io, .{ .sub_path = path, .data = data });
    _ = allocator;
}

fn readFile(allocator: std.mem.Allocator, io: Io, path: []const u8) ![]u8 {
    return Dir.cwd().readFileAlloc(io, path, allocator, .limited(8 * 1024 * 1024));
}

fn runInherit(allocator: std.mem.Allocator, io: Io, argv: []const []const u8, cwd: []const u8) !void {
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

fn runCapture(allocator: std.mem.Allocator, io: Io, argv: []const []const u8) !std.process.RunResult {
    return std.process.run(allocator, io, .{
        .argv = argv,
        .stdout_limit = .limited(1024 * 1024),
        .stderr_limit = .limited(1024 * 1024),
        .reserve_amount = 4096,
        .environ_map = child_environ,
    });
}

fn prompt(allocator: std.mem.Allocator, label: []const u8, default: []const u8) ![]u8 {
    std.debug.print("{s} [{s}]: ", .{ label, default });
    var buf: [1024]u8 = undefined;
    var n: usize = 0;
    while (n < buf.len) {
        const r = std.posix.read(std.posix.STDIN_FILENO, buf[n .. n + 1]) catch 0;
        if (r == 0 or buf[n] == '\n') break;
        n += 1;
    }
    const value = std.mem.trimEnd(u8, buf[0..n], "\r");
    return allocator.dupe(u8, if (value.len == 0) default else value);
}

fn promptYesNo(allocator: std.mem.Allocator, label: []const u8, default: bool) !bool {
    const def = if (default) "Y/n" else "y/N";
    while (true) {
        std.debug.print("{s} [{s}]: ", .{ label, def });
        var buf: [16]u8 = undefined;
        var n: usize = 0;
        while (n < buf.len) {
            const r = std.posix.read(std.posix.STDIN_FILENO, buf[n .. n + 1]) catch 0;
            if (r == 0 or buf[n] == '\n') break;
            n += 1;
        }
        const v = std.mem.trim(u8, buf[0..n], " \r\n");
        if (v.len == 0) return default;
        if (std.ascii.eqlIgnoreCase(v, "y") or std.ascii.eqlIgnoreCase(v, "yes")) return true;
        if (std.ascii.eqlIgnoreCase(v, "n") or std.ascii.eqlIgnoreCase(v, "no")) return false;
        _ = allocator;
    }
}

fn listIdentities(allocator: std.mem.Allocator, io: Io) ![]const []const u8 {
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

fn escapeXml(allocator: std.mem.Allocator, text: []const u8) ![]u8 {
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

fn loadConfig(allocator: std.mem.Allocator, io: Io, root: []const u8) !AppConfig {
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
    if (parsed.sandbox) |v| cfg.sandbox = v;
    if (parsed.signing_identity) |v| cfg.signing_identity = v;
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
    const title = try escapeXml(allocator, cfg.title);
    defer allocator.free(title);
    const bundle = try escapeXml(allocator, cfg.bundle_id);
    defer allocator.free(bundle);
    return std.fmt.allocPrint(allocator,
        \\<?xml version="1.0" encoding="UTF-8"?>
        \\<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
        \\<plist version="1.0"><dict>
        \\<key>CFBundleDevelopmentRegion</key><string>zh_CN</string>
        \\<key>CFBundleDisplayName</key><string>{s}</string>
        \\<key>CFBundleExecutable</key><string>app</string>
        \\<key>CFBundleIdentifier</key><string>{s}</string>
        \\<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
        \\<key>CFBundleName</key><string>{s}</string>
        \\<key>CFBundlePackageType</key><string>APPL</string>
        \\<key>CFBundleShortVersionString</key><string>{s}</string>
        \\<key>CFBundleVersion</key><string>1</string>
        \\<key>LSMinimumSystemVersion</key><string>13.0</string>
        \\<key>NSHighResolutionCapable</key><true/>
        \\<key>NSPrincipalClass</key><string>NSApplication</string>
        \\</dict></plist>
        \\
    , .{ title, bundle, title, cfg.version });
}

fn syncProject(allocator: std.mem.Allocator, io: Io, root: []const u8, cfg: *const AppConfig) !void {
    const config_source = try generateConfigSource(allocator, cfg);
    defer allocator.free(config_source);
    const config_path = try std.fs.path.join(allocator, &.{ root, "src", "app_config.zig" });
    defer allocator.free(config_path);
    try writeFile(allocator, io, config_path, config_source);

    const plist = try generateInfoPlist(allocator, cfg);
    defer allocator.free(plist);
    const plist_path = try std.fs.path.join(allocator, &.{ root, "Info.plist" });
    defer allocator.free(plist_path);
    try writeFile(allocator, io, plist_path, plist);

    const ent_path = try std.fs.path.join(allocator, &.{ root, "app.entitlements" });
    defer allocator.free(ent_path);
    if (cfg.sandbox) {
        try writeFile(allocator, io, ent_path,
            \\<?xml version="1.0" encoding="UTF-8"?>
            \\<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
            \\<plist version="1.0"><dict>
            \\<key>com.apple.security.app-sandbox</key><true/>
            \\</dict></plist>
            \\
        );
    }
}

fn packageApp(allocator: std.mem.Allocator, io: Io, root: []const u8, cfg: *const AppConfig, release: bool) ![]u8 {
    const args = if (release)
        &[_][]const u8{ "zig", "build", "-Doptimize=ReleaseFast" }
    else
        &[_][]const u8{ "zig", "build" };
    step("{s}", .{if (release) "正在编译 ReleaseFast" else "正在编译"});
    try runInherit(allocator, io, args, root);
    ok("编译完成", .{});

    const app_name = cfg.title;
    if (std.mem.indexOfScalar(u8, app_name, '/') != null) fail("应用标题不能包含 '/'", .{});
    const app_path = try std.fmt.allocPrint(allocator, "{s}/zig-out/{s}.app", .{ root, app_name });
    errdefer allocator.free(app_path);
    Dir.cwd().deleteTree(io, app_path) catch {};
    const macos_path = try std.fmt.allocPrint(allocator, "{s}/Contents/MacOS", .{app_path});
    defer allocator.free(macos_path);
    try Dir.cwd().createDirPath(io, macos_path);

    const src_exe = try std.fmt.allocPrint(allocator, "{s}/zig-out/bin/app", .{root});
    defer allocator.free(src_exe);
    const dst_exe = try std.fmt.allocPrint(allocator, "{s}/Contents/MacOS/app", .{app_path});
    defer allocator.free(dst_exe);
    try Dir.cwd().copyFile(src_exe, std.Io.Dir.cwd(), dst_exe, io, .{ .replace = true, .make_path = true });

    const plist = try generateInfoPlist(allocator, cfg);
    defer allocator.free(plist);
    const dst_plist = try std.fmt.allocPrint(allocator, "{s}/Contents/Info.plist", .{app_path});
    defer allocator.free(dst_plist);
    try writeFile(allocator, io, dst_plist, plist);

    const identity = if (cfg.signing_identity.len == 0) "none" else cfg.signing_identity;
    var sign_argv: std.ArrayListUnmanaged([]const u8) = .empty;
    defer sign_argv.deinit(allocator);
    try sign_argv.appendSlice(allocator, &.{ "codesign", "--force", "--deep" });
    const ent_path: ?[]const u8 = if (cfg.sandbox)
        try std.fmt.allocPrint(allocator, "{s}/app.entitlements", .{root})
    else
        null;
    defer if (ent_path) |ent| allocator.free(ent);
    if (ent_path) |ent| {
        try sign_argv.appendSlice(allocator, &.{ "--entitlements", ent });
    }
    if (std.mem.eql(u8, identity, "none")) {
        try sign_argv.appendSlice(allocator, &.{ "--sign", "-" });
    } else {
        try sign_argv.appendSlice(allocator, &.{ "--options", "runtime", "--timestamp", "--sign", identity });
    }
    try sign_argv.append(allocator, app_path);
    step("正在签名（{s}）", .{identity});
    try runInherit(allocator, io, sign_argv.items, root);
    ok("已生成 {s}", .{app_path});
    return app_path;
}

fn validName(name: []const u8) bool {
    if (name.len == 0 or !std.ascii.isAlphabetic(name[0])) return false;
    for (name) |ch| {
        if (!(std.ascii.isAlphanumeric(ch) or ch == '-' or ch == '_')) return false;
    }
    return true;
}

fn validBundle(bundle: []const u8) bool {
    if (bundle.len == 0 or !std.ascii.isAlphabetic(bundle[0])) return false;
    for (bundle) |ch| {
        if (!(std.ascii.isAlphanumeric(ch) or ch == '-' or ch == '.')) return false;
    }
    return true;
}

fn pathExists(io: Io, path: []const u8) bool {
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
    if (pathExists(io, target)) {
        if (!force) fail("目录已存在: {s}（使用 --force 覆盖）", .{target});
        Dir.cwd().deleteTree(io, target) catch {};
    }

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
    var bundle: []const u8 = bundle_arg orelse default_bundle;
    var sandbox = sandbox_arg orelse false;
    var identity: []const u8 = sign_arg orelse "none";

    if (!assume_yes) {
        std.debug.print("\nZant UI 项目生成器\n\n", .{});
        if (title_arg == null) {
            const value = try prompt(allocator, "应用标题", title);
            title = value;
        }
        if (style_arg == null) {
            std.debug.print("\n窗口风格:\n  1) 原生标题栏\n  2) 隐藏标题栏（全尺寸内容 + 红黄绿按钮）\n  3) 无边框（可拖拽移动，Cmd+Q 退出）\n", .{});
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
        if (bundle_arg == null) {
            const value = try prompt(allocator, "Apple Bundle ID", bundle);
            bundle = value;
        }
        if (sandbox_arg == null) sandbox = try promptYesNo(allocator, "是否启用 App Sandbox", false);
        if (sign_arg == null) {
            const identities = try listIdentities(allocator, io);
            std.debug.print("\n打包开发者账号:\n  0) Ad-hoc（无开发者账号）\n", .{});
            for (identities, 0..) |id, idx| std.debug.print("  {d}) {s}\n", .{ idx + 1, id });
            const value = try prompt(allocator, "选择", "0");
            if (!std.mem.eql(u8, value, "0") and value.len > 0) {
                if (std.fmt.parseInt(usize, value, 10)) |idx| {
                    if (idx >= 1 and idx <= identities.len) identity = identities[idx - 1];
                } else |_| {
                    identity = value;
                }
            }
        }
    }

    if (std.mem.eql(u8, style, "2")) style = "hidden-titlebar";
    if (std.mem.eql(u8, style, "3")) style = "borderless";
    if (!std.mem.eql(u8, style, "native") and !std.mem.eql(u8, style, "hidden-titlebar") and !std.mem.eql(u8, style, "borderless"))
        fail("窗口风格必须是 native、hidden-titlebar 或 borderless", .{});
    if (title.len == 0) fail("应用标题不能为空", .{});
    if (!validBundle(bundle)) fail("Apple Bundle ID 无效: {s}", .{bundle});

    step("创建 {s}", .{target});
    try Dir.cwd().createDirPath(io, target);
    ok("目录已创建", .{});
    try Dir.cwd().createDirPath(io, try std.fs.path.join(allocator, &.{ target, "src", "components" }));

    step("写入模板、配置和自定义组件", .{});
    const zroot = build_options.zant_root;
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

    const zant_rel = try std.fs.path.relativeAlloc(allocator, cwd, null, target, zroot);
    defer allocator.free(zant_rel);
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
    // UI/UX design guide (highest-priority rules for AI agents)
    const design_path = try std.fs.path.join(allocator, &.{ target, "DESIGN.md" });
    defer allocator.free(design_path);
    try writeFile(allocator, io, design_path, design);

    const sandbox_text = if (sandbox) "true" else "false";
    const sign_display = if (std.mem.eql(u8, identity, "none")) "Ad-hoc（无开发者账号）" else identity;
    const readme_text = try std.fmt.allocPrint(allocator, "{s}", .{readme});
    defer allocator.free(readme_text);
    var readme_owned = try allocator.dupe(u8, readme_text);
    defer allocator.free(readme_owned);
    readme_owned = try replace(allocator, readme_owned, "__APP_TITLE__", title);
    readme_owned = try replace(allocator, readme_owned, "__BUNDLE_ID__", bundle);
    readme_owned = try replace(allocator, readme_owned, "__SANDBOX__", sandbox_text);
    readme_owned = try replace(allocator, readme_owned, "__SIGN_IDENTITY__", sign_display);
    const readme_path = try std.fs.path.join(allocator, &.{ target, "README.md" });
    defer allocator.free(readme_path);
    try writeFile(allocator, io, readme_path, readme_owned);

    const config_json = try std.fmt.allocPrint(allocator,
        \\{{
        \\  "title": "{s}",
        \\  "bundle_id": "{s}",
        \\  "version": "0.1.0",
        \\  "sandbox": {},
        \\  "signing_identity": "{s}",
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
        \\  }}
        \\}}
        \\
    , .{ title, bundle, sandbox, identity, style });
    defer allocator.free(config_json);
    const config_path = try std.fs.path.join(allocator, &.{ target, "app.config.json" });
    defer allocator.free(config_path);
    try writeFile(allocator, io, config_path, config_json);

    var cfg = AppConfig{
        .title = title,
        .bundle_id = bundle,
        .version = "0.1.0",
        .sandbox = sandbox,
        .signing_identity = identity,
        .window = .{ .style = style },
    };
    try syncProject(allocator, io, target, &cfg);
    ok("app.config.json / app_config.zig / Info.plist 已生成", .{});
    ok("项目已生成: {s}", .{target});
    std.debug.print("\n下一步：\n  cd {s}\n  {s}/zig-out/bin/zant run\n\n", .{ name, zroot });
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
    const config = try loadConfig(allocator, io, root);
    try syncProject(allocator, io, root, &config);
}

fn cmdBuild(allocator: std.mem.Allocator, io: Io, release: bool) !void {
    const root = try std.process.currentPathAlloc(io, allocator);
    defer allocator.free(root);
    try cmdSync(allocator, io, root);
    const args = if (release) &[_][]const u8{ "zig", "build", "-Doptimize=ReleaseFast" } else &[_][]const u8{ "zig", "build" };
    try runInherit(allocator, io, args, root);
}

fn cmdRun(allocator: std.mem.Allocator, io: Io, release: bool) !void {
    const root = try std.process.currentPathAlloc(io, allocator);
    defer allocator.free(root);
    const config = try loadConfig(allocator, io, root);
    try syncProject(allocator, io, root, &config);
    if (config.sandbox) {
        const app_path = try packageApp(allocator, io, root, &config, true);
        defer allocator.free(app_path);
        // App Sandbox apps must be launched through LaunchServices, not by
        // executing the Mach-O inside the bundle directly.
        try runInherit(allocator, io, &.{ "open", "-n", "-W", app_path }, root);
    } else {
        const args = if (release) &[_][]const u8{ "zig", "build", "-Doptimize=ReleaseFast" } else &[_][]const u8{ "zig", "build" };
        try runInherit(allocator, io, args, root);
        const exe = try std.fmt.allocPrint(allocator, "{s}/zig-out/bin/app", .{root});
        defer allocator.free(exe);
        if (rawLogs()) {
            try runInherit(allocator, io, &.{exe}, root);
        } else {
            const argv = appArgv(exe);
            try runInherit(allocator, io, &argv, root);
        }
    }
}

fn buildOnce(allocator: std.mem.Allocator, io: Io, root: []const u8, release: bool) !bool {
    _ = allocator;
    const args = if (release) &[_][]const u8{ "zig", "build", "-Doptimize=ReleaseFast" } else &[_][]const u8{ "zig", "build" };
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

fn rawLogs() bool {
    const value = std.c.getenv("ZANT_RAW_LOGS") orelse return false;
    return value[0] == '1';
}

fn spawnApp(io: Io, exe: []const u8, root: []const u8) !std.process.Child {
    const filtered = appArgv(exe);
    const plain = [_][]const u8{exe};
    return std.process.spawn(io, .{
        .argv = if (rawLogs()) &plain else &filtered,
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
    const config = try loadConfig(allocator, io, root);
    if (config.sandbox) fail("App Sandbox 下暂不支持热更新；请使用 zant run", .{});
    try syncProject(allocator, io, root, &config);
    step("首次编译", .{});
    if (!try buildOnce(allocator, io, root, release)) fail("编译失败", .{});
    ok("编译完成", .{});

    var last = try sourceStamp(allocator, io, root);
    const exe_path = try std.fmt.allocPrint(allocator, "{s}/zig-out/bin/app", .{root});
    defer allocator.free(exe_path);
    var child = try spawnApp(io, exe_path, root);
    std.debug.print("\n热更新已开启：监听 src/、app.config.json；Ctrl+C 退出\n", .{});

    while (true) {
        std.Io.sleep(io, .fromMilliseconds(400), .awake) catch {};
        const now = try sourceStamp(allocator, io, root);
        if (now == last) continue;
        last = now;
        std.debug.print("\n检测到改动，重新编译...\n", .{});
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
    const config = try loadConfig(allocator, io, root);
    try syncProject(allocator, io, root, &config);
    const app_path = try packageApp(allocator, io, root, &config, true);
    defer allocator.free(app_path);
    std.debug.print("\n输出: {s}\n", .{app_path});
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
    child_environ = init.environ_map;
    if (init.environ_map.get("ZIG_GLOBAL_CACHE_DIR") == null) {
        const cache = try std.fmt.allocPrint(allocator, "{s}/.zig-global-cache", .{build_options.zant_root});
        try init.environ_map.put("ZIG_GLOBAL_CACHE_DIR", cache);
    }
    const args = try init.minimal.args.toSlice(allocator);
    if (args.len < 2) {
        std.debug.print("用法: zant <create|run|build|package|update> ...\n  zant run --watch   热更新（重启式）\n", .{});
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
        try cmdBuild(allocator, io, hasArg(args, "--release"));
    } else if (std.mem.eql(u8, args[1], "package")) {
        try cmdPackage(allocator, io);
    } else if (std.mem.eql(u8, args[1], "sync")) {
        const root = try std.process.currentPathAlloc(io, allocator);
        defer allocator.free(root);
        try cmdSync(allocator, io, root);
    } else if (std.mem.eql(u8, args[1], "update")) {
        step("重新编译 Zant UI 全局 CLI", .{});
        try runInherit(allocator, io, &.{ "zig", "build" }, build_options.zant_root);
        ok("已更新：{s}/zig-out/bin/zant", .{build_options.zant_root});
    } else {
        std.debug.print("未知命令: {s}\n", .{args[1]});
    }
}
