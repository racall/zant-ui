const std = @import("std");
const zant_build = @import("zant_ui");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});
    const zant_dep = b.dependency("zant_ui", .{ .target = target, .optimize = optimize });
    const app = zant_build.addTsxApp(b, zant_dep, .{ .target = target, .optimize = optimize });
    const exe = b.addExecutable(.{ .name = "app", .root_module = app });
    b.installArtifact(exe);

    const run_cmd = b.addRunArtifact(exe);
    run_cmd.step.dependOn(b.getInstallStep());
    run_cmd.addPassthruArgs();
    b.step("run", "构建并运行应用（推荐使用 zant run）").dependOn(&run_cmd.step);

    // Windows: manifest, runtime DLLs beside app.exe and DLL path for runs.
    zant_build.configureApp(b, zant_dep, exe, &.{run_cmd});
}
