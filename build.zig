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
        "AppKit",       "Metal", "MetalKit", "QuartzCore", "CoreText",               "CoreFoundation",
        "CoreGraphics", "IOKit", "Security", "CoreVideo",  "UniformTypeIdentifiers", "PDFKit",
        "Carbon",
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
    installRuntime(b, zant_dep.namedLazyPath("zant_skia"), runs);
}

/// App icons in `assets/icons/` (created by `zant create`, replaceable).
/// Each one that exists is importable by name: `@embedFile("icon_tray")`.
pub const app_icons = [_]struct { file: []const u8, import: []const u8 }{
    .{ .file = "assets/icons/app-macos.png", .import = "icon_app_macos" }, // 1024×1024, Apple icon grid
    .{ .file = "assets/icons/app-windows.png", .import = "icon_app_windows" }, // 256×256, full bleed
    .{ .file = "assets/icons/tray.png", .import = "icon_tray" }, // Windows notification area
    .{ .file = "assets/icons/statusbar.png", .import = "icon_statusbar" }, // macOS menu bar template
};

fn appFileExists(b: *std.Build, sub_path: []const u8) bool {
    const full = b.root.joinString(b.allocator, sub_path) catch return false;
    std.Io.Dir.cwd().access(b.graph.io, full, .{}) catch return false;
    return true;
}

fn addAppIcons(b: *std.Build, module: *std.Build.Module) void {
    for (app_icons) |icon| {
        if (appFileExists(b, icon.file)) module.addAnonymousImport(icon.import, .{ .root_source_file = b.path(icon.file) });
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

fn installRuntime(b: *std.Build, dir: std.Build.LazyPath, runs: []const *std.Build.Step.Run) void {
    const runtime = b.step("zant-runtime", "Install zant_skia.dll and pdfium.dll");
    for ([_][]const u8{ "zant_skia.dll", "pdfium.dll" }) |name| {
        runtime.dependOn(&b.addInstallBinFile(dir.path(b, name), name).step);
    }
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

    if (target.result.os.tag == .windows) {
        for ([_]*std.Build.Step.Compile{ exe, demo, math_demo, md_demo, pdf_demo, components_demo }) |app| {
            app.win32_manifest = b.path("src/platform/windows/zant.manifest");
            if (optimize != .debug) app.subsystem = .windows;
        }
        if (bridge_dir) |dir| installRuntime(b, dir, &.{ run_demo, run_gallery_cmd, run_math, run_md, run_pdf, run_components, run_tests, run_lib_tests });
    }
}
