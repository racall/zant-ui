const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;

const plans = [_][]const u8{ "Starter", "Pro", "Enterprise" };
const themes = [_][]const u8{ "Light", "System", "Dark" };

const State = struct {
    name_buf: [96]u8 = undefined,
    name_len: usize = 7,
    notifications: bool = true,
    beta: bool = false,
    theme_choice: usize = 0,
    plan: usize = 1,
    tab: usize = 0,
    saved: u32 = 0,

    fn init() State {
        var state = State{};
        @memcpy(state.name_buf[0..7], "Zant UI");
        return state;
    }

    fn name(self: *const State) []const u8 {
        return self.name_buf[0..self.name_len];
    }

    fn onName(self: *State, value: []const u8) void {
        const n = @min(value.len, self.name_buf.len);
        @memcpy(self.name_buf[0..n], value[0..n]);
        self.name_len = n;
    }

    fn setNotifications(self: *State, checked: bool) void {
        self.notifications = checked;
    }

    fn setBeta(self: *State, checked: bool) void {
        self.beta = checked;
    }

    fn setTheme(self: *State, index: usize) void {
        self.theme_choice = index;
    }

    fn setPlan(self: *State, index: usize) void {
        self.plan = index;
    }

    fn setTab(self: *State, index: usize) void {
        self.tab = index;
    }

    fn save(self: *State) void {
        self.saved += 1;
    }

    fn render(context: *anyopaque) ui.Node {
        const self: *State = @ptrCast(@alignCast(context));
        var plan_options: [plans.len][]const u8 = undefined;
        for (plans, 0..) |plan, i| plan_options[i] = plan;
        var theme_options: [themes.len]ui.RadioOption = undefined;
        for (themes, 0..) |theme_name, i| theme_options[i] = .{ .label = theme_name };

        const overview = ui.column(.{
            .gap = 14,
            .align_items = .start,
            .children = &.{
                ui.tooltip(.{
                    .text = "Tooltip is rendered by Skia above the UI tree",
                    .child = ui.button(.{ .text = "Hover me", .variant = .default }),
                }),
                ui.image(.{
                    .src = "examples/assets/zant.png",
                    .width = 280,
                    .height = 140,
                    .fit = .cover,
                    .radius = 10,
                }),
                ui.text("Live preview", .{ .size = 16, .weight = .semibold }),
                ui.text(self.name(), .{ .size = 22, .weight = .semibold, .color = 0xff1677ff }),
                ui.paragraph("A declarative UI tree built with ui.column, ui.input, ui.radioGroup, ui.select, ui.tabs and ui.image.", .{}),
            },
        });

        const details = ui.column(.{
            .gap = 10,
            .children = &.{
                ui.text("Component state", .{ .size = 16, .weight = .semibold }),
                ui.text("Notifications", .{ .size = 13, .color = 0xa6000000 }),
                ui.text(if (self.notifications) "On" else "Off", .{ .size = 13 }),
                ui.text("Beta updates", .{ .size = 13, .color = 0xa6000000 }),
                ui.text(if (self.beta) "Enabled" else "Disabled", .{ .size = 13 }),
                ui.text("Theme", .{ .size = 13, .color = 0xa6000000 }),
                ui.text(themes[self.theme_choice], .{ .size = 13 }),
                ui.text("Plan", .{ .size = 13, .color = 0xa6000000 }),
                ui.text(plans[self.plan], .{ .size = 13 }),
                ui.text("Saves", .{ .size = 13, .color = 0xa6000000 }),
                ui.textFmt("{}", .{self.saved}, .{ .size = 13 }),
            },
        });

        var tab_items = [_]ui.TabItem{
            .{ .label = "Overview", .content = overview },
            .{ .label = "Details", .content = details },
        };

        const left_card = ui.card(.{
            .title = "Settings",
            .width = 486,
            .children = &.{
                ui.text("Project name", .{ .size = 12, .color = 0xa6000000 }),
                ui.input(.{
                    .value = self.name(),
                    .placeholder = "Project name",
                    .on_change = ui.textAction(self, State.onName),
                }),
                ui.space(.{
                    .gap = 20,
                    .children = &.{
                        ui.checkbox(.{
                            .label = "Notifications",
                            .checked = self.notifications,
                            .on_change = ui.boolAction(self, State.setNotifications),
                        }),
                        ui.@"switch"(.{
                            .label = "Beta updates",
                            .checked = self.beta,
                            .on_change = ui.boolAction(self, State.setBeta),
                        }),
                    },
                }),
                ui.divider(.{ .margin = ui.insetXY(0, 4) }),
                ui.text("Theme", .{ .size = 13, .weight = .medium }),
                ui.radioGroup(.{
                    .key = "theme",
                    .options = &theme_options,
                    .selected = self.theme_choice,
                    .direction = .row,
                    .gap = 20,
                    .on_change = ui.indexAction(self, State.setTheme),
                }),
                ui.text("Plan", .{ .size = 13, .weight = .medium }),
                ui.select(.{
                    .options = &plan_options,
                    .selected = self.plan,
                    .width = 220,
                    .on_change = ui.indexAction(self, State.setPlan),
                }),
                ui.space(.{
                    .gap = 10,
                    .children = &.{
                        ui.button(.{
                            .text = "Save Changes",
                            .variant = .primary,
                            .on_click = ui.action(self, State.save),
                        }),
                        ui.button(.{ .text = "Cancel", .variant = .default }),
                    },
                }),
            },
        });

        const right_card = ui.card(.{
            .title = "Preview",
            .width = 326,
            .children = &.{
                ui.tabs(.{
                    .key = "preview-tabs",
                    .items = &tab_items,
                    .active = self.tab,
                    .on_change = ui.indexAction(self, State.setTab),
                }),
                ui.alert(.{
                    .type = .info,
                    .title = "Declarative components",
                    .description = "State changes reconcile only the affected nodes; editing an input does not rebuild layout.",
                }),
            },
        });

        return ui.column(.{
            .padding = .{ .left = 32, .right = 32, .top = 28, .bottom = 28 },
            .gap = 18,
            .children = &.{
                ui.text("Zant UI", .{ .size = 30, .weight = .semibold }),
                ui.text("Declarative native components powered by Zig + Skia", .{
                    .size = 14,
                    .color = 0xa6000000,
                }),
                ui.row(.{
                    .align_items = .start,
                    .gap = 18,
                    .children = &.{ left_card, right_card },
                }),
            },
        });
    }
};

pub fn main(init: std.process.Init) !void {
    var state = State.init();
    const runtime = try Runtime.init(init.gpa, &state, State.render, .{
        .title = "Zant UI — Declarative macOS",
        .window = .{ .width = 900, .height = 700 },
        .theme = zant.tokens.antLight(),
    });
    defer runtime.deinit();
    try runtime.run();
}
