//! Markdown editor component: edit / split / preview modes, a formatting
//! toolbar and editing shortcuts (⌘B ⌘I ⌘K ⌘E ⌘⇧X, list continuation on
//! Enter, Tab / ⇧Tab list indentation).
//!
//! Stateless like every Zant component: the text, selection and mode live in
//! the app and come back through callbacks.
const std = @import("std");
const ui = @import("../ui.zig");
const actions = @import("../core/actions.zig");
const events = @import("../core/events.zig");
const edit = @import("edit.zig");
const cache_mod = @import("cache.zig");
const view = @import("render/view.zig");

pub const Mode = enum(usize) { edit = 0, split = 1, preview = 2 };

pub const Labels = struct {
    edit: []const u8 = "Edit",
    split: []const u8 = "Split",
    preview: []const u8 = "Preview",
    /// Toolbar tooltips, in `edit.Command` order.
    tools: [15][]const u8 = .{ "Bold ⌘B", "Italic ⌘I", "Strikethrough ⇧⌘X", "Inline code ⌘E", "Link ⌘K", "Heading", "Quote", "Bulleted list", "Numbered list", "Task list", "Code block", "Inline math", "Math block", "Divider", "Table" },

    pub const zh: Labels = .{
        .edit = "编辑",
        .split = "分栏",
        .preview = "预览",
        .tools = .{ "粗体 ⌘B", "斜体 ⌘I", "删除线 ⇧⌘X", "行内代码 ⌘E", "链接 ⌘K", "标题", "引用", "无序列表", "有序列表", "任务列表", "代码块", "行内公式", "公式块", "分隔线", "表格" },
    };
};

pub const Props = struct {
    key: []const u8 = "md-editor",
    value: []const u8,
    /// Controlled selection `(anchor, cursor)`; feed `on_selection` back in.
    selection: ?[2]usize = null,
    mode: Mode = .edit,
    on_change: ?actions.TextAction = null,
    on_selection: ?actions.RangeAction = null,
    /// Receives the new `Mode` as an index.
    on_mode: ?actions.IndexAction = null,
    on_undo: ?actions.Action = null,
    on_redo: ?actions.Action = null,
    /// Block cache for the preview (required for split / preview modes).
    cache: ?*cache_mod.Cache = null,
    preview: view.Props = .{},
    placeholder: []const u8 = "",
    font_size: f32 = 16,
    line_height: f32 = 1.8,
    toolbar: bool = true,
    mode_switch: bool = true,
    shortcuts: bool = true,
    /// Width of the preview column in preview mode (null = fill).
    preview_width: ?f32 = null,
    labels: Labels = .{},
    /// Centre the text column at this width while the editor and preview
    /// (and their scroll bars) span the full available width.
    content_max_width: ?f32 = null,
    toolbar_commands: []const edit.Command = &default_toolbar,
};

pub const default_toolbar = [_]edit.Command{ .heading, .bold, .italic, .strikethrough, .inline_code, .link, .quote, .bullet_list, .ordered_list, .task_list, .code_block, .math_block, .table };

/// Callbacks shared by the key hook and the toolbar.
const Sink = struct {
    on_change: ?actions.TextAction,
    on_selection: ?actions.RangeAction,

    fn emit(self: *const Sink, result: edit.Result) void {
        if (self.on_change) |a| a.func(a.ctx, result.text);
        if (self.on_selection) |a| a.func(a.ctx, result.selection.anchor, result.selection.cursor);
    }
};

const KeyCtx = struct {
    sink: Sink,

    fn onKey(self: *KeyCtx, key: events.KeyEvent, text: []const u8, anchor: usize, cursor: usize) bool {
        const sel: edit.Selection = .{ .anchor = anchor, .cursor = cursor };
        var arena = std.heap.ArenaAllocator.init(std.heap.c_allocator);
        defer arena.deinit();
        const a = arena.allocator();
        const meta = key.modifiers.meta or key.modifiers.control;
        if (meta) {
            const cmd: ?edit.Command = if (events.characterEquals(key.key, 'b'))
                .bold
            else if (events.characterEquals(key.key, 'i'))
                .italic
            else if (events.characterEquals(key.key, 'k'))
                .link
            else if (events.characterEquals(key.key, 'e'))
                .inline_code
            else if (key.modifiers.shift and events.characterEquals(key.key, 'x'))
                .strikethrough
            else
                null;
            const c = cmd orelse return false;
            const result = edit.apply(a, text, sel, c) catch return false;
            self.sink.emit(result);
            return true;
        }
        switch (key.key) {
            .enter => {
                if (key.modifiers.shift) return false;
                const result = (edit.continueList(a, text, sel) catch return false) orelse return false;
                self.sink.emit(result);
                return true;
            },
            .tab => {
                const result = (edit.indentList(a, text, sel, key.modifiers.shift) catch return false) orelse blk: {
                    if (key.modifiers.shift) return true;
                    const s = sel.start();
                    const out = std.mem.concat(a, u8, &.{ text[0..s], "  ", text[sel.end()..] }) catch return false;
                    break :blk edit.Result{ .text = out, .selection = edit.Selection.collapsed(s + 2) };
                };
                self.sink.emit(result);
                return true;
            },
            else => return false,
        }
    }
};

const ToolbarCtx = struct {
    sink: Sink,
    text: []const u8,
    selection: edit.Selection,
    commands: []const edit.Command,

    fn run(self: *ToolbarCtx, index: usize) void {
        if (index >= self.commands.len) return;
        var arena = std.heap.ArenaAllocator.init(std.heap.c_allocator);
        defer arena.deinit();
        const result = edit.apply(arena.allocator(), self.text, self.selection, self.commands[index]) catch return;
        self.sink.emit(result);
    }
};

fn commandIcon(cmd: edit.Command) ui.Icon {
    return switch (cmd) {
        .bold => .bold,
        .italic => .italic,
        .strikethrough => .strikethrough,
        .inline_code => .code,
        .link => .link,
        .heading => .heading,
        .quote => .quote,
        .bullet_list => .list_bullet,
        .ordered_list => .list_ordered,
        .task_list => .list_checks,
        .code_block => .code_block,
        .inline_math, .math_block => .sigma,
        .rule => .minus_rule,
        .table => .table,
    };
}

/// Centres `child` in a full-width row, capped at `max` (plus side gutters).
fn centered(key: []const u8, max: ?f32, gutter: f32, child: ui.Node) ui.Node {
    return ui.row(.{
        .key = key,
        .width_percent = 100,
        .justify = .center,
        .padding = .{ .left = gutter, .right = gutter },
        .children = &.{ui.column(.{ .width_percent = 100, .max_width = max, .align_items = .stretch, .children = &.{child} })},
    });
}

pub fn editor(p: Props) ui.Node {
    const arena = ui.frameAllocator();
    const theme = p.preview.render.theme;
    const sink: Sink = .{ .on_change = p.on_change, .on_selection = p.on_selection };
    const sel: edit.Selection = if (p.selection) |s| .{ .anchor = @min(s[0], p.value.len), .cursor = @min(s[1], p.value.len) } else edit.Selection.collapsed(p.value.len);
    const gutter: f32 = if (p.content_max_width != null) 24 else 0;
    const k = struct {
        fn f(base: []const u8, comptime suffix: []const u8) []const u8 {
            return std.fmt.allocPrint(ui.frameAllocator(), "{s}-" ++ suffix, .{base}) catch base;
        }
    }.f;

    var rows: std.ArrayListUnmanaged(ui.Node) = .empty;
    var pane_bar: ?ui.Node = null;
    if (p.toolbar or p.mode_switch) {
        var left: std.ArrayListUnmanaged(ui.Node) = .empty;
        if (p.toolbar and p.mode != .preview) {
            const tctx = arena.create(ToolbarCtx) catch @panic("Zant UI: out of memory");
            tctx.* = .{ .sink = sink, .text = ui.frameString(p.value), .selection = sel, .commands = p.toolbar_commands };
            for (p.toolbar_commands, 0..) |cmd, i| {
                left.append(arena, ui.iconButton(.{
                    .key = std.fmt.allocPrint(arena, "{s}-tool-{d}", .{ p.key, i }) catch p.key,
                    .icon = commandIcon(cmd),
                    .size = 17,
                    .box = 30,
                    .color = theme.muted_foreground,
                    .tooltip = p.labels.tools[@backingInt(cmd)],
                    .index_action = ui.indexAction(tctx, ToolbarCtx.run),
                    .index = i,
                })) catch {};
            }
        }
        const switcher = if (p.mode_switch) ui.segmented(.{
            .key = k(p.key, "mode"),
            .options = &.{
                .{ .label = p.labels.edit, .icon = .pencil },
                .{ .label = p.labels.split, .icon = .columns },
                .{ .label = p.labels.preview, .icon = .book_open },
            },
            .selected = @backingInt(p.mode),
            .on_change = p.on_mode,
        }) else ui.space(.{ .width = 1 });
        const bar = centered(k(p.key, "bar-wrap"), p.content_max_width, gutter, ui.row(.{
            .key = k(p.key, "bar"),
            .width_percent = 100,
            .height = 38,
            .align_items = .center,
            .justify = .space_between,
            .children = &.{ ui.row(.{ .gap = 2, .align_items = .center, .children = left.items }), switcher },
        }));
        // split view without a mode switch: the formatting tools belong to
        // the editing pane, so they sit above it rather than the whole window
        if (p.mode == .split and !p.mode_switch) pane_bar = bar else rows.append(arena, bar) catch {};
    }

    const kctx = arena.create(KeyCtx) catch @panic("Zant UI: out of memory");
    kctx.* = .{ .sink = sink };
    const input = ui.styled(ui.input(.{
        .key = k(p.key, "input"),
        .value = p.value,
        .multiline = true,
        .height = 120,
        .font_size = p.font_size,
        .line_height = p.line_height,
        .focus_ring = false,
        .placeholder = p.placeholder,
        .on_change = p.on_change,
        .on_undo = p.on_undo,
        .on_redo = p.on_redo,
        .selection = p.selection,
        .on_selection = p.on_selection,
        .on_key = if (p.shortcuts) ui.inputKeyAction(kctx, KeyCtx.onKey) else null,
        .content_max_width = p.content_max_width,
    }), .{ .grow = 1, .shrink = 1, .basis = 0, .background = theme.background, .border_color = 0x00000000, .border_width = 0, .radius = 0, .padding = .{} });
    // a growing column lets the input fill the height in rows (split mode)
    const input_children: []const ui.Node = if (pane_bar) |bar| (arena.dupe(ui.Node, &.{ bar, input }) catch &.{input}) else (arena.dupe(ui.Node, &.{input}) catch &.{});
    const input_column = ui.column(.{ .key = k(p.key, "input-col"), .grow = 1, .shrink = 1, .basis = 0, .align_items = .stretch, .gap = 6, .children = input_children });

    const preview = if (p.mode != .edit) blk: {
        const content = if (p.cache) |c| view.streamdown(c, p.value, p.preview) else ui.text("(no preview cache)", .{ .color = theme.muted_foreground });
        const max = p.content_max_width orelse p.preview_width;
        // the scroll container spans the full width (scroll bar on the
        // window edge); the text column is centred inside it
        break :blk ui.column(.{
            .key = k(p.key, "preview"),
            .grow = 1,
            .shrink = 1,
            .basis = 0,
            .scroll = true,
            .padding = .{ .top = 8, .bottom = 32 },
            .align_items = .stretch,
            .children = &.{centered(k(p.key, "preview-col"), max, if (max != null) 24 else 12, content)},
        });
    } else null;

    const body = switch (p.mode) {
        .edit => input_column,
        .preview => preview.?,
        .split => ui.row(.{
            .key = k(p.key, "split"),
            .grow = 1,
            .align_items = .stretch,
            .children = &.{ input_column, ui.styled(ui.space(.{ .width = 1 }), .{ .background = theme.border }), preview.? },
        }),
    };
    rows.append(arena, ui.styled(body, .{ .grow = 1, .shrink = 1 })) catch {};
    return ui.column(.{
        .key = p.key,
        .grow = 1,
        .align_items = .stretch,
        .gap = 6,
        .children = rows.items,
    });
}
