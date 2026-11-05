const std = @import("std");

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
        .files = &.{"src/renderer/skia/skia_bridge.mm"},
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
    };
    for (frameworks) |framework| {
        mod.linkFramework(framework, .{});
    }
    mod.linkSystemLibrary("z", .{ .use_pkg_config = .no });
    mod.linkSystemLibrary("sqlite3", .{ .use_pkg_config = .no });
}

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const zant_mod = b.addModule("zant", .{
        .root_source_file = b.path("src/root.zig"),
        .target = target,
        .optimize = optimize,
        .link_libc = true,
        .link_libcpp = true,
    });
    switch (target.result.os.tag) {
        .macos => configureMacosSkia(b, target, zant_mod),
        else => @panic("Zant UI currently supports macOS only; the Windows backend is not implemented yet."),
    }

    const exe = b.addExecutable(.{
        .name = "zant-gallery",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/main.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
            .link_libcpp = true,
            .imports = &.{.{ .name = "zant", .module = zant_mod }},
        }),
    });

    b.installArtifact(exe);

    const tool_options = b.addOptions();
    tool_options.addOption([]const u8, "zant_root", b.root.root_dir.path orelse ".");
    const zant_tool = b.addExecutable(.{
        .name = "zant",
        .root_module = b.createModule(.{
            .root_source_file = b.path("tools/zant/main.zig"),
            .target = target,
            .optimize = optimize,
            .imports = &.{.{ .name = "build_options", .module = tool_options.createModule() }},
        }),
    });
    b.installArtifact(zant_tool);

    const demo = b.addExecutable(.{
        .name = "zant-demo",
        .root_module = b.createModule(.{
            .root_source_file = b.path("examples/declarative.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
            .link_libcpp = true,
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

    const test_mod = b.createModule(.{
        .root_source_file = b.path("tests/all.zig"),
        .target = target,
        .optimize = optimize,
        .link_libc = true,
        .link_libcpp = true,
        .imports = &.{.{ .name = "zant", .module = zant_mod }},
    });
    const tests = b.addTest(.{ .root_module = test_mod });
    const run_tests = b.addRunArtifact(tests);
    const test_step = b.step("test", "Run unit tests");
    test_step.dependOn(&run_tests.step);
}
