const std = @import("std");
const zant_build = @import("zant_ui");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});
    const zant_dep = b.dependency("zant_ui", .{ .target = target, .optimize = optimize });
    // src/ 下的 TSX，以及 native/ 下的原生模块（native/lib.zig）
    const app = zant_build.addTsxApp(b, zant_dep, .{ .target = target, .optimize = optimize });
    const exe = b.addExecutable(.{ .name = "app", .root_module = app });
    b.installArtifact(exe);

    const run_cmd = b.addRunArtifact(exe);
    run_cmd.step.dependOn(b.getInstallStep());
    run_cmd.addPassthruArgs();
    b.step("run", "构建并运行应用（推荐使用 zant run）").dependOn(&run_cmd.step);

    // 原生模块的单元测试（native/lib.zig 引用到的 Zig 文件）
    const test_step = b.step("test", "运行原生模块的单元测试");
    var runs: [2]*std.Build.Step.Run = .{ run_cmd, undefined };
    var run_count: usize = 1;
    if (zant_build.tsxNative(app)) |native| {
        runs[1] = b.addRunArtifact(b.addTest(.{ .root_module = native }));
        test_step.dependOn(&runs[1].step);
        run_count = 2;
    }

    // Windows: manifest, runtime DLLs beside app.exe and DLL path for runs.
    zant_build.configureApp(b, zant_dep, exe, runs[0..run_count]);
}
