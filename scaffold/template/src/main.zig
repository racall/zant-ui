const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const cfg = @import("app_config.zig");
const AppState = @import("state.zig").AppState;
const dashboard = @import("components/dashboard.zig");

// 界面入口。状态、回调和布局都在这个文件；组件在 src/components/ 下。
fn render(self: *AppState) ui.Node {
    return ui.column(.{
        .padding = .{ .left = 24, .right = 24, .top = __TOP_PADDING__, .bottom = 24 },
        .gap = 14,
        .children = &.{
            ui.heading(cfg.title, .{ .level = 2 }),
            ui.paragraph("修改 main.zig、state.zig 和 components/ 试试。", .{}),
            ui.input(.{
                .value = self.name(),
                .placeholder = "项目名称",
                .on_change = ui.textAction(self, AppState.onName),
            }),
            // 父组件读取同一份状态，证明兄弟组件共享数据。
            ui.textFmt("父组件读取的点击次数：{d}", .{self.clicks}, .{
                .color = 0xa6000000,
            }),
            // 自定义父组件：内部再组合两个自定义子组件。
            dashboard.dashboard(self),
        },
    });
}

pub fn main(init: std.process.Init) !void {
    // app.config.json 提供默认窗口配置。
    // 代码中的 WindowOptions 是最终值，优先级更高。
    const window = cfg.window;
    // window.always_on_top = true;

    try zant.launch.run(init.gpa, AppState.init(cfg.title), render, .{
        .title = cfg.title,
        .window = window,
        .theme = zant.tokens.antLight(),
    });
}

test "状态初始为空" {
    const state = AppState.init("Test");
    try std.testing.expectEqual(@as(u32, 0), state.clicks);
    try std.testing.expectEqualStrings("Test", state.name());
}
