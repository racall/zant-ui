//! Every `ui.*` component in one window: `zig build run-components`.
//!
//! Each interaction prints `[event] ...` to stderr and shows it under the
//! title, which `scripts/win/components-check.ps1` uses (with
//! ZANT_LAYOUT_FILE) to click every component and check the result.
//! ZANT_DARK=1 starts in dark mode. ZANT_AUTOPLAY=1 cycles through the
//! animated states by itself (menu, modal, message, theme, tabs...) so frame
//! rates can be measured without input, e.g. with ZANT_FRAME_STATS=1.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;

const State = struct {
    dark: bool = false,
    last_event: [96]u8 = undefined,
    last_len: usize = 0,
    notifications: bool = true,
    beta: bool = false,
    plan: usize = 0,
    region: usize = 0,
    view: usize = 0,
    tab: usize = 0,
    progress: f32 = 40,
    menu_open: bool = false,
    modal_open: bool = false,
    message_until_ms: i64 = 0,
    autoplay: bool = false,
    /// ZANT_AUTOPLAY=quiet: no `[event]` lines (stderr writes are slow on Windows)
    quiet: bool = false,
    autoplay_step: usize = 0,
    autoplay_at_ms: i64 = 0,

    /// One state change every 700 ms, so each enter / exit animation runs
    /// to the end before the next one starts.
    fn autoplayTick(self: *State) void {
        const now = ui.nowMs();
        if (self.autoplay_at_ms == 0) self.autoplay_at_ms = now + 700;
        if (now >= self.autoplay_at_ms) {
            switch (self.autoplay_step % 12) {
                0 => self.openMenu(),
                1 => self.closeMenu(),
                2 => self.openModal(),
                3 => self.closeModal(),
                4 => self.showMessage(),
                5 => self.toggleDark(),
                6 => self.onTab(1),
                7 => self.onView(2),
                8 => self.moreProgress(),
                9 => self.toggleDark(),
                10 => self.onTab(0),
                else => self.onView(0),
            }
            self.autoplay_step += 1;
            self.autoplay_at_ms = now + 700;
        }
        ui.wakeAfter(@intCast(@max(1, self.autoplay_at_ms - now)));
    }

    fn event(self: *State, comptime fmt: []const u8, args: anytype) void {
        const text = std.fmt.bufPrint(&self.last_event, fmt, args) catch return;
        self.last_len = text.len;
        if (!self.quiet) std.debug.print("[event] " ++ fmt ++ "\n", args);
    }

    fn clickPrimary(self: *State) void {
        self.event("click primary", .{});
    }
    fn clickDefault(self: *State) void {
        self.event("click default", .{});
    }
    fn clickText(self: *State) void {
        self.event("click text", .{});
    }
    fn clickLinkButton(self: *State) void {
        self.event("click link-button", .{});
    }
    fn clickDisabled(self: *State) void {
        self.event("click disabled", .{});
    }
    fn clickIcon(self: *State) void {
        self.event("click icon", .{});
    }
    fn clickLink(self: *State) void {
        self.event("click link", .{});
    }
    fn onNotify(self: *State, value: bool) void {
        self.notifications = value;
        self.event("checkbox {}", .{value});
    }
    fn onBeta(self: *State, value: bool) void {
        self.beta = value;
        self.event("switch {}", .{value});
    }
    fn onPlan(self: *State, index: usize) void {
        self.plan = index;
        self.event("radio {d}", .{index});
    }
    fn onRegion(self: *State, index: usize) void {
        self.region = index;
        self.event("select {d}", .{index});
    }
    fn onView(self: *State, index: usize) void {
        self.view = index;
        self.event("segmented {d}", .{index});
    }
    fn onTab(self: *State, index: usize) void {
        self.tab = index;
        self.event("tab {d}", .{index});
    }
    fn onInput(self: *State, text: []const u8) void {
        self.event("input {s}", .{text});
    }
    fn onSubmit(self: *State) void {
        self.event("submit", .{});
    }
    fn moreProgress(self: *State) void {
        self.progress = if (self.progress >= 100) 0 else self.progress + 20;
        self.event("progress {d}", .{@as(u32, @intFromFloat(self.progress))});
    }
    fn toggleDark(self: *State) void {
        self.dark = !self.dark;
        self.event("dark {}", .{self.dark});
    }
    fn openMenu(self: *State) void {
        self.menu_open = true;
        self.event("menu open", .{});
    }
    fn closeMenu(self: *State) void {
        self.menu_open = false;
        self.event("menu dismiss", .{});
    }
    fn menuNew(self: *State) void {
        self.menu_open = false;
        self.event("menu new", .{});
    }
    fn menuDelete(self: *State) void {
        self.menu_open = false;
        self.event("menu delete", .{});
    }
    fn openModal(self: *State) void {
        self.modal_open = true;
        self.event("modal open", .{});
    }
    fn closeModal(self: *State) void {
        self.modal_open = false;
        self.event("modal close", .{});
    }
    fn showMessage(self: *State) void {
        // antd's default message duration
        self.message_until_ms = ui.nowMs() + 3000;
        self.event("message show", .{});
    }

    pub fn theme(self: *State) zant.tokens.Theme {
        return if (self.dark) zant.tokens.antDark() else zant.tokens.antLight();
    }

    pub fn onKey(self: *State, key: zant.events.KeyEvent) bool {
        if (key.key == .escape and (self.menu_open or self.modal_open)) {
            self.menu_open = false;
            self.modal_open = false;
            self.event("escape", .{});
            return true;
        }
        return false;
    }
};

fn section(title: []const u8, children: []const ui.Node) ui.Node {
    return ui.card(.{ .title = title, .gap = 12, .padding = .all(16), .children = children });
}

fn label(text: []const u8) ui.Node {
    return ui.text(text, .{ .size = 12, .color = ui.currentTheme().color_text_secondary });
}

const shortcut_groups = [_]ui.ShortcutGroup{
    .{ .title = "文件", .items = &.{
        .{ .label = "保存", .keys = zant.keys.label("mod+s") },
        .{ .label = "另存为", .keys = zant.keys.label("mod+shift+s") },
    } },
    .{ .title = "编辑", .items = &.{
        .{ .label = "撤销 / 重做", .keys = zant.keys.pick(zant.keys.label("mod+z mod+shift+z"), zant.keys.label("mod+z mod+y")) },
    } },
};

fn render(s: *State) ui.Node {
    if (s.autoplay) s.autoplayTick();
    const arena = ui.frameAllocator();
    const theme = ui.currentTheme();

    const buttons = section("按钮 Button / IconButton / Link", &.{
        ui.row(.{ .gap = 8, .align_items = .center, .children = &.{
            ui.button(.{ .key = "btn-primary", .text = "Primary", .variant = .primary, .on_click = ui.action(s, State.clickPrimary) }),
            ui.button(.{ .key = "btn-default", .text = "Default", .on_click = ui.action(s, State.clickDefault) }),
            ui.button(.{ .key = "btn-text", .text = "Text", .variant = .text, .on_click = ui.action(s, State.clickText) }),
            ui.button(.{ .key = "btn-link", .text = "Link", .variant = .link, .on_click = ui.action(s, State.clickLinkButton) }),
            ui.button(.{ .key = "btn-disabled", .text = "Disabled", .disabled = true, .on_click = ui.action(s, State.clickDisabled) }),
        } }),
        ui.row(.{ .gap = 8, .align_items = .center, .children = &.{
            ui.button(.{ .key = "btn-small", .text = "Small", .size = .small, .variant = .primary, .on_click = ui.action(s, State.clickPrimary) }),
            ui.button(.{ .key = "btn-large", .text = "Large", .size = .large, .on_click = ui.action(s, State.clickDefault) }),
            ui.iconButton(.{ .key = "icon-btn", .icon = .search, .tooltip = "搜索", .on_click = ui.action(s, State.clickIcon) }),
            ui.link(.{ .key = "link", .text = "文档链接", .on_click = ui.action(s, State.clickLink) }),
        } }),
    });

    const form = section("表单 Input / Checkbox / Switch / Radio / Select", &.{
        ui.input(.{ .key = "input", .placeholder = "输入文字后回车", .width = 320, .on_change = ui.textAction(s, State.onInput), .on_submit = ui.action(s, State.onSubmit) }),
        ui.input(.{ .key = "input-password", .placeholder = "密码", .password = true, .width = 320 }),
        ui.row(.{ .gap = 24, .align_items = .center, .children = &.{
            ui.checkbox(.{ .key = "checkbox", .label = "通知", .checked = s.notifications, .on_change = ui.boolAction(s, State.onNotify) }),
            ui.@"switch"(.{ .key = "switch", .label = "测试版", .checked = s.beta, .on_change = ui.boolAction(s, State.onBeta) }),
        } }),
        ui.radioGroup(.{ .key = "radio", .direction = .row, .gap = 16, .selected = s.plan, .options = &.{
            .{ .label = "基础" },
            .{ .label = "专业" },
            .{ .label = "企业", .disabled = true },
        }, .on_change = ui.indexAction(s, State.onPlan) }),
        ui.select(.{ .key = "select", .width = 200, .options = &.{ "华东", "华北", "华南" }, .selected = s.region, .on_change = ui.indexAction(s, State.onRegion) }),
    });

    const nav = section("导航 Segmented / Tabs", &.{
        ui.segmented(.{ .key = "segmented", .selected = s.view, .on_change = ui.indexAction(s, State.onView), .options = &.{
            .{ .icon = .pencil, .label = "编辑" },
            .{ .icon = .columns, .label = "分栏" },
            .{ .icon = .book_open, .label = "预览" },
        } }),
        ui.tabs(.{ .key = "tabs", .active = s.tab, .on_change = ui.indexAction(s, State.onTab), .items = &.{
            .{ .label = "概览", .content = ui.text("第一个标签页的内容", .{}) },
            .{ .label = "成员", .content = ui.text("第二个标签页的内容", .{}) },
        } }),
    });

    const percent_text = std.fmt.allocPrint(arena, "{d}%", .{@as(u32, @intFromFloat(s.progress))}) catch "";
    const display = section("展示 Tag / Avatar / Progress / Icon / Image / Tooltip", &.{
        ui.row(.{
            .gap = 8,
            .align_items = .center,
            .children = &.{
                ui.tag(.{ .text = "默认" }),
                // antd green / orange presets, light and dark
                ui.tag(.{ .text = "成功", .color = if (s.dark) 0xff49aa19 else 0xff389e0d, .background = if (s.dark) 0xff162312 else 0xfff6ffed }),
                ui.tag(.{ .text = "警告", .color = if (s.dark) 0xffd89614 else 0xffd46b08, .background = if (s.dark) 0xff2b1d11 else 0xfffff7e6 }),
                ui.avatar(.{ .text = "Z" }),
                ui.avatar(.{ .text = "UI", .size = 40, .background = theme.color_primary }),
            },
        }),
        ui.row(.{ .gap = 12, .align_items = .center, .children = &.{
            ui.progress(.{ .key = "progress", .percent = s.progress, .width = 220 }),
            ui.text(percent_text, .{ .size = 12 }),
            ui.button(.{ .key = "progress-more", .text = "+20%", .size = .small, .on_click = ui.action(s, State.moreProgress) }),
        } }),
        ui.row(.{ .gap = 10, .align_items = .center, .children = &.{
            ui.icon(.{ .name = .search }), ui.icon(.{ .name = .settings }), ui.icon(.{ .name = .folder }),
            ui.icon(.{ .name = .save }),   ui.icon(.{ .name = .copy }),     ui.icon(.{ .name = .trash }),
            ui.icon(.{ .name = .sun }),    ui.icon(.{ .name = .moon }),     ui.icon(.{ .name = .info }),
            ui.icon(.{ .name = .alert }),  ui.icon(.{ .name = .check }),    ui.icon(.{ .name = .close }),
        } }),
        ui.row(.{ .gap = 12, .align_items = .center, .children = &.{
            ui.image(.{ .key = "image", .src = "examples/assets/zant.png", .width = 120, .height = 68, .tooltip = "图片提示" }),
            ui.tooltip(.{ .text = "这是一个提示", .child = ui.button(.{ .key = "tooltip-btn", .text = "悬停查看提示" }) }),
        } }),
    });

    const feedback = section("反馈 Alert / Dropdown / Modal / Message", &.{
        ui.alert(.{ .type = .info, .title = "信息", .description = "普通提示信息" }),
        ui.alert(.{ .type = .success, .title = "成功", .description = "操作已完成" }),
        ui.alert(.{ .type = .warning, .title = "警告" }),
        ui.alert(.{ .type = .error_state, .title = "错误", .description = "出现问题" }),
        ui.row(.{ .gap = 8, .children = &.{
            ui.button(.{ .key = "menu-btn", .text = "下拉菜单", .on_click = ui.action(s, State.openMenu) }),
            ui.button(.{ .key = "modal-btn", .text = "对话框", .on_click = ui.action(s, State.openModal) }),
            ui.button(.{ .key = "message-btn", .text = "全局提示", .on_click = ui.action(s, State.showMessage) }),
            ui.button(.{ .key = "dark-btn", .text = if (s.dark) "浅色" else "深色", .on_click = ui.action(s, State.toggleDark) }),
        } }),
    });

    var children: std.ArrayListUnmanaged(ui.Node) = .empty;
    children.append(arena, ui.column(.{
        .key = "page",
        .grow = 1,
        .scroll = true,
        .align_items = .stretch,
        .padding = .{ .left = 24, .right = 24, .top = 20, .bottom = 24 },
        .gap = 16,
        .children = &.{
            ui.row(.{ .gap = 12, .align_items = .center, .children = &.{
                ui.text("组件总览", .{ .size = 20, .weight = .semibold }),
                ui.text(if (s.last_len > 0) s.last_event[0..s.last_len] else "（点击任意组件）", .{ .key = "last-event", .size = 12, .color = theme.color_text_secondary }),
            } }),
            ui.row(.{ .gap = 16, .align_items = .start, .children = &.{
                ui.column(.{ .grow = 1, .basis = 0, .gap = 16, .align_items = .stretch, .children = &.{ buttons, form, nav } }),
                ui.column(.{ .grow = 1, .basis = 0, .gap = 16, .align_items = .stretch, .children = &.{ display, feedback } }),
            } }),
        },
    })) catch {};

    if (s.menu_open) {
        children.append(arena, ui.dismissLayer(.{ .on_dismiss = ui.action(s, State.closeMenu) })) catch {};
        // below its button (antd placement bottomLeft), above it when the
        // window has no room; boxes come from the previous frame's layout
        const menu_w: f32 = 200;
        const measured = ui.keyBounds("menu");
        const menu_h: f32 = if (measured) |m| m.height else 160;
        // first frame: place with an estimate, rebuild once laid out
        if (measured == null) ui.wakeAfter(1);
        var left: f32 = 40;
        var top: f32 = 120;
        var above = false;
        if (ui.keyBounds("menu-btn")) |b| {
            const page_h: f32 = if (ui.keyBounds("page")) |page| page.height else 820;
            left = b.x;
            above = b.bottom() + 4 + menu_h > page_h - 8;
            top = if (above) b.y - 4 - menu_h else b.bottom() + 4;
        }
        children.append(arena, ui.styled(ui.dropdownMenu(.{ .key = "menu", .width = menu_w, .motion_origin = if (above) .bottom_left else .top_left, .items = &.{
            ui.MenuItem.group("文件"),
            .{ .key = "menu-new", .label = "新建", .icon = .file, .extra = zant.keys.label("mod+n"), .on_click = ui.action(s, State.menuNew) },
            .{ .key = "menu-disabled", .label = "不可用", .icon = .settings, .disabled = true },
            ui.MenuItem.divider,
            .{ .key = "menu-delete", .label = "删除", .icon = .trash, .danger = true, .on_click = ui.action(s, State.menuDelete) },
        } }), .{ .absolute = true, .top = top, .left = left })) catch {};
    }
    if (s.modal_open) {
        children.append(arena, ui.modal(.{
            .key = "modal",
            .title = "键盘快捷键",
            .width = null,
            .on_close = ui.action(s, State.closeModal),
            .content = ui.shortcutList(.{ .key = "modal-shortcuts", .groups = &shortcut_groups }),
            .footer = ui.button(.{ .key = "modal-ok", .text = "知道了", .variant = .primary, .on_click = ui.action(s, State.closeModal) }),
        })) catch {};
    }
    const now = ui.nowMs();
    if (now < s.message_until_ms) {
        children.append(arena, ui.message(.{ .key = "message", .text = "已保存", .kind = .success })) catch {};
        // the earlier of the autoplay step and the message expiry wins
        ui.wakeAfter(@intCast(@min(s.message_until_ms - now, if (s.autoplay) @max(1, s.autoplay_at_ms - now) else s.message_until_ms - now)));
    }

    return ui.column(.{ .grow = 1, .align_items = .stretch, .children = children.items });
}

pub fn main(init: std.process.Init) !void {
    const dark = init.environ_map.get("ZANT_DARK") != null;
    const autoplay_env = init.environ_map.get("ZANT_AUTOPLAY");
    const quiet = if (autoplay_env) |v| std.mem.eql(u8, v, "quiet") else false;
    try zant.launch.run(init.gpa, State{ .dark = dark, .autoplay = autoplay_env != null, .quiet = quiet }, render, .{
        .title = "Zant 组件总览",
        .window = .{ .width = 1100, .height = 820, .min_width = 900, .min_height = 600 },
    });
}
