//! 自定义父组件：接收 AppState，把值拆成 props 传给子组件。
//!
//! 事件通过 AppState 的方法回调向上传递。子组件不直接依赖 State 的内部实现。

const zant = @import("zant");
const ui = zant.ui;
const AppState = @import("../state.zig").AppState;
const stat_card = @import("stat_card.zig");
const counter_card = @import("counter_card.zig");

pub fn dashboard(state: *AppState) ui.Node {
    return ui.column(.{
        .gap = 12,
        .children = &.{
            ui.row(.{
                .gap = 12,
                .children = &.{
                    stat_card.statCard(.{
                        .label = "构建次数",
                        .value = "128",
                        .hint = "父组件传给子组件的 props",
                    }),
                    stat_card.statCard(.{
                        .label = "组件",
                        .value = "12",
                        .hint = "由 ui.* 组合而成",
                        .accent = 0xff52c41a,
                    }),
                },
            }),
            counter_card.counterCard(.{
                .count = state.clicks,
                .on_increment = ui.action(state, AppState.increment),
                .on_reset = ui.action(state, AppState.reset),
            }),
        },
    });
}
