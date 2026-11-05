//! 自定义示例组件：StatCard
//!
//! 这个组件完全由 Zant UI 的基础组件组合而成，展示了业务项目如何封装
//! 自己的可复用 UI。它没有任何平台相关代码。

const zant = @import("zant");
const ui = zant.ui;

pub const Options = struct {
    label: []const u8,
    value: []const u8,
    hint: []const u8 = "",
    accent: u32 = 0xff1677ff,
};

pub fn statCard(options: Options) ui.Node {
    var children: [3]ui.Node = undefined;
    var count: usize = 0;

    children[count] = ui.text(options.label, .{
        .size = 12,
        .color = 0xa6000000,
    });
    count += 1;

    children[count] = ui.text(options.value, .{
        .size = 28,
        .weight = .semibold,
        .color = options.accent,
    });
    count += 1;

    if (options.hint.len > 0) {
        children[count] = ui.text(options.hint, .{
            .size = 12,
            .color = 0xa6000000,
        });
        count += 1;
    }

    return ui.card(.{
        .width = 248,
        .height = 150,
        .gap = 6,
        .padding = ui.inset(16),
        .children = children[0..count],
    });
}
