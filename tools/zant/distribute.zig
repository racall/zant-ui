//! `zant build [配置文件]`: the release build. zant reads the build settings
//! (build.config.json by default, see docs/build.md), asks in a terminal for
//! what is missing (and offers to save the answers), compiles, builds and
//! signs the app, then hands the platform steps to the scripts in
//! tools/zant/package/ (macos.sh: DMG, notarization, App Store; windows.ps1:
//! signing, installer, MSIX, Microsoft Store) through ZANT_* environment
//! variables.
//!
//! Secrets are never written to the config file: a value `$NAME` (or
//! `${NAME}`) reads the environment variable NAME, notarization credentials
//! can live in the keychain (notarytool profiles), and passwords typed in the
//! terminal are used for this run only.
const std = @import("std");
const m = @import("main.zig");
const png = @import("png");
const Io = std.Io;
const Dir = Io.Dir;
const Allocator = std.mem.Allocator;
const is_windows = m.is_windows;
const step = m.step;
const ok = m.ok;
const fail = m.fail;
const eql = std.mem.eql;

pub const default_file = "build.config.json";

/// build.config.json. Every field is optional; a missing one is asked in a
/// terminal or (with --yes / in CI) uses its default or stops with a message
/// naming the key.
pub const BuildFile = struct {
    /// CFBundleVersion: App Store Connect needs a new one for every upload of
    /// the same version (default: `version` from app.config.json)
    build_number: ?[]const u8 = null,
    macos: ?Macos = null,
    windows: ?Windows = null,
    /// API key for App Store uploads (and optionally notarization)
    app_store_connect: ?AscKey = null,
};

pub const Macos = struct {
    /// "developer-id" (DMG, notarized) | "app-store"
    distribution: ?[]const u8 = null,
    /// codesign identity as `security find-identity` shows it; "none" = ad-hoc
    signing_identity: ?[]const u8 = null,
    /// CI: a .p12 (path, or base64 contents) imported into a temporary keychain
    certificate: ?[]const u8 = null,
    certificate_password: ?[]const u8 = null,
    /// developer-id: "dmg" (default) | "zip"
    format: ?[]const u8 = null,
    /// DMG window background: a PNG (its size is the window size in points,
    /// 640×400 recommended; `name@2x.png` next to it is used on Retina
    /// screens), "default" (tools/zant/package/dmg-background.png) or
    /// "none" (plain window)
    dmg_background: ?[]const u8 = null,
    notarize: ?bool = null,
    notary: ?Notary = null,
    /// app-store: LSApplicationCategoryType
    category: ?[]const u8 = null,
    /// app-store: "3rd Party Mac Developer Installer: …" / "Mac Installer Distribution: …"
    installer_identity: ?[]const u8 = null,
    /// app-store: the Mac App Store provisioning profile (.provisionprofile)
    provisioning_profile: ?[]const u8 = null,
    /// app-store: upload the .pkg to App Store Connect
    upload: ?bool = null,
};

/// One of: a notarytool keychain profile, the App Store Connect API key, or
/// an Apple ID with an app-specific password.
pub const Notary = struct {
    profile: ?[]const u8 = null,
    api_key: ?bool = null,
    apple_id: ?[]const u8 = null,
    password: ?[]const u8 = null,
    team_id: ?[]const u8 = null,
};

pub const AscKey = struct {
    issuer_id: ?[]const u8 = null,
    key_id: ?[]const u8 = null,
    /// the .p8 file: a path, or its contents (CI secret)
    key: ?[]const u8 = null,
};

pub const Windows = struct {
    /// "installer" (Inno Setup) | "store" (MSIX) | "folder" (zip)
    distribution: ?[]const u8 = null,
    /// shown by the installer and in Settings → Apps
    publisher: ?[]const u8 = null,
    /// a certificate in the user's certificate store: thumbprint or subject
    /// name; "none" = not signed
    signing_identity: ?[]const u8 = null,
    /// a .pfx (path, or base64 contents) instead of the certificate store
    certificate: ?[]const u8 = null,
    certificate_password: ?[]const u8 = null,
    timestamp_url: ?[]const u8 = null,
    store: ?Store = null,
};

/// Values from Partner Center → the app → Product management → Product identity.
pub const Store = struct {
    identity_name: ?[]const u8 = null,
    publisher: ?[]const u8 = null,
    publisher_display_name: ?[]const u8 = null,
    /// submit with the Microsoft Store Developer CLI (msstore)
    upload: ?bool = null,
    product_id: ?[]const u8 = null,
    tenant_id: ?[]const u8 = null,
    seller_id: ?[]const u8 = null,
    client_id: ?[]const u8 = null,
    client_secret: ?[]const u8 = null,
};

const Ctx = struct {
    a: Allocator,
    io: Io,
    root: []const u8,
    cfg: m.AppConfig,
    file: BuildFile = .{},
    path: []const u8,
    exists: bool,
    interactive: bool,
    changed: bool = false,
    saved: bool = false,
    /// ZANT_* variables for the scripts
    env: std.process.Environ.Map,
    asc: ?Asc = null,
};

const Asc = struct { issuer: []const u8, id: []const u8, path: []const u8 };

// cleanup on exit and on `fail` (temporary key files, the CI keychain)
var g_ctx: ?*Ctx = null;
var temp_files: std.ArrayListUnmanaged([]const u8) = .empty;
var temp_keychain: ?[]const u8 = null;

fn cleanup() void {
    const c = g_ctx orelse return;
    for (temp_files.items) |path| Dir.cwd().deleteFile(c.io, path) catch {};
    temp_files.clearRetainingCapacity();
    if (temp_keychain) |kc| {
        temp_keychain = null;
        var env = c.env.clone(c.a) catch return;
        env.put("ZANT_KEYCHAIN", kc) catch return;
        _ = runScriptEnv(c, &env, "remove-keychain", false);
    }
}

pub fn cmdBuild(a: Allocator, io: Io, args: []const [:0]const u8) !void {
    var file_arg: ?[]const u8 = null;
    var yes = false;
    for (args[2..]) |arg| {
        if (eql(u8, arg, "--yes") or eql(u8, arg, "-y")) {
            yes = true;
        } else if (eql(u8, arg, "--release")) {
            fail("zant build 现在是发布构建（签名、打包、公证或提交商店）；只编译请用 zant compile [--release]", .{});
        } else if (arg.len > 0 and arg[0] == '-') {
            fail("未知参数：{s}（用法：zant build [配置文件] [--yes]）", .{arg});
        } else if (file_arg == null) {
            file_arg = arg;
        } else fail("只能指定一个配置文件（用法：zant build [配置文件] [--yes]）", .{});
    }

    const root = try std.process.currentPathAlloc(io, a);
    var cfg = try m.loadConfig(a, io, root);
    try m.completePlatformConfig(a, io, root, &cfg, .run);
    try m.syncProject(a, io, root, &cfg);

    const path = if (file_arg) |f|
        (if (std.fs.path.isAbsolute(f)) f else try std.fs.path.join(a, &.{ root, f }))
    else
        try std.fs.path.join(a, &.{ root, default_file });
    const data: ?[]u8 = m.readFile(a, io, path) catch null;
    if (data == null and file_arg != null) fail("找不到配置文件 {s}", .{path});

    const ci = if (m.child_environ) |e| e.get("CI") != null else false;
    var c = Ctx{
        .a = a,
        .io = io,
        .root = root,
        .cfg = cfg,
        .path = path,
        .exists = data != null,
        .interactive = !yes and !ci and try std.Io.File.stdin().isTty(io),
        .env = if (m.child_environ) |e| try e.clone(a) else std.process.Environ.Map.init(a),
    };
    if (data) |text| {
        c.file = std.json.parseFromSliceLeaky(BuildFile, a, text, .{}) catch |err|
            fail("{s} 解析失败：{s}（未知的字段名也会报错，检查拼写；格式见 Zant UI 的 docs/build.md）", .{ path, @errorName(err) });
    }
    g_ctx = &c;
    m.on_fail = cleanup;
    defer {
        cleanup();
        m.on_fail = null;
    }

    std.debug.print("\n" ++ m.Color.cyan ++ "Zant 发布构建" ++ m.Color.reset ++ "（{s}）\n", .{if (is_windows) "Windows" else "macOS"});
    if (c.exists) {
        std.debug.print("  配置文件：{s}\n", .{path});
    } else {
        std.debug.print("  没有 {s}：{s}\n", .{ std.fs.path.basename(path), if (c.interactive) "缺的设置会逐项询问，最后可以保存" else "使用默认设置" });
    }
    if (!c.interactive) std.debug.print("  非交互模式（--yes / CI）：不提问，缺少必需的设置时停止\n", .{});

    if (is_windows) try windowsFlow(&c) else try macFlow(&c);
    save(&c);
}

// ---- values, questions, saving ----

fn envName(v: []const u8) ?[]const u8 {
    if (v.len < 2 or v[0] != '$') return null;
    const name = v[1..];
    if (name.len > 2 and name[0] == '{' and name[name.len - 1] == '}') return name[1 .. name.len - 1];
    return name;
}

fn getEnv(c: *Ctx, name: []const u8) ?[]const u8 {
    const v = c.env.get(name) orelse return null;
    return if (v.len == 0) null else v;
}

/// A config value: the literal, or `$NAME` from the environment.
fn value(c: *Ctx, v: []const u8, key: []const u8) []const u8 {
    if (envName(v)) |name| return getEnv(c, name) orelse fail("{s} 是环境变量 {s}，但它没有设置", .{ key, name });
    return v;
}

/// Like `value`, but an unset variable is typed (without echo) in a
/// terminal, for this run only.
fn secret(c: *Ctx, v: []const u8, key: []const u8, label: []const u8) ![]const u8 {
    if (envName(v)) |name| {
        if (getEnv(c, name)) |s| return s;
        if (!c.interactive) fail("{s} 是环境变量 {s}，但它没有设置", .{ key, name });
        std.debug.print("{s}（不显示、不保存；也可以设置环境变量 {s}）: ", .{ label, name });
        var buf: [512]u8 = undefined;
        return c.a.dupe(u8, m.console.readSecret(&buf));
    }
    return v;
}

fn need(c: *Ctx, key: []const u8, what: []const u8) noreturn {
    fail("{s} 缺少 {s}（{s}）。在终端里运行 zant build 按提示选择，或写进配置文件", .{ std.fs.path.basename(c.path), key, what });
}

fn askRequired(c: *Ctx, label: []const u8, default: []const u8) ![]const u8 {
    while (true) {
        const v = if (default.len > 0) try m.prompt(c.a, label, default) else blk: {
            std.debug.print("{s}: ", .{label});
            var buf: [1024]u8 = undefined;
            break :blk try c.a.dupe(u8, std.mem.trim(u8, m.console.readLine(&buf), " "));
        };
        if (v.len > 0) return v;
    }
}

/// Numbered choice; returns the index.
fn choose(c: *Ctx, title: []const u8, options: []const []const u8, default: usize) !usize {
    std.debug.print("\n{s}:\n", .{title});
    for (options, 0..) |opt, i| std.debug.print("  {d}) {s}\n", .{ i + 1, opt });
    const def = try std.fmt.allocPrint(c.a, "{d}", .{default + 1});
    while (true) {
        const v = try m.prompt(c.a, "选择", def);
        const n = std.fmt.parseInt(usize, v, 10) catch continue;
        if (n >= 1 and n <= options.len) return n - 1;
    }
}

fn expandPath(c: *Ctx, p: []const u8) ![]const u8 {
    if (p.len >= 2 and p[0] == '~' and (p[1] == '/' or p[1] == '\\')) {
        const home = getEnv(c, if (is_windows) "USERPROFILE" else "HOME") orelse return p;
        return std.fs.path.join(c.a, &.{ home, p[2..] });
    }
    if (std.fs.path.isAbsolute(p)) return p;
    return std.fs.path.join(c.a, &.{ c.root, p });
}

fn fileExists(c: *Ctx, p: []const u8) bool {
    Dir.cwd().access(c.io, p, .{}) catch return false;
    return true;
}

fn inProject(c: *Ctx, p: []const u8) bool {
    return std.mem.startsWith(u8, p, c.root) and p.len > c.root.len and (p[c.root.len] == '/' or p[c.root.len] == '\\');
}

/// Writes a secret (key file contents from an environment variable) to
/// zig-out/build, readable only by the user; deleted when zant exits.
fn writeSecretFile(c: *Ctx, name: []const u8, data: []const u8) ![]const u8 {
    const dir = try std.fs.path.join(c.a, &.{ c.root, "zig-out", "build" });
    try Dir.cwd().createDirPath(c.io, dir);
    const path = try std.fs.path.join(c.a, &.{ dir, name });
    try temp_files.append(c.a, path);
    try Dir.cwd().writeFile(c.io, .{
        .sub_path = path,
        .data = data,
        .flags = .{ .permissions = if (is_windows) .default_file else .fromMode(0o600) },
    });
    return path;
}

/// A file given as a path or as base64 contents (CI secrets).
fn fileOrBase64(c: *Ctx, v: []const u8, key: []const u8, name: []const u8) ![]const u8 {
    const p = try expandPath(c, v);
    if (fileExists(c, p)) return p;
    const decoder = std.base64.standard.decoderWithIgnore(" \t\r\n");
    const buf = try c.a.alloc(u8, decoder.calcSizeUpperBound(v.len));
    const n = decoder.decode(buf, v) catch fail("{s}：既不是存在的文件，也不是 base64 内容", .{key});
    return writeSecretFile(c, name, buf[0..n]);
}

/// Offers once to write the answers to the config file (no passwords or key
/// contents are ever in it: only names, options and `$VARIABLE` references).
fn save(c: *Ctx) void {
    if (!c.changed or c.saved or !c.interactive) return;
    c.saved = true;
    std.debug.print("\n", .{});
    const label = std.fmt.allocPrint(c.a, "把这些设置保存到 {s}？下次直接使用；文件里只有证书名称和选项，不含密码和密钥内容", .{std.fs.path.basename(c.path)}) catch return;
    if (!(m.promptYesNo(c.a, label, true) catch false)) return;
    const text = std.json.Stringify.valueAlloc(c.a, c.file, .{ .whitespace = .indent_2, .emit_null_optional_fields = false }) catch return;
    m.writeFile(c.a, c.io, c.path, std.mem.concat(c.a, u8, &.{ text, "\n" }) catch return) catch |err| {
        std.debug.print("  保存失败：{s}\n", .{@errorName(err)});
        return;
    };
    ok("已保存 {s}", .{c.path});
}

// ---- scripts ----

fn scriptPath(c: *Ctx) ![]const u8 {
    return std.fs.path.join(c.a, &.{ m.zant_root, "tools", "zant", "package", if (is_windows) "windows.ps1" else "macos.sh" });
}

fn scriptArgv(c: *Ctx, action: []const u8) ![]const []const u8 {
    const script = try scriptPath(c);
    if (is_windows) return c.a.dupe([]const u8, &.{ "powershell", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", script, action });
    return c.a.dupe([]const u8, &.{ "/bin/bash", script, action });
}

fn runScriptEnv(c: *Ctx, env: *const std.process.Environ.Map, action: []const u8, must: bool) bool {
    const argv = scriptArgv(c, action) catch return false;
    var child = std.process.spawn(c.io, .{
        .argv = argv,
        .cwd = .{ .path = c.root },
        .stdin = .inherit,
        .stdout = .inherit,
        .stderr = .inherit,
        .create_no_window = false,
        .environ_map = env,
    }) catch |err| {
        if (must) fail("无法运行打包脚本：{s}", .{@errorName(err)});
        return false;
    };
    const term = child.wait(c.io) catch return false;
    if (!term.success()) {
        if (must) fail("打包没有完成（见上面的说明）", .{});
        return false;
    }
    return true;
}

fn runScript(c: *Ctx, action: []const u8) void {
    _ = runScriptEnv(c, &c.env, action, true);
}

fn captureScript(c: *Ctx, action: []const u8) ![]const u8 {
    const res = std.process.run(c.a, c.io, .{
        .argv = try scriptArgv(c, action),
        .cwd = .{ .path = c.root },
        .environ_map = &c.env,
        .stdout_limit = .limited(1 << 20),
        .stderr_limit = .limited(1 << 20),
    }) catch return "";
    return std.mem.trim(u8, res.stdout, " \t\r\n");
}

fn setEnv(c: *Ctx, key: []const u8, v: []const u8) !void {
    try c.env.put(key, v);
}

fn buildNumber(c: *Ctx) ?[]const u8 {
    return if (c.file.build_number) |b| value(c, b, "build_number") else null;
}

// ---- macOS ----

fn macFlow(c: *Ctx) !void {
    if (c.file.macos == null) c.file.macos = .{};
    const mac = &c.file.macos.?;
    if (mac.distribution == null) {
        if (c.interactive) {
            const i = try choose(c, "发布方式（macOS）", &.{
                "自己分发：Developer ID 签名 → DMG → Apple 公证（推荐）",
                "Mac App Store：Apple Distribution 签名 → .pkg → 提交 App Store Connect",
            }, 0);
            mac.distribution = if (i == 0) "developer-id" else "app-store";
            c.changed = true;
        } else mac.distribution = "developer-id";
    }
    const dist = value(c, mac.distribution.?, "macos.distribution");
    const store = eql(u8, dist, "app-store");
    if (!store and !eql(u8, dist, "developer-id")) fail("macos.distribution 只能是 developer-id 或 app-store（现在是 {s}）", .{dist});

    // CI: the certificate goes into a temporary keychain
    var keychain: ?[]const u8 = null;
    if (mac.certificate) |cert| {
        const kc = try std.fs.path.join(c.a, &.{ c.root, "zig-out", "build", "zant-signing.keychain-db" });
        try Dir.cwd().createDirPath(c.io, std.fs.path.dirname(kc).?);
        try setEnv(c, "ZANT_CERT", try fileOrBase64(c, value(c, cert, "macos.certificate"), "macos.certificate", "signing.p12"));
        try setEnv(c, "ZANT_CERT_PASSWORD", if (mac.certificate_password) |p| try secret(c, p, "macos.certificate_password", "证书密码") else "");
        try setEnv(c, "ZANT_KEYCHAIN", kc);
        temp_keychain = kc;
        runScript(c, "import-cert");
        keychain = kc;
    }

    if (mac.signing_identity == null) {
        if (keychain) |kc| {
            mac.signing_identity = try firstIdentity(c, kc, store);
        } else if (c.interactive) {
            mac.signing_identity = try askIdentity(c, store);
            c.changed = true;
        } else need(c, "macos.signing_identity", "签名证书，如 Developer ID Application: 名字 (TEAMID)");
    }
    const identity = value(c, mac.signing_identity.?, "macos.signing_identity");
    const adhoc = eql(u8, identity, "none");
    if (store and adhoc) fail("Mac App Store 需要 Apple Distribution 证书，不能用 Ad-hoc 签名", .{});
    if (!adhoc and keychain == null) try checkIdentity(c, identity, "macos.signing_identity", false);

    // App Store: every question before the long build
    var profile: ?[]const u8 = null;
    var team: ?[]const u8 = null;
    var category: ?[]const u8 = if (mac.category) |v| value(c, v, "macos.category") else null;
    if (store) {
        if (category == null) {
            if (!c.interactive) need(c, "macos.category", "App Store 分类，如 public.app-category.utilities");
            std.debug.print("\nApp Store 分类（LSApplicationCategoryType），常用：utilities 工具、productivity 效率、developer-tools 开发者工具、graphics-design 图形和设计\n", .{});
            const v = try m.prompt(c.a, "分类", "public.app-category.utilities");
            mac.category = if (std.mem.startsWith(u8, v, "public.app-category.")) v else try std.fmt.allocPrint(c.a, "public.app-category.{s}", .{v});
            category = mac.category;
            c.changed = true;
        }
        if (mac.provisioning_profile == null) {
            if (!c.interactive) need(c, "macos.provisioning_profile", "Mac App Store 描述文件（.provisionprofile）的路径");
            mac.provisioning_profile = try askProfile(c);
            c.changed = true;
        }
        profile = try expandPath(c, value(c, mac.provisioning_profile.?, "macos.provisioning_profile"));
        if (!fileExists(c, profile.?)) fail("找不到描述文件 {s}", .{profile.?});
        team = (try profileInfo(c, profile.?)).team orelse teamOf(identity) orelse fail("无法从描述文件或证书得到团队 ID", .{});
        if (mac.installer_identity == null) {
            if (keychain) |kc| {
                mac.installer_identity = try firstInstaller(c, kc);
            } else if (c.interactive) {
                mac.installer_identity = try askInstaller(c);
                c.changed = true;
            } else need(c, "macos.installer_identity", "安装包证书，如 3rd Party Mac Developer Installer: 名字 (TEAMID)");
        }
        if (keychain == null) try checkIdentity(c, value(c, mac.installer_identity.?, "macos.installer_identity"), "macos.installer_identity", true);
        if (mac.upload == null) {
            if (c.interactive) {
                std.debug.print("\n", .{});
                mac.upload = try m.promptYesNo(c.a, "打包后提交到 App Store Connect？", true);
                c.changed = true;
            } else mac.upload = false;
        }
        if (mac.upload.?) _ = try ascKey(c);
        save(c);
        if (!c.cfg.sandbox) std.debug.print("  " ++ m.Color.dim ++ "Mac App Store 要求 App Sandbox：这次自动开启。上架前在 app.config.json 设 macos.sandbox: true，用 zant run 检查沙盒里各功能是否正常" ++ m.Color.reset ++ "\n", .{});
    }

    // the signed app
    const build_dir = try std.fs.path.join(c.a, &.{ c.root, "zig-out", "build" });
    try Dir.cwd().createDirPath(c.io, build_dir);
    var ent: ?[]const u8 = null;
    if (store or c.cfg.sandbox or c.cfg.entitlements != null) {
        ent = try std.fs.path.join(c.a, &.{ build_dir, "app.entitlements" });
        try m.writeFile(c.a, c.io, ent.?, try m.entitlementsPlist(c.a, &c.cfg, if (store) team else null));
    }
    std.debug.print("\n", .{});
    try m.compileApp(c.a, c.io, c.root, true);
    const app = try m.buildMacBundle(c.a, c.io, c.root, &c.cfg, .{
        .identity = identity,
        .entitlements = ent,
        .provisioning_profile = profile,
        .keychain = keychain,
        .plist = .{ .build_number = buildNumber(c), .category = category },
    });
    if (!adhoc) {
        try m.runInherit(c.a, c.io, &.{ "codesign", "--verify", "--strict", "--deep", app }, c.root);
        ok("签名有效", .{});
    }

    const out = try std.fs.path.join(c.a, &.{ c.root, "zig-out", "dist" });
    try setEnv(c, "ZANT_APP", app);
    try setEnv(c, "ZANT_TITLE", c.cfg.title);
    try setEnv(c, "ZANT_VERSION", c.cfg.version);
    try setEnv(c, "ZANT_OUT", out);
    try setEnv(c, "ZANT_IDENTITY", identity);
    if (store) {
        try setEnv(c, "ZANT_INSTALLER_IDENTITY", value(c, mac.installer_identity.?, "macos.installer_identity"));
        try setEnv(c, "ZANT_UPLOAD", if (mac.upload.?) "1" else "0");
        if (mac.upload.?) {
            const k = try ascKey(c);
            try setEnv(c, "ZANT_ASC_KEY", k.path);
            try setEnv(c, "ZANT_ASC_KEY_ID", k.id);
            try setEnv(c, "ZANT_ASC_ISSUER", k.issuer);
        }
        runScript(c, "app-store");
        printResult(c, try std.fmt.allocPrint(c.a, "{s}/{s}-{s}.pkg", .{ out, c.cfg.title, c.cfg.version }));
        return;
    }

    const format = if (mac.format) |f| value(c, f, "macos.format") else "dmg";
    if (!eql(u8, format, "dmg") and !eql(u8, format, "zip")) fail("macos.format 只能是 dmg 或 zip（现在是 {s}）", .{format});
    const can_notarize = std.mem.startsWith(u8, identity, "Developer ID Application");
    if (mac.notarize == null) {
        if (!can_notarize) {
            std.debug.print("  " ++ m.Color.dim ++ "不是 Developer ID Application 证书，跳过公证（只有它签名的应用能公证）" ++ m.Color.reset ++ "\n", .{});
            mac.notarize = false;
        } else if (c.interactive) {
            std.debug.print("\n", .{});
            mac.notarize = try m.promptYesNo(c.a, "签名完成。要提交 Apple 公证吗？（公证后别人下载打开时不会被系统拦截，需要几分钟）", true);
            c.changed = true;
        } else {
            std.debug.print("  " ++ m.Color.dim ++ "没有设置 macos.notarize，跳过公证" ++ m.Color.reset ++ "\n", .{});
            mac.notarize = false;
        }
    }
    if (eql(u8, format, "dmg")) try setEnv(c, "ZANT_DMG_BACKGROUND", try dmgBackground(c, mac));
    if (mac.notarize.? and !can_notarize) fail("只有 Developer ID Application 证书签名的应用能公证（现在是 {s}）", .{identity});
    if (mac.notarize.?) try notaryEnv(c, mac, identity);
    save(c);

    try setEnv(c, "ZANT_FORMAT", format);
    try setEnv(c, "ZANT_NOTARIZE", if (mac.notarize.?) "1" else "0");
    if (keychain) |kc| try setEnv(c, "ZANT_KEYCHAIN", kc);
    runScript(c, "developer-id");
    printResult(c, try std.fmt.allocPrint(c.a, "{s}/{s}-{s}.{s}", .{ out, c.cfg.title, c.cfg.version, format }));
}

/// The DMG background PNG for the script ("" = plain window); asks once.
fn dmgBackground(c: *Ctx, mac: *Macos) ![]const u8 {
    if (mac.dmg_background == null) {
        if (c.interactive) {
            std.debug.print("\n", .{});
            while (true) {
                const v = try m.prompt(c.a, "DMG 背景图片（PNG，建议 640×400，同目录可放 @2x 版本；回车用默认背景，none 不用背景）", "默认");
                if (eql(u8, v, "默认") or eql(u8, v, "default")) {
                    mac.dmg_background = "default";
                } else if (eql(u8, v, "none")) {
                    mac.dmg_background = "none";
                } else if (fileExists(c, try expandPath(c, v))) {
                    mac.dmg_background = v;
                } else {
                    std.debug.print("  找不到这个文件\n", .{});
                    continue;
                }
                break;
            }
            c.changed = true;
        } else mac.dmg_background = "default";
    }
    const v = value(c, mac.dmg_background.?, "macos.dmg_background");
    if (eql(u8, v, "none")) return "";
    const path = if (eql(u8, v, "default"))
        try std.fs.path.join(c.a, &.{ m.zant_root, "tools", "zant", "package", "dmg-background.png" })
    else
        try expandPath(c, v);
    const data = m.readFile(c.a, c.io, path) catch fail("找不到 DMG 背景图片 {s}", .{path});
    const info = png.info(data) catch fail("{s} 不是可用的 PNG（需要 8 位、非隔行扫描）", .{path});
    if (info.width < 320 or info.height < 240 or info.width > 2000 or info.height > 1400)
        fail("DMG 背景图片 {s} 是 {d}×{d}：它的尺寸就是窗口大小（点），建议 640×400，Retina 版本另存为 @2x", .{ path, info.width, info.height });
    return path;
}

fn printResult(c: *Ctx, artifact: []const u8) void {
    _ = c;
    std.debug.print("\n" ++ m.Color.green ++ "完成" ++ m.Color.reset ++ "：{s}\n\n", .{artifact});
}

/// "Name (TEAMID)" → TEAMID
fn teamOf(identity: []const u8) ?[]const u8 {
    if (identity.len < 12 or identity[identity.len - 1] != ')') return null;
    const open = std.mem.lastIndexOfScalar(u8, identity, '(') orelse return null;
    return identity[open + 1 .. identity.len - 1];
}

/// Valid identities in the keychains: codesigning ones, or (installer) the
/// installer certificates, which `security` lists only without a policy.
fn identities(c: *Ctx, keychain: ?[]const u8, installer: bool) ![]const []const u8 {
    var argv: std.ArrayListUnmanaged([]const u8) = .empty;
    try argv.appendSlice(c.a, &.{ "security", "find-identity", "-v" });
    if (!installer) try argv.appendSlice(c.a, &.{ "-p", "codesigning" });
    if (keychain) |kc| try argv.append(c.a, kc);
    const res = m.runCapture(c.a, c.io, argv.items) catch return &.{};
    var list: std.ArrayListUnmanaged([]const u8) = .empty;
    var lines = std.mem.splitScalar(u8, res.stdout, '\n');
    while (lines.next()) |line| {
        const start = std.mem.indexOfScalar(u8, line, '"') orelse continue;
        const end = std.mem.indexOfScalarPos(u8, line, start + 1, '"') orelse continue;
        const name = line[start + 1 .. end];
        if (installer != (std.mem.indexOf(u8, name, "Installer") != null)) continue;
        var dup = false;
        for (list.items) |x| dup = dup or eql(u8, x, name);
        if (!dup) try list.append(c.a, name);
    }
    return list.items;
}

fn recommended(store: bool, name: []const u8) bool {
    if (store) return std.mem.startsWith(u8, name, "Apple Distribution") or std.mem.startsWith(u8, name, "3rd Party Mac Developer Application");
    return std.mem.startsWith(u8, name, "Developer ID Application");
}

fn askIdentity(c: *Ctx, store: bool) ![]const u8 {
    const all = try identities(c, null, false);
    var list: std.ArrayListUnmanaged([]const u8) = .empty;
    for (all) |id| if (recommended(store, id)) try list.append(c.a, id);
    const n_rec = list.items.len;
    for (all) |id| if (!recommended(store, id)) try list.append(c.a, id);
    const kind = if (store) "Apple Distribution" else "Developer ID Application";
    std.debug.print("\n签名证书（来自钥匙串）:\n", .{});
    if (!store) std.debug.print("  0) Ad-hoc（不用证书，只能在本机测试，不能公证）\n", .{});
    for (list.items, 0..) |id, i| std.debug.print("  {d}) {s}{s}\n", .{ i + 1, id, if (i < n_rec) "（推荐）" else "" });
    if (n_rec == 0) {
        std.debug.print("  " ++ m.Color.dim ++ "钥匙串里没有「{s}」证书：在 Xcode → 设置 → 帐户 → 管理证书 里添加，或从 developer.apple.com 下载后双击安装" ++ m.Color.reset ++ "\n", .{kind});
        if (store and list.items.len == 0) fail("没有可用的签名证书", .{});
    }
    const def = if (n_rec > 0) "1" else if (store) "1" else "0";
    while (true) {
        const v = try m.prompt(c.a, "选择", def);
        const n = std.fmt.parseInt(usize, v, 10) catch continue;
        if (n == 0 and !store) return "none";
        if (n >= 1 and n <= list.items.len) {
            if (n > n_rec) std.debug.print("  " ++ m.Color.dim ++ "提示：这个证书不是「{s}」，{s}" ++ m.Color.reset ++ "\n", .{ kind, if (store) "App Store 不会接受" else "签名的应用不能公证，别人的 Mac 会拦截" });
            return list.items[n - 1];
        }
    }
}

fn firstIdentity(c: *Ctx, keychain: []const u8, store: bool) ![]const u8 {
    const all = try identities(c, keychain, false);
    for (all) |id| if (recommended(store, id)) return id;
    if (all.len > 0) return all[0];
    fail("导入的证书里没有可用的签名身份", .{});
}

fn firstInstaller(c: *Ctx, keychain: []const u8) ![]const u8 {
    const all = try identities(c, keychain, true);
    if (all.len > 0) return all[0];
    fail("导入的证书里没有安装包证书（3rd Party Mac Developer Installer / Mac Installer Distribution）：macos.installer_identity 要另外设置", .{});
}

fn checkIdentity(c: *Ctx, identity: []const u8, key: []const u8, installer: bool) !void {
    const all = try identities(c, null, installer);
    for (all) |id| if (eql(u8, id, identity) or std.mem.indexOf(u8, id, identity) != null) return;
    std.debug.print("\n  钥匙串里可用的{s}证书：\n", .{if (installer) "安装包" else "签名"});
    for (all) |id| std.debug.print("    {s}\n", .{id});
    fail("{s}：钥匙串里没有可用的证书「{s}」", .{ key, identity });
}

fn askInstaller(c: *Ctx) ![]const u8 {
    const all = try identities(c, null, true);
    if (all.len == 0) fail("钥匙串里没有安装包证书：在 Xcode → 设置 → 帐户 → 管理证书 里添加「Mac Installer Distribution」", .{});
    const idx = try choose(c, "安装包证书（签 .pkg）", all, 0);
    return all[idx];
}

const ProfileInfo = struct { name: ?[]const u8 = null, team: ?[]const u8 = null, app_id: ?[]const u8 = null, mac: bool = false };

/// The `<string>` after `<key>key</key>` in a decoded profile.
fn plistString(xml: []const u8, key: []const u8, buf: []u8) ?[]const u8 {
    const needle = std.fmt.bufPrint(buf, "<key>{s}</key>", .{key}) catch return null;
    const at = std.mem.indexOf(u8, xml, needle) orelse return null;
    const s = std.mem.indexOfPos(u8, xml, at, "<string>") orelse return null;
    const e = std.mem.indexOfPos(u8, xml, s, "</string>") orelse return null;
    return xml[s + "<string>".len .. e];
}

fn profileInfo(c: *Ctx, path: []const u8) !ProfileInfo {
    const res = m.runCapture(c.a, c.io, &.{ "security", "cms", "-D", "-i", path }) catch return .{};
    const xml = res.stdout;
    var buf: [128]u8 = undefined;
    return .{
        .name = if (plistString(xml, "Name", &buf)) |v| try c.a.dupe(u8, v) else null,
        .team = if (plistString(xml, "TeamIdentifier", &buf)) |v| try c.a.dupe(u8, v) else null,
        .app_id = if (plistString(xml, "com.apple.application-identifier", &buf)) |v| try c.a.dupe(u8, v) else null,
        .mac = std.mem.indexOf(u8, xml, "<string>OSX</string>") != null,
    };
}

/// Installed Mac provisioning profiles for this bundle id (Xcode keeps them
/// in these folders), or a path typed in.
fn askProfile(c: *Ctx) ![]const u8 {
    const home = getEnv(c, "HOME") orelse "";
    var found: std.ArrayListUnmanaged([]const u8) = .empty;
    var labels: std.ArrayListUnmanaged([]const u8) = .empty;
    for ([_][]const u8{ "Library/Developer/Xcode/UserData/Provisioning Profiles", "Library/MobileDevice/Provisioning Profiles" }) |sub| {
        const dir_path = try std.fs.path.join(c.a, &.{ home, sub });
        var dir = Dir.cwd().openDir(c.io, dir_path, .{ .iterate = true }) catch continue;
        defer dir.close(c.io);
        var it = dir.iterate();
        while (it.next(c.io) catch null) |entry| {
            if (!std.mem.endsWith(u8, entry.name, ".provisionprofile")) continue;
            const p = try std.fs.path.join(c.a, &.{ dir_path, entry.name });
            const info = try profileInfo(c, p);
            const app_id = info.app_id orelse continue;
            const dot = std.mem.indexOfScalar(u8, app_id, '.') orelse continue;
            const bundle = app_id[dot + 1 ..];
            if (!info.mac or !(eql(u8, bundle, c.cfg.bundle_id) or eql(u8, bundle, "*"))) continue;
            try found.append(c.a, p);
            try labels.append(c.a, try std.fmt.allocPrint(c.a, "{s}（{s}）", .{ info.name orelse entry.name, app_id }));
        }
    }
    try labels.append(c.a, "输入 .provisionprofile 文件路径");
    std.debug.print("\n  " ++ m.Color.dim ++ "描述文件：developer.apple.com → Certificates, IDs & Profiles → Profiles 新建「Mac App Store Connect」类型，选 {s}，下载" ++ m.Color.reset ++ "\n", .{c.cfg.bundle_id});
    const idx = try choose(c, "Mac App Store 描述文件", labels.items, 0);
    if (idx < found.items.len) return found.items[idx];
    while (true) {
        const p = try askRequired(c, ".provisionprofile 文件路径", "");
        if (fileExists(c, try expandPath(c, p))) return p;
        std.debug.print("  找不到这个文件\n", .{});
    }
}

/// Notarization credentials as ZANT_NOTARY_* variables for the script.
fn notaryEnv(c: *Ctx, mac: *Macos, identity: []const u8) !void {
    if (mac.notary == null) {
        if (!c.interactive) need(c, "macos.notary", "公证凭据：{\"profile\": \"钥匙串配置名\"}、{\"api_key\": true} 或 {\"apple_id\", \"password\", \"team_id\"}");
        mac.notary = try askNotary(c, identity);
        c.changed = true;
    }
    const n = mac.notary.?;
    if (n.profile) |p| return setEnv(c, "ZANT_NOTARY_PROFILE", value(c, p, "macos.notary.profile"));
    if (n.api_key orelse false) {
        const k = try ascKey(c);
        try setEnv(c, "ZANT_NOTARY_KEY", k.path);
        try setEnv(c, "ZANT_NOTARY_KEY_ID", k.id);
        return setEnv(c, "ZANT_NOTARY_ISSUER", k.issuer);
    }
    if (n.apple_id) |id| {
        const pw = n.password orelse need(c, "macos.notary.password", "App 专用密码，写成 $环境变量名");
        try setEnv(c, "ZANT_NOTARY_APPLE_ID", value(c, id, "macos.notary.apple_id"));
        try setEnv(c, "ZANT_NOTARY_PASSWORD", try secret(c, pw, "macos.notary.password", "App 专用密码"));
        const team = if (n.team_id) |t| value(c, t, "macos.notary.team_id") else teamOf(identity) orelse need(c, "macos.notary.team_id", "团队 ID");
        return setEnv(c, "ZANT_NOTARY_TEAM", team);
    }
    fail("macos.notary 要设置 profile、api_key 或 apple_id 其中一种", .{});
}

fn askNotary(c: *Ctx, identity: []const u8) !Notary {
    const idx = try choose(c, "公证用的苹果帐户", &.{
        "已经用 notarytool 存在钥匙串里的配置（输入名称）",
        "Apple ID + App 专用密码（notarytool 验证后存进钥匙串，不写进配置文件）",
        "App Store Connect API key（Issuer ID、Key ID、.p8 文件）",
    }, 1);
    switch (idx) {
        0 => return .{ .profile = try askRequired(c, "钥匙串配置名称", "zant-notary") },
        1 => {
            const apple_id = try askRequired(c, "Apple ID（邮箱）", "");
            const team = try askRequired(c, "团队 ID", teamOf(identity) orelse "");
            const name = try askRequired(c, "存进钥匙串的配置名称", "zant-notary");
            std.debug.print("  " ++ m.Color.dim ++ "App 专用密码在 account.apple.com → 登录与安全 → App 专用密码 生成；下面由 notarytool 询问，zant 看不到它" ++ m.Color.reset ++ "\n", .{});
            try m.runInherit(c.a, c.io, &.{ "xcrun", "notarytool", "store-credentials", name, "--apple-id", apple_id, "--team-id", team }, c.root);
            return .{ .profile = name };
        },
        else => {
            _ = try ascKey(c);
            return .{ .api_key = true };
        },
    }
}

/// The App Store Connect API key, asked once (Issuer ID, Key ID, .p8 path).
fn ascKey(c: *Ctx) !Asc {
    if (c.asc) |k| return k;
    if (c.file.app_store_connect == null) c.file.app_store_connect = .{};
    const k = &c.file.app_store_connect.?;
    if (k.issuer_id == null or k.key_id == null or k.key == null) {
        if (!c.interactive) need(c, "app_store_connect", "issuer_id、key_id、key（.p8 文件路径或内容，写成 $环境变量名）");
        std.debug.print("\nApp Store Connect API key（App Store Connect → 用户和访问 → 集成 → App Store Connect API，权限「App 管理」）\n", .{});
        if (k.issuer_id == null) k.issuer_id = try askRequired(c, "Issuer ID", "");
        if (k.key_id == null) k.key_id = try askRequired(c, "Key ID", "");
        if (k.key == null) {
            const home = getEnv(c, "HOME") orelse "";
            var default: []const u8 = "";
            for ([_][]const u8{ ".appstoreconnect/private_keys", "private_keys", ".private_keys" }) |sub| {
                const p = try std.fmt.allocPrint(c.a, "{s}/{s}/AuthKey_{s}.p8", .{ home, sub, k.key_id.? });
                if (fileExists(c, p)) default = try std.fmt.allocPrint(c.a, "~/{s}/AuthKey_{s}.p8", .{ sub, k.key_id.? });
            }
            while (true) {
                const p = try askRequired(c, ".p8 文件路径", default);
                const full = try expandPath(c, p);
                if (!fileExists(c, full)) {
                    std.debug.print("  找不到这个文件\n", .{});
                    continue;
                }
                if (inProject(c, full)) std.debug.print("  " ++ m.Color.red ++ "注意" ++ m.Color.reset ++ "：这个 .p8 在项目目录里，千万不要提交到仓库（建议移到 ~/.appstoreconnect/private_keys/）\n", .{});
                k.key = p;
                break;
            }
        }
        c.changed = true;
    }
    const issuer = value(c, k.issuer_id.?, "app_store_connect.issuer_id");
    const id = value(c, k.key_id.?, "app_store_connect.key_id");
    const key = value(c, k.key.?, "app_store_connect.key");
    const path = if (std.mem.indexOf(u8, key, "-----BEGIN") != null)
        try writeSecretFile(c, try std.fmt.allocPrint(c.a, "AuthKey_{s}.p8", .{id}), key)
    else
        try fileOrBase64(c, key, "app_store_connect.key", try std.fmt.allocPrint(c.a, "AuthKey_{s}.p8", .{id}));
    c.asc = .{ .issuer = issuer, .id = id, .path = path };
    return c.asc.?;
}

// ---- Windows ----

const WinKind = enum { installer, store, folder };

fn windowsFlow(c: *Ctx) !void {
    if (c.file.windows == null) c.file.windows = .{};
    const w = &c.file.windows.?;
    if (w.distribution == null) {
        if (c.interactive) {
            const i = try choose(c, "发布方式（Windows）", &.{
                "安装包：setup.exe，可签名（推荐）",
                "Microsoft Store：MSIX 包，可提交到 Partner Center",
                "免安装：程序目录压缩成 zip",
            }, 0);
            w.distribution = switch (i) {
                0 => "installer",
                1 => "store",
                else => "folder",
            };
            c.changed = true;
        } else w.distribution = "installer";
    }
    const dist = value(c, w.distribution.?, "windows.distribution");
    const kind = std.meta.stringToEnum(WinKind, dist) orelse fail("windows.distribution 只能是 installer、store 或 folder（现在是 {s}）", .{dist});

    var subject_cn: ?[]const u8 = null;
    try winSigning(c, w, kind == .store, &subject_cn);

    if (kind == .installer) {
        if (w.publisher == null) {
            if (!c.interactive) need(c, "windows.publisher", "发布者名称，安装程序和「设置 → 应用」里显示");
            std.debug.print("\n", .{});
            w.publisher = try askRequired(c, "发布者名称（安装程序和「设置 → 应用」里显示）", subject_cn orelse "");
            c.changed = true;
        }
        try setEnv(c, "ZANT_PUBLISHER", value(c, w.publisher.?, "windows.publisher"));
        if ((try captureScript(c, "find-iscc")).len == 0) {
            if (!c.interactive) fail("找不到 Inno Setup（ISCC.exe）：winget install JRSoftware.InnoSetup，或设置 ZANT_ISCC", .{});
            std.debug.print("\n", .{});
            if (!try m.promptYesNo(c.a, "制作安装包要用 Inno Setup（免费）。现在用 winget 安装吗？", true)) fail("没有 Inno Setup，不能制作安装包", .{});
            runScript(c, "install-iscc");
            if ((try captureScript(c, "find-iscc")).len == 0) fail("安装后仍找不到 ISCC.exe：重新打开终端再试，或设置 ZANT_ISCC", .{});
        }
    }
    if (kind == .store) try storeSettings(c, w, subject_cn);
    save(c);

    std.debug.print("\n", .{});
    try m.compileApp(c.a, c.io, c.root, true);
    const folder = try m.packageWindows(c.a, c.io, c.root, &c.cfg);
    const out = try std.fs.path.join(c.a, &.{ c.root, "zig-out", "dist" });
    try setEnv(c, "ZANT_KIND", dist);
    try setEnv(c, "ZANT_TITLE", c.cfg.title);
    try setEnv(c, "ZANT_VERSION", c.cfg.version);
    try setEnv(c, "ZANT_BUNDLE_ID", c.cfg.bundle_id);
    try setEnv(c, "ZANT_DIR", folder);
    try setEnv(c, "ZANT_OUT", out);
    if (kind == .installer) {
        if (try appIcon(c)) |img| {
            const ico = try std.fs.path.join(c.a, &.{ c.root, "zig-out", "build", "app.ico" });
            try m.writeFile(c.a, c.io, ico, try png.buildIco(c.a, img));
            try setEnv(c, "ZANT_ICO", ico);
        }
    }
    if (kind == .store) try msixAssets(c);
    runScript(c, "package");
    const artifact = switch (kind) {
        .installer => try std.fmt.allocPrint(c.a, "{s}\\{s}-{s}-setup.exe", .{ out, c.cfg.title, c.cfg.version }),
        .store => try std.fmt.allocPrint(c.a, "{s}\\{s}-{s}.msix", .{ out, c.cfg.title, c.cfg.version }),
        .folder => try std.fmt.allocPrint(c.a, "{s}\\{s}-{s}-windows.zip", .{ out, c.cfg.title, c.cfg.version }),
    };
    printResult(c, artifact);
}

fn appIcon(c: *Ctx) !?png.Image {
    const source = try std.fs.path.join(c.a, &.{ c.root, "assets", "icons", "app-windows.png" });
    const data = m.readFile(c.a, c.io, source) catch return null;
    return png.decode(c.a, data) catch |err| fail("{s} 无法读取（{s}）：需要 8 位、非隔行扫描的 PNG", .{ source, @errorName(err) });
}

fn isThumbprint(v: []const u8) bool {
    if (v.len != 40) return false;
    for (v) |ch| if (!std.ascii.isHex(ch)) return false;
    return true;
}

/// "CN=Name, O=…" → "Name"
fn commonName(subject: []const u8) ?[]const u8 {
    const at = std.mem.indexOf(u8, subject, "CN=") orelse return null;
    const rest = subject[at + 3 ..];
    const end = std.mem.indexOfScalar(u8, rest, ',') orelse rest.len;
    return std.mem.trim(u8, rest[0..end], " \"");
}

/// Signing settings as ZANT_SIGN_* variables; asks when nothing is set.
fn winSigning(c: *Ctx, w: *Windows, store: bool, subject_cn: *?[]const u8) !void {
    if (w.certificate == null and w.signing_identity == null) {
        if (!c.interactive) {
            std.debug.print("  " ++ m.Color.dim ++ "没有设置 windows.signing_identity / certificate：不签名" ++ m.Color.reset ++ "\n", .{});
            return;
        }
        const listing = try captureScript(c, "list-certs");
        var thumbs: std.ArrayListUnmanaged([]const u8) = .empty;
        var subjects: std.ArrayListUnmanaged([]const u8) = .empty;
        std.debug.print("\n代码签名证书（Windows 证书存储：当前用户 → 个人）:\n", .{});
        std.debug.print("  0) 不签名{s}\n", .{if (store) "（提交商店时由微软签名，推荐）" else "（用户运行安装包时会看到「未知发布者」提示）"});
        var lines = std.mem.splitScalar(u8, listing, '\n');
        while (lines.next()) |raw| {
            const line = std.mem.trim(u8, raw, " \r");
            var parts = std.mem.splitScalar(u8, line, '|');
            const thumb = parts.next() orelse continue;
            const subject = parts.next() orelse continue;
            const expiry = parts.next() orelse "";
            if (!isThumbprint(thumb)) continue;
            try thumbs.append(c.a, thumb);
            try subjects.append(c.a, subject);
            std.debug.print("  {d}) {s}（到期 {s}）\n", .{ thumbs.items.len, subject, expiry });
        }
        std.debug.print("  f) 使用 .pfx 证书文件\n", .{});
        const def = if (thumbs.items.len > 0 and !store) "1" else "0";
        while (true) {
            const v = try m.prompt(c.a, "选择", def);
            if (eql(u8, v, "0")) {
                w.signing_identity = "none";
                break;
            }
            if (eql(u8, v, "f") or eql(u8, v, "F")) {
                while (true) {
                    const p = try askRequired(c, ".pfx 文件路径", "");
                    const full = try expandPath(c, p);
                    if (!fileExists(c, full)) {
                        std.debug.print("  找不到这个文件\n", .{});
                        continue;
                    }
                    if (inProject(c, full)) std.debug.print("  " ++ m.Color.red ++ "注意" ++ m.Color.reset ++ "：证书在项目目录里，千万不要提交到仓库\n", .{});
                    w.certificate = p;
                    break;
                }
                const name = try askRequired(c, "证书密码放在哪个环境变量（密码不写进配置；变量没设置时每次询问）", "ZANT_WIN_CERT_PASSWORD");
                w.certificate_password = try std.fmt.allocPrint(c.a, "${s}", .{name});
                break;
            }
            const n = std.fmt.parseInt(usize, v, 10) catch continue;
            if (n >= 1 and n <= thumbs.items.len) {
                w.signing_identity = thumbs.items[n - 1];
                subject_cn.* = commonName(subjects.items[n - 1]);
                break;
            }
        }
        c.changed = true;
    }
    if (w.certificate) |cert| {
        try setEnv(c, "ZANT_SIGN_PFX", try fileOrBase64(c, value(c, cert, "windows.certificate"), "windows.certificate", "signing.pfx"));
        if (w.certificate_password) |p| try setEnv(c, "ZANT_SIGN_PFX_PASSWORD", try secret(c, p, "windows.certificate_password", "证书密码"));
    } else {
        const id = value(c, w.signing_identity.?, "windows.signing_identity");
        if (eql(u8, id, "none")) return;
        try setEnv(c, if (isThumbprint(id)) "ZANT_SIGN_THUMBPRINT" else "ZANT_SIGN_SUBJECT", id);
    }
    try setEnv(c, "ZANT_SIGN_TIMESTAMP", if (w.timestamp_url) |t| value(c, t, "windows.timestamp_url") else "http://timestamp.digicert.com");
}

fn storeSettings(c: *Ctx, w: *Windows, subject_cn: ?[]const u8) !void {
    if (w.store == null) w.store = .{};
    const s = &w.store.?;
    if (s.identity_name == null or s.publisher == null or s.publisher_display_name == null) {
        if (!c.interactive) need(c, "windows.store", "identity_name、publisher、publisher_display_name（Partner Center → 产品管理 → 产品标识）");
        std.debug.print("\nMicrosoft Store 产品标识（Partner Center → 应用 → 产品管理 → 产品标识）\n", .{});
        if (s.identity_name == null) s.identity_name = try askRequired(c, "Package/Identity/Name", "");
        if (s.publisher == null) s.publisher = try askRequired(c, "Package/Identity/Publisher（CN=…）", "");
        if (s.publisher_display_name == null) s.publisher_display_name = try askRequired(c, "Package/Properties/PublisherDisplayName", subject_cn orelse "");
        c.changed = true;
    }
    if (s.upload == null) {
        if (c.interactive) {
            std.debug.print("\n", .{});
            s.upload = try m.promptYesNo(c.a, "打包后用 msstore 命令行提交到 Partner Center？（否：自己在网页上传 .msix）", false);
            c.changed = true;
        } else s.upload = false;
    }
    if (s.upload.? and s.product_id == null) {
        if (!c.interactive) need(c, "windows.store.product_id", "Store ID，如 9NBLGGH4R315");
        s.product_id = try askRequired(c, "Store ID（Partner Center → 产品标识，9 开头）", "");
        c.changed = true;
    }
    try setEnv(c, "ZANT_MSIX_NAME", value(c, s.identity_name.?, "windows.store.identity_name"));
    try setEnv(c, "ZANT_MSIX_PUBLISHER", value(c, s.publisher.?, "windows.store.publisher"));
    try setEnv(c, "ZANT_MSIX_PUBLISHER_NAME", value(c, s.publisher_display_name.?, "windows.store.publisher_display_name"));
    try setEnv(c, "ZANT_MSIX_VERSION", try msixVersion(c.a, c.cfg.version));
    try setEnv(c, "ZANT_STORE_UPLOAD", if (s.upload.?) "1" else "0");
    if (s.upload.?) {
        try setEnv(c, "ZANT_STORE_PRODUCT_ID", value(c, s.product_id.?, "windows.store.product_id"));
        // Partner Center credentials (Azure AD app); without them msstore uses
        // the ones saved by an earlier `msstore reconfigure`
        if (s.client_secret) |cs| {
            try setEnv(c, "ZANT_STORE_CLIENT_SECRET", try secret(c, cs, "windows.store.client_secret", "客户端密钥"));
            try setEnv(c, "ZANT_STORE_TENANT_ID", value(c, s.tenant_id orelse need(c, "windows.store.tenant_id", "租户 ID"), "windows.store.tenant_id"));
            try setEnv(c, "ZANT_STORE_SELLER_ID", value(c, s.seller_id orelse need(c, "windows.store.seller_id", "卖家 ID"), "windows.store.seller_id"));
            try setEnv(c, "ZANT_STORE_CLIENT_ID", value(c, s.client_id orelse need(c, "windows.store.client_id", "客户端 ID"), "windows.store.client_id"));
        }
    }
}

/// "1.2.3" → "1.2.3.0": MSIX wants four numbers, the Store a last one of 0.
fn msixVersion(a: Allocator, version: []const u8) ![]const u8 {
    var parts: [3]u32 = .{ 0, 0, 0 };
    var it = std.mem.splitScalar(u8, version, '.');
    var i: usize = 0;
    while (it.next()) |part| : (i += 1) {
        if (i >= 3) break;
        const digits = part[0 .. std.mem.indexOfNone(u8, part, "0123456789") orelse part.len];
        parts[i] = std.fmt.parseInt(u16, digits, 10) catch fail("版本号 {s} 不能用于 MSIX：要是 1.2.3 这样的数字", .{version});
    }
    return std.fmt.allocPrint(a, "{d}.{d}.{d}.0", .{ parts[0], parts[1], parts[2] });
}

/// MSIX logos from assets/icons/app-windows.png.
fn msixAssets(c: *Ctx) !void {
    const img = (try appIcon(c)) orelse fail("MSIX 需要应用图标 assets/icons/app-windows.png", .{});
    const dir = try std.fs.path.join(c.a, &.{ c.root, "zig-out", "build", "msix-assets" });
    Dir.cwd().deleteTree(c.io, dir) catch {};
    try Dir.cwd().createDirPath(c.io, dir);
    for ([_]struct { name: []const u8, size: u32 }{
        .{ .name = "StoreLogo.png", .size = 50 },
        .{ .name = "Square44x44Logo.png", .size = 44 },
        .{ .name = "Square150x150Logo.png", .size = 150 },
    }) |asset| {
        const resized = try png.resize(c.a, img, asset.size, asset.size);
        try m.writeFile(c.a, c.io, try std.fs.path.join(c.a, &.{ dir, asset.name }), try png.encode(c.a, resized));
    }
    try setEnv(c, "ZANT_MSIX_ASSETS", dir);
}

test "msix version and identity helpers" {
    const a = std.testing.allocator;
    const v = try msixVersion(a, "0.1.0");
    defer a.free(v);
    try std.testing.expectEqualStrings("0.1.0.0", v);
    const v2 = try msixVersion(a, "2.5");
    defer a.free(v2);
    try std.testing.expectEqualStrings("2.5.0.0", v2);
    try std.testing.expectEqualStrings("ABCDE12345", teamOf("Developer ID Application: Some One (ABCDE12345)").?);
    try std.testing.expect(teamOf("none") == null);
    try std.testing.expectEqualStrings("Some One", commonName("CN=Some One, O=Acme, C=CN").?);
    try std.testing.expect(isThumbprint("0123456789abcdef0123456789ABCDEF01234567"));
    try std.testing.expectEqualStrings("NAME", envName("${NAME}").?);
    try std.testing.expectEqualStrings("NAME", envName("$NAME").?);
    try std.testing.expect(envName("plain") == null);
}
