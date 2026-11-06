const std = @import("std");
const zant_build = @import("zant_ui");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});
    // Skia's C++ is linked statically on macOS; on Windows it lives in
    // zant_skia.dll, so only macOS needs libc++
    const libcpp = target.result.os.tag == .macos;

    const zant_dep = b.dependency("zant_ui", .{
        .target = target,
        .optimize = optimize,
    });
    const zant = zant_dep.module("zant");

    const exe = b.addExecutable(.{
        .name = "app",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/main.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
            .link_libcpp = libcpp,
            .imports = &.{.{ .name = "zant", .module = zant }},
        }),
    });
    b.installArtifact(exe);

    const run_cmd = b.addRunArtifact(exe);
    run_cmd.step.dependOn(b.getInstallStep());
    run_cmd.addPassthruArgs();
    const run_step = b.step("run", "构建并运行应用（推荐使用 zant run）");
    run_step.dependOn(&run_cmd.step);

    const tests = b.addTest(.{ .root_module = b.createModule(.{
        .root_source_file = b.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
        .link_libc = true,
        .link_libcpp = libcpp,
        .imports = &.{.{ .name = "zant", .module = zant }},
    }) });
    const run_tests = b.addRunArtifact(tests);
    const test_step = b.step("test", "运行单元测试");
    test_step.dependOn(&run_tests.step);

    // Windows: manifest, zant_skia.dll next to app.exe, DLL path for runs
    // (a no-op on macOS)
    zant_build.configureApp(b, zant_dep, exe, &.{ run_cmd, run_tests });
}
