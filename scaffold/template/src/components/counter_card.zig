//! 自定义子组件：计数器卡片。
//!
//! 自己不保存状态，只接收父组件传入的 props，并通过回调把事件传给父组件。
//! 这是「props 向下、事件向上」的最小示例。

const zant = @import("zant");
const ui = zant.ui;

pub const Options = struct {
    count: u32,
    on_increment: ?zant.actions.Action = null,
    on_reset: ?zant.actions.Action = null,
};

pub fn counterCard(options: Options) ui.Node {
    return ui.card(.{
        .title = "跨组件状态",
        .extra = "props ↓ / events ↑",
        .children = &.{
            ui.textFmt("点击次数：{d}", .{options.count}, .{
                .size = 28,
                .weight = .semibold,
                .color = 0xff1677ff,
            }),
            ui.paragraph("状态在父组件 AppState 中；子组件只发事件。", .{}),
            ui.row(.{
                .gap = 8,
                .children = &.{
                    ui.button(.{
                        .text = "+1",
                        .variant = .primary,
                        .on_click = options.on_increment,
                    }),
                    ui.button(.{
                        .text = "重置",
                        .on_click = options.on_reset,
                    }),
                },
            }),
        },
    });
}
