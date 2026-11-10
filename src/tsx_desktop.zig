//! Desktop hooks of TSX apps: the tray (`useTray`) and global hotkeys
//! (`useHotkey`). Both are applied while rendering, only when something
//! changed, and their callbacks run from app events (`zant.tsx.app.Shell`
//! forwards them here).
const std = @import("std");
const ui = @import("ui.zig");
const desktop = @import("desktop.zig");
const actions = @import("core/actions.zig");

const gpa = std.heap.c_allocator;
/// Tests drive TSX apps headless: they must not add a tray icon or take
/// global hotkeys on the machine running them.
const system = !@import("builtin").is_test;

// ---------------------------------------------------------------- tray

/// One menu item of `useTray`.
pub const TrayItem = struct {
    title: []const u8 = "",
    checked: bool = false,
    disabled: bool = false,
    separator: bool = false,
    on_select: ?actions.Action = null,
};

pub const TrayOptions = struct {
    icon: []const u8,
    icon_light: ?[]const u8 = null,
    template: bool = false,
    tooltip: []const u8 = "",
    items: []const TrayItem = &.{},
    /// Windows: a left click on the icon.
    on_click: ?actions.Action = null,
};

/// The app's tray icon (one per app).
pub const Tray = struct {
    /// What `desktop.setTray` last got (its own copy), null before.
    applied: ?Applied = null,
    arena: std.heap.ArenaAllocator = std.heap.ArenaAllocator.init(gpa),
    /// This build's callbacks (frame memory: valid until the next build).
    selects: std.ArrayList(?actions.Action) = .empty,
    click_action: ?actions.Action = null,
    /// The system refused the icon (no tray): `useTray` returns false.
    failed: bool = false,

    const Applied = struct {
        icon: []const u8,
        icon_light: ?[]const u8,
        template: bool,
        tooltip: []const u8,
        items: []const desktop.TrayItem,
    };

    /// Applies `options` when they differ from the last call; true while
    /// the icon is shown.
    pub fn update(self: *Tray, options: TrayOptions) bool {
        self.selects.clearRetainingCapacity();
        for (options.items) |item| self.selects.append(gpa, item.on_select) catch {};
        self.click_action = options.on_click;
        if (self.applied) |last| if (same(last, options)) return !self.failed;
        _ = self.arena.reset(.retain_capacity);
        const a = self.arena.allocator();
        const items = a.alloc(desktop.TrayItem, options.items.len) catch return false;
        for (options.items, items, 0..) |item, *out, i| out.* = .{
            .id = @intCast(i + 1),
            .title = a.dupe(u8, item.title) catch "",
            .checked = item.checked,
            .disabled = item.disabled,
            .separator = item.separator,
        };
        const applied = Applied{
            .icon = a.dupe(u8, options.icon) catch return false,
            .icon_light = if (options.icon_light) |l| a.dupe(u8, l) catch null else null,
            .template = options.template,
            .tooltip = a.dupe(u8, options.tooltip) catch "",
            .items = items,
        };
        self.applied = applied;
        if (!system) return true;
        desktop.setTray(.{
            .icon = applied.icon,
            .icon_light = applied.icon_light,
            .template = applied.template,
            .tooltip = applied.tooltip,
            .items = applied.items,
        }) catch |err| {
            std.log.warn("zant: tray: {s}", .{@errorName(err)});
            self.failed = true;
            return false;
        };
        self.failed = false;
        return true;
    }

    fn same(last: Applied, options: TrayOptions) bool {
        if (!std.mem.eql(u8, last.icon, options.icon) or last.template != options.template) return false;
        if (!std.mem.eql(u8, last.tooltip, options.tooltip)) return false;
        const light_same = if (last.icon_light) |l|
            (if (options.icon_light) |o| std.mem.eql(u8, l, o) else false)
        else
            options.icon_light == null;
        if (!light_same or last.items.len != options.items.len) return false;
        for (last.items, options.items) |a, b| {
            if (!std.mem.eql(u8, a.title, b.title) or a.checked != b.checked) return false;
            if (a.disabled != b.disabled or a.separator != b.separator) return false;
        }
        return true;
    }

    /// The menu item with `id` (1-based) was chosen.
    pub fn select(self: *Tray, id: u32) void {
        if (id == 0 or id > self.selects.items.len) return;
        if (self.selects.items[id - 1]) |action| action.func(action.ctx);
    }

    pub fn click(self: *Tray) void {
        if (self.click_action) |action| action.func(action.ctx);
    }
};

pub var tray: Tray = .{};

// ---------------------------------------------------------------- shortcuts

/// `<HotkeyInput value="ctrl+cmd+v">`: null for "" or an invalid string.
pub fn parseShortcut(text: []const u8) ?desktop.Shortcut {
    return desktop.Shortcut.parse(text);
}

/// `<HotkeyInput onRecord={(shortcut: string) => …}>`: the recorded
/// shortcut as its string form ("ctrl+cmd+v", what `useHotkey` takes).
pub fn shortcutAction(target: actions.TextAction) ui.HotkeyAction {
    const Adapter = struct {
        target: actions.TextAction,
        fn call(self: *@This(), shortcut: desktop.Shortcut) void {
            var buf: [64]u8 = undefined;
            self.target.func(self.target.ctx, ui.frameString(shortcut.format(&buf)));
        }
    };
    const adapter = ui.frameAllocator().create(Adapter) catch @panic("Zant UI: out of memory");
    adapter.* = .{ .target = target };
    return ui.hotkeyAction(adapter, Adapter.call);
}

// ---------------------------------------------------------------- hotkeys

/// One `useHotkey`: its shortcut, registered while it is valid.
pub const Hotkey = struct {
    /// App-wide id (assigned on first use).
    id: u32 = 0,
    shortcut: [64]u8 = undefined,
    shortcut_len: usize = 0,
    registered: bool = false,
    action: ?actions.Action = null,

    /// Registers `shortcut` ("ctrl+cmd+v"; "" for none) when it changed;
    /// true while registered (false: taken by another app, or invalid).
    pub fn update(self: *Hotkey, shortcut: []const u8, action: actions.Action) bool {
        self.action = action;
        if (self.id == 0) register(self);
        const same = std.mem.eql(u8, self.shortcut[0..self.shortcut_len], shortcut);
        if (same and self.shortcut_len > 0) return self.registered;
        if (self.registered and system) desktop.unregisterHotkey(self.id);
        self.registered = false;
        const n = @min(shortcut.len, self.shortcut.len);
        @memcpy(self.shortcut[0..n], shortcut[0..n]);
        self.shortcut_len = n;
        const parsed = desktop.Shortcut.parse(shortcut) orelse return false;
        self.registered = !system or desktop.registerHotkey(self.id, parsed);
        return self.registered;
    }

    pub fn deinit(self: *Hotkey) void {
        if (self.registered and system) desktop.unregisterHotkey(self.id);
        self.registered = false;
        unregister(self);
    }
};

/// Ids from here up are TSX hotkeys (apps' own ids stay below).
const first_id: u32 = 0x7000_0000;
var hotkeys: std.ArrayList(*Hotkey) = .empty;
var next_id: u32 = first_id;

fn register(h: *Hotkey) void {
    h.id = next_id;
    next_id += 1;
    hotkeys.append(gpa, h) catch {};
}

fn unregister(h: *Hotkey) void {
    for (hotkeys.items, 0..) |item, i| if (item == h) {
        _ = hotkeys.swapRemove(i);
        return;
    };
}

/// A hotkey app event: runs the `useHotkey` callback with that id.
pub fn hotkeyPressed(id: u32) bool {
    for (hotkeys.items) |h| if (h.id == id) {
        const action = h.action orelse return false;
        action.func(action.ctx);
        return true;
    };
    return false;
}

test "tray menu items run their callbacks by id" {
    const Probe = struct {
        var hits: [3]u32 = .{ 0, 0, 0 };
        fn first(_: *u8) void {
            hits[0] += 1;
        }
        fn second(_: *u8) void {
            hits[1] += 1;
        }
        fn clicked(_: *u8) void {
            hits[2] += 1;
        }
    };
    var ctx: u8 = 0;
    var t = Tray{};
    defer t.arena.deinit();
    defer t.selects.deinit(gpa);
    // dispatch only (no system tray in tests)
    try t.selects.appendSlice(gpa, &.{ ui.action(&ctx, Probe.first), null, ui.action(&ctx, Probe.second) });
    t.click_action = ui.action(&ctx, Probe.clicked);
    t.select(1);
    t.select(2);
    t.select(3);
    t.select(9);
    t.click();
    try std.testing.expectEqualSlices(u32, &.{ 1, 1, 1 }, &Probe.hits);
}

test "tray changes are detected by value" {
    const items = [_]desktop.TrayItem{.{ .id = 1, .title = "Open" }};
    const last = Tray.Applied{ .icon = "png", .icon_light = null, .template = true, .tooltip = "App", .items = &items };
    const open: []const TrayItem = &.{.{ .title = "Open" }};
    const quit: []const TrayItem = &.{.{ .title = "Quit" }};
    try std.testing.expect(Tray.same(last, .{ .icon = "png", .template = true, .tooltip = "App", .items = open }));
    try std.testing.expect(!Tray.same(last, .{ .icon = "png", .template = true, .tooltip = "App", .items = quit }));
    try std.testing.expect(!Tray.same(last, .{ .icon = "png", .template = true, .tooltip = "App 2", .items = open }));
}

test "hotkey events reach the hook with that id" {
    const Probe = struct {
        var hits: u32 = 0;
        fn hit(_: *u8) void {
            hits += 1;
        }
    };
    var ctx: u8 = 0;
    var h = Hotkey{ .action = ui.action(&ctx, Probe.hit) };
    register(&h);
    defer unregister(&h);
    try std.testing.expect(hotkeyPressed(h.id));
    try std.testing.expect(!hotkeyPressed(h.id + 100));
    try std.testing.expectEqual(@as(u32, 1), Probe.hits);
}

test "a recorded shortcut reaches TSX as its string" {
    const Probe = struct {
        var got: [64]u8 = undefined;
        var len: usize = 0;
        fn record(_: *u8, text: []const u8) void {
            @memcpy(got[0..text.len], text);
            len = text.len;
        }
    };
    ui.beginFrame();
    defer ui.endFrame();
    var ctx: u8 = 0;
    const action = shortcutAction(ui.textAction(&ctx, Probe.record));
    const shortcut = parseShortcut("ctrl+alt+k").?;
    action.func(action.ctx, shortcut);
    var buf: [64]u8 = undefined;
    try std.testing.expectEqualStrings(shortcut.format(&buf), Probe.got[0..Probe.len]);
    try std.testing.expect(parseShortcut(Probe.got[0..Probe.len]).?.eql(shortcut));
    try std.testing.expect(parseShortcut("") == null);
}
