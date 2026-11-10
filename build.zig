const std = @import("std");
const builtin = @import("builtin");

const skia_libs = [_][]const u8{
    "third_party/skia/skia-binaries/libskparagraph.a",
    "third_party/skia/skia-binaries/libskshaper.a",
    "third_party/skia/skia-binaries/libskunicode_icu.a",
    "third_party/skia/skia-binaries/libskunicode_core.a",
    "third_party/skia/skia-binaries/libskia.a",
};

fn configureMacosSkia(b: *std.Build, target: std.Build.ResolvedTarget, mod: *std.Build.Module) void {
    _ = target;
    mod.addIncludePath(b.path(""));
    mod.addIncludePath(b.path("third_party/skia-headers"));
    mod.addIncludePath(b.path("third_party/skia-headers/include/private"));
    mod.addIncludePath(b.path("third_party/skia-headers/src"));
    mod.addCSourceFiles(.{
        .root = b.path(""),
        .files = &.{"src/renderer/skia/skia_bridge.cpp"},
        .language = .cpp,
        .flags = &.{ "-std=c++20", "-fno-exceptions", "-fno-rtti", "-Wno-deprecated-declarations" },
    });
    mod.addCSourceFiles(.{
        .root = b.path(""),
        .files = &.{"src/renderer/skia/skia_bridge_mac.mm"},
        .language = .objective_cpp,
        .flags = &.{ "-std=c++20", "-fobjc-arc", "-fno-exceptions", "-fno-rtti", "-Wno-deprecated-declarations" },
    });
    mod.addCSourceFiles(.{
        .root = b.path(""),
        .files = &.{"src/platform/macos/window.m"},
        .flags = &.{ "-fobjc-arc", "-fblocks", "-Wno-deprecated-declarations" },
    });
    mod.addCSourceFiles(.{
        .root = b.path(""),
        .files = &.{"src/platform/macos/pdf.mm"},
        .flags = &.{ "-fobjc-arc", "-Wno-deprecated-declarations" },
    });
    for (skia_libs) |lib| {
        mod.addObjectFile(b.path(lib));
    }
    const frameworks = [_][]const u8{
        "AppKit",       "Metal",             "MetalKit", "QuartzCore", "CoreText",               "CoreFoundation",
        "CoreGraphics", "IOKit",             "Security", "CoreVideo",  "UniformTypeIdentifiers", "PDFKit",
        "Carbon",       "ServiceManagement",
    };
    for (frameworks) |framework| {
        mod.linkFramework(framework, .{});
    }
    mod.linkSystemLibrary("z", .{ .use_pkg_config = .no });
    mod.linkSystemLibrary("sqlite3", .{ .use_pkg_config = .no });
}

/// Windows: Zig compiles the Win32 layer (window.c); Skia, the D3D12 swap
/// chain and PDF live in zant_skia.dll, built with MSVC by
/// scripts/win/build_bridge.ps1 (the prebuilt Skia uses MSVC's STL). Zig
/// links the DLL through an import library generated from zant_skia.def,
/// so `zig build -Dtarget=x86_64-windows` also type-checks on other hosts.
fn configureWindows(b: *std.Build, target: std.Build.ResolvedTarget, mod: *std.Build.Module) ?std.Build.LazyPath {
    mod.addIncludePath(b.path(""));
    mod.addCSourceFiles(.{
        .root = b.path(""),
        .files = &.{"src/platform/windows/window.c"},
        .flags = &.{ "-std=c11", "-Wno-deprecated-declarations" },
    });
    const machine = switch (target.result.cpu.arch) {
        .aarch64 => "arm64",
        else => "i386:x86-64",
    };
    mod.addObjectFile(importLibrary(b, "src/renderer/skia/zant_skia.def", "zant_skia.lib", machine));
    mod.addObjectFile(importLibrary(b, "src/platform/windows/winsqlite3.def", "winsqlite3.lib", machine));
    const system_libs = [_][]const u8{ "user32", "gdi32", "imm32", "dwmapi", "shell32", "ole32", "comctl32", "uuid", "kernel32", "advapi32" };
    for (system_libs) |lib| mod.linkSystemLibrary(lib, .{ .use_pkg_config = .no });

    // the DLL itself can only be built on Windows (MSVC)
    if (builtin.os.tag != .windows) return null;
    // release archives (install.ps1) ship it prebuilt, with the MSVC
    // runtime: no Visual Studio needed to build apps
    if (hasPrebuiltBridge(b)) {
        const dir = b.path(prebuilt_bridge);
        b.addNamedLazyPath("zant_skia", dir);
        return dir;
    }
    const bridge = b.addSystemCommand(&.{ "powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File" });
    bridge.addFileArg(b.path("scripts/win/build_bridge.ps1"));
    bridge.addArg("-OutDir");
    const out = bridge.addOutputDirectoryArg("zant_skia");
    const inputs = [_][]const u8{
        "src/renderer/skia/skia_bridge.cpp",
        "src/renderer/skia/skia_bridge_win.cpp",
        "src/renderer/skia/skia_bridge.h",
        "src/renderer/skia/skia_bridge_internal.h",
        "src/renderer/skia/zant_skia_resources.h",
        "src/renderer/skia/zant_skia.def",
        "src/platform/windows/pdf_win.cpp",
        "src/platform/pdf.h",
        "scripts/win/build_bridge.ps1",
    };
    for (inputs) |input| bridge.addFileInput(b.path(input));
    b.addNamedLazyPath("zant_skia", out);
    return out;
}

/// Folder of the prebuilt zant_skia.dll in release archives (made by
/// scripts/release/package.ps1; a source checkout never has it).
const prebuilt_bridge = "third_party/zant-skia-prebuilt";
/// MSVC runtime the bridge needs, next to it in release archives.
const msvc_runtime = [_][]const u8{ "vcruntime140.dll", "vcruntime140_1.dll", "msvcp140.dll" };

fn hasPrebuiltBridge(b: *std.Build) bool {
    const path = b.root.joinString(b.allocator, prebuilt_bridge ++ "/zant_skia.dll") catch return false;
    std.Io.Dir.cwd().access(b.graph.io, path, .{}) catch return false;
    return true;
}

fn importLibrary(b: *std.Build, def: []const u8, name: []const u8, machine: []const u8) std.Build.LazyPath {
    const run = b.addSystemCommand(&.{ b.graph.zig_exe, "dlltool", "-m", machine, "-d" });
    run.addFileArg(b.path(def));
    run.addArg("-l");
    return run.addOutputFileArg(name);
}

/// Per-app Windows setup, a no-op elsewhere. Call it from an app's build.zig:
///
///     const zant_build = @import("zant_ui");
///     zant_build.configureApp(b, zant_dep, exe, &.{ run_cmd, run_tests });
///
/// It embeds the manifest (per-monitor DPI, UTF-8 code page, Common Controls
/// v6), uses the GUI subsystem for release builds, installs zant_skia.dll
/// (and pdfium.dll) next to the exe and puts that directory on PATH for the
/// given run steps (tests are not installed next to the DLL).
pub fn configureApp(b: *std.Build, zant_dep: *std.Build.Dependency, exe: *std.Build.Step.Compile, runs: []const *std.Build.Step.Run) void {
    addAppIcons(b, exe.root_module);
    // the app's tests (and other compiled run steps) see the same icon imports
    for (runs) |run| {
        if (run.producer) |artifact| {
            if (artifact.root_module != exe.root_module) addAppIcons(b, artifact.root_module);
        }
    }
    if (exe.rootModuleTarget().os.tag != .windows) return;
    exe.win32_manifest = zant_dep.path("src/platform/windows/zant.manifest");
    if (exe.root_module.optimize != .debug) exe.subsystem = .windows;
    addWindowsIcon(b, zant_dep, exe);
    if (builtin.os.tag != .windows) return;
    installRuntime(b, zant_dep.namedLazyPath("zant_skia"), hasPrebuiltBridge(zant_dep.builder), runs);
}

/// App icons in `assets/icons/` (created by `zant create`, replaceable).
/// Each one that exists is importable by name: `@embedFile("icon_tray_dark")`.
pub const app_icons = [_]struct { file: []const u8, import: []const u8 }{
    .{ .file = "assets/icons/app-macos.png", .import = "icon_app_macos" }, // 1024×1024, Apple icon grid
    .{ .file = "assets/icons/app-windows.png", .import = "icon_app_windows" }, // 256×256, full bleed
    .{ .file = "assets/icons/tray-dark.png", .import = "icon_tray_dark" }, // Windows tray, dark taskbar (white)
    .{ .file = "assets/icons/tray-light.png", .import = "icon_tray_light" }, // Windows tray, light taskbar (black)
    .{ .file = "assets/icons/statusbar.png", .import = "icon_statusbar" }, // macOS menu bar template, 18 pt
};

fn appFileExists(b: *std.Build, sub_path: []const u8) bool {
    const full = b.root.joinString(b.allocator, sub_path) catch return false;
    std.Io.Dir.cwd().access(b.graph.io, full, .{}) catch return false;
    return true;
}

/// Every PNG in assets/icons is importable as `icon_<name>` ("-" → "_"):
/// the defaults above plus the app's own (ClipHop's `statusbar-sync.png` is
/// `@embedFile("icon_statusbar_sync")`).
fn addAppIcons(b: *std.Build, module: *std.Build.Module) void {
    const io = b.graph.io;
    const full = b.root.joinString(b.allocator, "assets/icons") catch return;
    var dir = std.Io.Dir.cwd().openDir(io, full, .{ .iterate = true }) catch return;
    defer dir.close(io);
    var it = dir.iterate();
    while (it.next(io) catch null) |entry| {
        if (entry.kind != .file or !std.mem.endsWith(u8, entry.name, ".png")) continue;
        const stem = entry.name[0 .. entry.name.len - ".png".len];
        const import = b.fmt("icon_{s}", .{stem});
        std.mem.replaceScalar(u8, import, '-', '_');
        module.addAnonymousImport(import, .{ .root_source_file = b.path(b.fmt("assets/icons/{s}", .{entry.name})) });
    }
}

/// Windows: the .ico (16–256, built from app-windows.png by `zant-icon`) as
/// icon resource 1 — Explorer's file icon, and the window / taskbar icon
/// (the Win32 backend loads resource 1).
fn addWindowsIcon(b: *std.Build, zant_dep: *std.Build.Dependency, exe: *std.Build.Step.Compile) void {
    const source = "assets/icons/app-windows.png";
    if (!appFileExists(b, source)) return;
    const make_ico = b.addRunArtifact(zant_dep.artifact("zant-icon"));
    make_ico.addArg("ico");
    make_ico.addFileArg(b.path(source));
    const ico = make_ico.addOutputFileArg("app.ico");
    const files = b.addWriteFiles();
    _ = files.addCopyFile(ico, "app.ico");
    const rc = files.add("app.rc", "1 ICON \"app.ico\"\n");
    exe.root_module.addWin32ResourceFile(.{ .file = rc, .include_paths = &.{files.getDirectory()} });
}

fn installRuntime(b: *std.Build, dir: std.Build.LazyPath, prebuilt: bool, runs: []const *std.Build.Step.Run) void {
    const runtime = b.step("zant-runtime", "Install zant_skia.dll and pdfium.dll");
    for ([_][]const u8{ "zant_skia.dll", "pdfium.dll" }) |name| {
        runtime.dependOn(&b.addInstallBinFile(dir.path(b, name), name).step);
    }
    // the prebuilt bridge comes with the MSVC runtime (a machine without
    // Visual Studio may not have it)
    if (prebuilt) for (msvc_runtime) |name| {
        runtime.dependOn(&b.addInstallBinFile(dir.path(b, name), name).step);
    };
    b.getInstallStep().dependOn(runtime);
    // the install prefix is only known when the build runs; this assumes the
    // default (zig-out) for the PATH of run steps
    const bin = b.pathJoin(&.{ b.root.root_dir.path orelse ".", "zig-out", "bin" });
    const path = if (b.graph.environ_map.get("PATH")) |old| b.fmt("{s};{s}", .{ bin, old }) else bin;
    for (runs) |run| {
        run.step.dependOn(runtime);
        run.setEnvironmentVariable("PATH", path);
    }
}

/// Compiles a `.tsx` component file into a Zig module that imports `zant`
/// (`docs/tsx.md`). `name` is the generated file's name in Zig errors.
///
///     const counter = zant_build.addTsx(b, tsx_tool, b.path("src/App.tsx"), "App.zig", zant_mod);
///     exe.root_module.addImport("app_tsx", counter);
pub fn addTsx(b: *std.Build, tool: *std.Build.Step.Compile, source: std.Build.LazyPath, name: []const u8, zant_mod: *std.Build.Module) *std.Build.Module {
    const run = b.addRunArtifact(tool);
    run.addFileArg(source);
    const out = run.addOutputFileArg(name);
    return b.createModule(.{
        .root_source_file = out,
        .imports = &.{.{ .name = "zant", .module = zant_mod }},
    });
}

/// Compiles `.tsx` files as one project and copies hand-written `.zig` sources
/// beside the generated files (relative imports resolve; docs/tsx.md).
/// The module's root is `entry`'s generated file.
///
///     const sources = &.{ b.path("src/App.tsx"), b.path("src/NoteCard.tsx") };
///     const notes = addTsxProject(b, tsx_tool, sources, "App", zant_mod);
pub fn addTsxProject(
    b: *std.Build,
    tool: *std.Build.Step.Compile,
    sources: []const std.Build.LazyPath,
    entry: []const u8,
    zant_mod: *std.Build.Module,
) *std.Build.Module {
    const run = b.addRunArtifact(tool);
    run.addArg("--out");
    const out = run.addOutputDirectoryArg("tsx");
    // imported assets (`import icon from "./icon.png"`) are inputs too
    run.addArg("--dep");
    _ = run.addDepFileOutputArg("tsx.d");
    for (sources) |source| run.addFileArg(source);
    return b.createModule(.{
        .root_source_file = out.path(b, b.fmt("{s}.zig", .{entry})),
        .imports = &.{.{ .name = "zant", .module = zant_mod }},
    });
}

pub const TsxAppOptions = struct {
    /// Every `.tsx` / `.zig` under this directory (relative to the build
    /// root, any depth) is part of the app.
    src: []const u8 = "src",
    /// The file whose default export is the root component (`App.tsx`).
    entry: []const u8 = "App",
    target: std.Build.ResolvedTarget,
    optimize: std.builtin.OptimizeMode,
};

/// A TS + TSX app (`zant create`, language ts): compiles every file under
/// `options.src` and returns the module whose root is the generated
/// `main.zig` (it runs the entry's default export with `zant.tsx.app.run`).
///
///     const app = zant_build.addTsxApp(b, zant_dep, .{ .target = target, .optimize = optimize });
///     const exe = b.addExecutable(.{ .name = "app", .root_module = app });
pub fn addTsxApp(b: *std.Build, zant_dep: *std.Build.Dependency, options: TsxAppOptions) *std.Build.Module {
    return tsxApp(b, zant_dep.artifact("zant-tsx"), zant_dep.module("zant"), options);
}

fn tsxApp(
    b: *std.Build,
    tool: *std.Build.Step.Compile,
    zant_mod: *std.Build.Module,
    options: TsxAppOptions,
) *std.Build.Module {
    const run = b.addRunArtifact(tool);
    run.addArg("--out");
    const out = run.addOutputDirectoryArg("tsx");
    run.addArg("--dep");
    _ = run.addDepFileOutputArg("tsx.d");
    run.addArgs(&.{ "--main", options.entry, "--src" });
    run.addDirectoryArg(b.path(options.src));
    // a directory argument isn't hashed by content: list its files as inputs
    addSourceInputs(b, run, options.src, 0);
    return b.createModule(.{
        .root_source_file = out.path(b, "main.zig"),
        .target = options.target,
        .optimize = options.optimize,
        .link_libc = true,
        .link_libcpp = options.target.result.os.tag == .macos,
        .imports = &.{.{ .name = "zant", .module = zant_mod }},
    });
}

fn addSourceInputs(b: *std.Build, run: *std.Build.Step.Run, sub_path: []const u8, depth: usize) void {
    if (depth > 16) return;
    const io = b.graph.io;
    const full = b.root.joinString(b.allocator, sub_path) catch return;
    var dir = std.Io.Dir.cwd().openDir(io, full, .{ .iterate = true }) catch return;
    defer dir.close(io);
    var it = dir.iterate();
    while (it.next(io) catch null) |entry| {
        if (entry.name.len == 0 or entry.name[0] == '.') continue;
        const child = b.pathJoin(&.{ sub_path, entry.name });
        switch (entry.kind) {
            .directory => addSourceInputs(b, run, child, depth + 1),
            .file => if (std.mem.endsWith(u8, entry.name, ".tsx") or std.mem.endsWith(u8, entry.name, ".zig")) {
                run.addFileInput(b.path(child));
            },
            else => {},
        }
    }
}

/// Golden files of the TSX compiler (`tools/tsx/testdata/<name>.tsx` → `.zig`).
const tsx_golden = [_][]const u8{
    "counter",      "layout",    "text",     "events",         "conditional", "components", "todos",   "arrays",
    "children",     "theme",     "list",     "local_state",    "hooks",       "strings",    "store",   "nesting",
    "widgets",      "functions", "loops",    "spread",         "app_hooks",   "builtins",   "windows", "desktop",
    "react_compat", "entities",  "platform", "named_handlers", "narrowing",   "defaults",
};

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const zant_mod = b.addModule("zant", .{
        .root_source_file = b.path("src/root.zig"),
        .target = target,
        .optimize = optimize,
        .link_libc = true,
        // Windows: C++ lives in zant_skia.dll (MSVC), not in Zig-built code
        .link_libcpp = target.result.os.tag == .macos,
    });
    var bridge_dir: ?std.Build.LazyPath = null;
    switch (target.result.os.tag) {
        .macos => configureMacosSkia(b, target, zant_mod),
        .windows => bridge_dir = configureWindows(b, target, zant_mod),
        else => @panic("Zant UI supports macOS and Windows; this target has no platform backend."),
    }
    const is_macos = target.result.os.tag == .macos;

    const exe = b.addExecutable(.{
        .name = "zant-gallery",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/main.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
            .link_libcpp = is_macos,
            .imports = &.{.{ .name = "zant", .module = zant_mod }},
        }),
    });

    b.installArtifact(exe);

    // PNG read / resize / write (src/core/png.zig): shared by the CLI tools
    const png_mod = b.createModule(.{ .root_source_file = b.path("src/core/png.zig") });
    const tool_options = b.addOptions();
    tool_options.addOption([]const u8, "zant_root", b.root.root_dir.path orelse ".");
    // the Zig that builds zant is the one it runs for apps (the install
    // scripts may put a pinned Zig outside PATH)
    tool_options.addOption([]const u8, "zig_exe", b.graph.zig_exe);
    const zant_tool = b.addExecutable(.{
        .name = "zant",
        .root_module = b.createModule(.{
            .root_source_file = b.path("tools/zant/main.zig"),
            .target = target,
            .optimize = optimize,
            .imports = &.{ .{ .name = "build_options", .module = tool_options.createModule() }, .{ .name = "png", .module = png_mod } },
        }),
    });
    b.installArtifact(zant_tool);
    const cli_step = b.step("cli", "Build only the zant CLI (install scripts, zant update)");
    cli_step.dependOn(&b.addInstallArtifact(zant_tool, .{}).step);

    // build-time icon tool for apps (runs on the build machine)
    const icon_tool = b.addExecutable(.{
        .name = "zant-icon",
        .root_module = b.createModule(.{
            .root_source_file = b.path("tools/zant/icon.zig"),
            .target = b.graph.host,
            .optimize = .ReleaseSafe,
            .imports = &.{.{ .name = "png", .module = png_mod }},
        }),
    });
    b.installArtifact(icon_tool);

    // TSX components → Zig (docs/tsx.md); runs on the build machine
    const tsx_tool = b.addExecutable(.{
        .name = "zant-tsx",
        .root_module = b.createModule(.{
            .root_source_file = b.path("tools/tsx/main.zig"),
            .target = b.graph.host,
            .optimize = .ReleaseSafe,
        }),
    });
    b.installArtifact(tsx_tool);

    // the generated tag table (tools/tsx/ui_tags.zig) comes from reflecting
    // the ui.* option structs; `zig build tsx-tags` rewrites it and the test
    // step checks it is up to date
    const tsx_tags_tool = b.addExecutable(.{
        .name = "zant-tsx-tags",
        .root_module = b.createModule(.{
            .root_source_file = b.path("tools/tsx/manifest/main.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
            .link_libcpp = is_macos,
            .imports = &.{.{ .name = "zant", .module = zant_mod }},
        }),
    });
    const ui_tags_path = b.fmt("{s}/tools/tsx/ui_tags.zig", .{b.root.root_dir.path orelse "."});
    const write_tags = b.addRunArtifact(tsx_tags_tool);
    write_tags.addArg(ui_tags_path);
    b.step("tsx-tags", "Regenerate the TSX tag table (tools/tsx/ui_tags.zig)").dependOn(&write_tags.step);
    const check_tags = b.addRunArtifact(tsx_tags_tool);
    check_tags.addArgs(&.{ "--check", ui_tags_path });

    const tsx_dts_tool = b.addExecutable(.{
        .name = "zant-tsx-dts",
        .root_module = b.createModule(.{
            .root_source_file = b.path("tools/tsx/dts.zig"),
            .target = b.graph.host,
            .optimize = optimize,
        }),
    });
    // `zant create` copies it into TS projects
    const dts_paths = &.{b.fmt("{s}/tools/tsx/zant.d.ts", .{b.root.root_dir.path orelse "."})};
    const write_dts = b.addRunArtifact(tsx_dts_tool);
    write_dts.addArgs(dts_paths);
    b.step("tsx-dts", "Regenerate the TSX editor declarations").dependOn(&write_dts.step);
    const check_dts = b.addRunArtifact(tsx_dts_tool);
    check_dts.addArg("--check");
    check_dts.addArgs(dts_paths);

    const demo = b.addExecutable(.{
        .name = "zant-demo",
        .root_module = b.createModule(.{
            .root_source_file = b.path("examples/declarative.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
            .link_libcpp = is_macos,
            .imports = &.{.{ .name = "zant", .module = zant_mod }},
        }),
    });
    b.installArtifact(demo);

    const run_demo = b.addRunArtifact(demo);
    run_demo.step.dependOn(b.getInstallStep());
    run_demo.addPassthruArgs();
    const run_step = b.step("run", "Run the declarative Zant UI demo");
    run_step.dependOn(&run_demo.step);

    const run_gallery_cmd = b.addRunArtifact(exe);
    run_gallery_cmd.step.dependOn(b.getInstallStep());
    run_gallery_cmd.addPassthruArgs();
    const run_gallery_step = b.step("run-gallery", "Run the imperative component gallery");
    run_gallery_step.dependOn(&run_gallery_cmd.step);

    const math_demo = b.addExecutable(.{
        .name = "zant-math",
        .root_module = b.createModule(.{
            .root_source_file = b.path("examples/math.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
            .link_libcpp = is_macos,
            .imports = &.{.{ .name = "zant", .module = zant_mod }},
        }),
    });
    b.installArtifact(math_demo);
    const run_math = b.addRunArtifact(math_demo);
    run_math.step.dependOn(b.getInstallStep());
    b.step("run-math", "Run the math formula gallery").dependOn(&run_math.step);

    const md_demo = b.addExecutable(.{
        .name = "zant-markdown",
        .root_module = b.createModule(.{
            .root_source_file = b.path("examples/markdown.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
            .link_libcpp = is_macos,
            .imports = &.{.{ .name = "zant", .module = zant_mod }},
        }),
    });
    b.installArtifact(md_demo);
    const run_md = b.addRunArtifact(md_demo);
    run_md.step.dependOn(b.getInstallStep());
    b.step("run-markdown", "Run the Markdown (Streamdown) gallery").dependOn(&run_md.step);

    const stream_demo = b.addExecutable(.{
        .name = "zant-md-stream",
        .root_module = b.createModule(.{
            .root_source_file = b.path("examples/markdown_stream.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
            .link_libcpp = is_macos,
            .imports = &.{.{ .name = "zant", .module = zant_mod }},
        }),
    });
    b.installArtifact(stream_demo);
    const run_stream = b.addRunArtifact(stream_demo);
    run_stream.step.dependOn(b.getInstallStep());
    b.step("run-md-stream", "Streaming Markdown benchmark (chat with history, SSE replay or stdin)").dependOn(&run_stream.step);

    const bench = b.addExecutable(.{
        .name = "zant-bench",
        .root_module = b.createModule(.{
            .root_source_file = b.path("examples/bench.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
            .link_libcpp = is_macos,
            .imports = &.{.{ .name = "zant", .module = zant_mod }},
        }),
    });
    b.installArtifact(bench);
    const run_bench = b.addRunArtifact(bench);
    run_bench.step.dependOn(b.getInstallStep());
    b.step("bench", "Headless frame benchmark (open, stream, typing, caret, hover, resize, scroll)").dependOn(&run_bench.step);

    const pdf_demo = b.addExecutable(.{
        .name = "zant-pdf",
        .root_module = b.createModule(.{
            .root_source_file = b.path("examples/pdf.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
            .link_libcpp = is_macos,
            .imports = &.{.{ .name = "zant", .module = zant_mod }},
        }),
    });
    b.installArtifact(pdf_demo);
    const run_pdf = b.addRunArtifact(pdf_demo);
    run_pdf.step.dependOn(b.getInstallStep());
    run_pdf.addPassthruArgs();
    b.step("run-pdf", "Run the PDF preview example (zig build run-pdf -- file.pdf)").dependOn(&run_pdf.step);

    const components_demo = b.addExecutable(.{
        .name = "zant-components",
        .root_module = b.createModule(.{
            .root_source_file = b.path("examples/components.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
            .link_libcpp = is_macos,
            .imports = &.{.{ .name = "zant", .module = zant_mod }},
        }),
    });
    b.installArtifact(components_demo);
    const run_components = b.addRunArtifact(components_demo);
    run_components.step.dependOn(b.getInstallStep());
    run_components.addPassthruArgs();
    b.step("run-components", "Run the all-components example").dependOn(&run_components.step);

    const tsx_counter = addTsx(b, tsx_tool, b.path("examples/tsx/counter.tsx"), "counter.zig", zant_mod);
    const tsx_demo = b.addExecutable(.{
        .name = "zant-tsx-counter",
        .root_module = b.createModule(.{
            .root_source_file = b.path("examples/tsx/main.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
            .link_libcpp = is_macos,
            .imports = &.{ .{ .name = "zant", .module = zant_mod }, .{ .name = "counter_tsx", .module = tsx_counter } },
        }),
    });
    b.installArtifact(tsx_demo);
    const run_tsx = b.addRunArtifact(tsx_demo);
    run_tsx.step.dependOn(b.getInstallStep());
    run_tsx.addPassthruArgs();
    b.step("run-tsx", "Run the TSX counter example (examples/tsx/counter.tsx)").dependOn(&run_tsx.step);

    const tsx_gallery = addTsx(b, tsx_tool, b.path("examples/tsx/gallery.tsx"), "gallery.zig", zant_mod);
    const tsx_gallery_demo = b.addExecutable(.{
        .name = "zant-tsx-gallery",
        .root_module = b.createModule(.{
            .root_source_file = b.path("examples/tsx/gallery_main.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
            .link_libcpp = is_macos,
            .imports = &.{ .{ .name = "zant", .module = zant_mod }, .{ .name = "gallery_tsx", .module = tsx_gallery } },
        }),
    });
    b.installArtifact(tsx_gallery_demo);
    const run_tsx_gallery = b.addRunArtifact(tsx_gallery_demo);
    run_tsx_gallery.step.dependOn(b.getInstallStep());
    run_tsx_gallery.addPassthruArgs();
    b.step("run-tsx-gallery", "Run the TSX component gallery (examples/tsx/gallery.tsx)").dependOn(&run_tsx_gallery.step);

    const tsx_todos = addTsx(b, tsx_tool, b.path("examples/tsx/todos.tsx"), "todos.zig", zant_mod);
    const tsx_todos_demo = b.addExecutable(.{
        .name = "zant-tsx-todos",
        .root_module = b.createModule(.{
            .root_source_file = b.path("examples/tsx/todos_main.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
            .link_libcpp = is_macos,
            .imports = &.{ .{ .name = "zant", .module = zant_mod }, .{ .name = "todos_tsx", .module = tsx_todos } },
        }),
    });
    b.installArtifact(tsx_todos_demo);
    const run_tsx_todos = b.addRunArtifact(tsx_todos_demo);
    run_tsx_todos.step.dependOn(b.getInstallStep());
    run_tsx_todos.addPassthruArgs();
    b.step("run-tsx-todos", "Run the TSX todo app (examples/tsx/todos.tsx)").dependOn(&run_tsx_todos.step);

    const tsx_reader = addTsx(b, tsx_tool, b.path("examples/tsx/reader.tsx"), "reader.zig", zant_mod);
    const tsx_reader_demo = b.addExecutable(.{
        .name = "zant-tsx-reader",
        .root_module = b.createModule(.{
            .root_source_file = b.path("examples/tsx/reader_main.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
            .link_libcpp = is_macos,
            .imports = &.{ .{ .name = "zant", .module = zant_mod }, .{ .name = "reader_tsx", .module = tsx_reader } },
        }),
    });
    b.installArtifact(tsx_reader_demo);
    const run_tsx_reader = b.addRunArtifact(tsx_reader_demo);
    run_tsx_reader.step.dependOn(b.getInstallStep());
    run_tsx_reader.addPassthruArgs();
    b.step("run-tsx-reader", "Run the TSX Markdown and PDF reader").dependOn(&run_tsx_reader.step);

    // a TS + TSX app built the way `zant create` (language ts) builds one
    const tsx_app_mod = tsxApp(b, tsx_tool, zant_mod, .{
        .src = "examples/tsx/app",
        .target = target,
        .optimize = optimize,
    });
    const tsx_app_demo = b.addExecutable(.{ .name = "zant-tsx-app", .root_module = tsx_app_mod });
    b.installArtifact(tsx_app_demo);
    const run_tsx_app = b.addRunArtifact(tsx_app_demo);
    run_tsx_app.step.dependOn(b.getInstallStep());
    run_tsx_app.addPassthruArgs();
    b.step("run-tsx-app", "Run the TS + TSX app example (examples/tsx/app)").dependOn(&run_tsx_app.step);

    const tsx_notes = addTsxProject(b, tsx_tool, &.{
        b.path("examples/tsx/notes/App.tsx"),
        b.path("examples/tsx/notes/NoteCard.tsx"),
        b.path("examples/tsx/notes/format.zig"),
    }, "App", zant_mod);
    const tsx_zig_types = addTsxProject(b, tsx_tool, &.{
        b.path("tests/tsx/zig_types/App.tsx"),
        b.path("tests/tsx/zig_types/devices.zig"),
    }, "App", zant_mod);
    const tsx_media = addTsxProject(b, tsx_tool, &.{
        b.path("tests/tsx/media/App.tsx"),
        b.path("tests/tsx/media/thumbs.zig"),
    }, "App", zant_mod);
    const tsx_wake = addTsxProject(b, tsx_tool, &.{
        b.path("tests/tsx/zig_types/Worker.tsx"),
        b.path("tests/tsx/zig_types/jobs.zig"),
    }, "Worker", zant_mod);
    const tsx_notes_demo = b.addExecutable(.{
        .name = "zant-tsx-notes",
        .root_module = b.createModule(.{
            .root_source_file = b.path("examples/tsx/notes_main.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
            .link_libcpp = is_macos,
            .imports = &.{ .{ .name = "zant", .module = zant_mod }, .{ .name = "notes_tsx", .module = tsx_notes } },
        }),
    });
    b.installArtifact(tsx_notes_demo);
    const run_tsx_notes = b.addRunArtifact(tsx_notes_demo);
    run_tsx_notes.step.dependOn(b.getInstallStep());
    run_tsx_notes.addPassthruArgs();
    b.step("run-tsx-notes", "Run the two-file TSX notes app (examples/tsx/notes/)").dependOn(&run_tsx_notes.step);

    const tsx_list_bench = addTsx(b, tsx_tool, b.path("examples/tsx/list_bench.tsx"), "list_bench.zig", zant_mod);
    const tsx_settings = addTsx(b, tsx_tool, b.path("examples/tsx/settings.tsx"), "settings.zig", zant_mod);
    const tsx_local_bench = addTsx(b, tsx_tool, b.path("examples/tsx/local_bench.tsx"), "local_bench.zig", zant_mod);
    const tsx_markdown_bench = addTsx(
        b,
        tsx_tool,
        b.path("examples/tsx/markdown_bench.tsx"),
        "markdown_bench.zig",
        zant_mod,
    );
    const tsx_bench = b.addExecutable(.{
        .name = "zant-tsx-bench",
        .root_module = b.createModule(.{
            .root_source_file = b.path("examples/tsx/bench.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
            .link_libcpp = is_macos,
            .imports = &.{
                .{ .name = "zant", .module = zant_mod },
                .{ .name = "counter_tsx", .module = tsx_counter },
                .{ .name = "settings_tsx", .module = tsx_settings },
                .{ .name = "todos_tsx", .module = tsx_todos },
                .{ .name = "list_bench_tsx", .module = tsx_list_bench },
                .{ .name = "local_bench_tsx", .module = tsx_local_bench },
                .{ .name = "markdown_bench_tsx", .module = tsx_markdown_bench },
            },
        }),
    });
    b.installArtifact(tsx_bench);
    const run_tsx_bench = b.addRunArtifact(tsx_bench);
    run_tsx_bench.step.dependOn(b.getInstallStep());
    run_tsx_bench.addPassthruArgs();
    b.step("bench-tsx", "Headless TSX click / typing benchmark (examples/tsx/bench.zig)").dependOn(&run_tsx_bench.step);

    const test_mod = b.createModule(.{
        .root_source_file = b.path("tests/all.zig"),
        .target = target,
        .optimize = optimize,
        .link_libc = true,
        .link_libcpp = is_macos,
        .imports = &.{.{ .name = "zant", .module = zant_mod }},
    });
    const tests = b.addTest(.{ .root_module = test_mod });
    const run_tests = b.addRunArtifact(tests);
    const test_step = b.step("test", "Run unit tests");
    test_step.dependOn(&run_tests.step);

    // unit tests inside the framework module (parser, remend, math, ...)
    const lib_tests = b.addTest(.{ .root_module = zant_mod });
    const run_lib_tests = b.addRunArtifact(lib_tests);
    test_step.dependOn(&run_lib_tests.step);

    // the `zant` CLI's own tests (app identifier rules, ...)
    const tool_tests = b.addTest(.{ .root_module = zant_tool.root_module });
    test_step.dependOn(&b.addRunArtifact(tool_tests).step);

    // TSX compiler (docs/tsx.md): its own tests (lexer, parser, codegen,
    // golden files), the tag table against the real ui options, and the
    // generated code compiled and driven through the runtime
    const tsx_tests = b.addRunArtifact(b.addTest(.{ .root_module = tsx_tool.root_module }));
    b.step("test-tsx-compiler", "Run the TSX compiler's unit and golden tests").dependOn(&tsx_tests.step);
    test_step.dependOn(&tsx_tests.step);

    const tsx_tags_tests = b.addRunArtifact(b.addTest(.{ .root_module = b.createModule(.{
        .root_source_file = b.path("tests/tsx_tags_test.zig"),
        .target = target,
        .optimize = optimize,
        .link_libc = true,
        .link_libcpp = is_macos,
        .imports = &.{
            .{ .name = "zant", .module = zant_mod },
            .{ .name = "tsx_tags", .module = b.createModule(.{ .root_source_file = b.path("tools/tsx/tags.zig") }) },
        },
    }) }));
    const tsx_tags_step = b.step("test-tsx-tags", "Check the TSX tag table against the ui options and the source");
    tsx_tags_step.dependOn(&tsx_tags_tests.step);
    tsx_tags_step.dependOn(&check_tags.step);
    test_step.dependOn(&tsx_tags_tests.step);
    test_step.dependOn(&check_tags.step);
    test_step.dependOn(&check_dts.step);

    const tsx_e2e_mod = b.createModule(.{
        .root_source_file = b.path("tests/tsx_all.zig"),
        .target = target,
        .optimize = optimize,
        .link_libc = true,
        .link_libcpp = is_macos,
        .imports = &.{
            .{ .name = "zant", .module = zant_mod },
            .{ .name = "counter_tsx", .module = tsx_counter },
            .{ .name = "notes_tsx", .module = tsx_notes },
            .{ .name = "zig_types_tsx", .module = tsx_zig_types },
            .{ .name = "tsx_app", .module = tsx_app_mod },
            .{ .name = "wake_tsx", .module = tsx_wake },
            .{ .name = "media_tsx", .module = tsx_media },
        },
    });
    // every golden output of the TSX compiler must compile against zant
    for (tsx_golden) |name| {
        tsx_e2e_mod.addImport(b.fmt("tsx_golden_{s}", .{name}), b.createModule(.{
            .root_source_file = b.path(b.fmt("tools/tsx/testdata/{s}.zig", .{name})),
            .imports = &.{.{ .name = "zant", .module = zant_mod }},
        }));
    }
    const tsx_e2e_tests = b.addRunArtifact(b.addTest(.{ .root_module = tsx_e2e_mod }));
    b.step("test-tsx", "Compile the TSX example and golden outputs and drive them").dependOn(&tsx_e2e_tests.step);
    test_step.dependOn(&tsx_e2e_tests.step);

    if (target.result.os.tag == .windows) {
        for ([_]*std.Build.Step.Compile{
            exe,              demo,           math_demo,      md_demo,         pdf_demo, components_demo, tsx_demo,
            tsx_gallery_demo, tsx_todos_demo, tsx_notes_demo, tsx_reader_demo,
        }) |app| {
            app.win32_manifest = b.path("src/platform/windows/zant.manifest");
            if (optimize != .debug) app.subsystem = .windows;
        }
        if (bridge_dir) |dir| installRuntime(b, dir, hasPrebuiltBridge(b), &.{
            run_demo,  run_gallery_cmd, run_math,       run_md,        run_pdf,        run_components,
            run_tsx,   run_tsx_gallery, run_tsx_todos,  run_tsx_notes, run_tsx_reader, run_tsx_bench,
            run_tests, run_lib_tests,   tsx_tags_tests, tsx_e2e_tests,
        });
    }
}
