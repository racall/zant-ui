//! PDF preview example: `zig build run-pdf -- path/to/file.pdf`.
//!
//! Shows `ui.pdfPreview` (PDFKit on macOS, PDFium on Windows) with page
//! navigation and "copy page text". For visual checks, ZANT_PDF_PAGE picks
//! the page shown first (0-based; "last" for the last page) and
//! ZANT_PDF_PROBE=1 prints the page count and extracted text sizes instead
//! of opening a window.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;

const State = struct {
    path: [:0]const u8,
    page: usize = 0,

    fn setPage(self: *State, page: usize) void {
        self.page = page;
    }

    fn render(self: *State) ui.Node {
        return ui.column(.{
            .grow = 1,
            .align_items = .center,
            .padding = .{ .top = 20, .bottom = 20, .left = 20, .right = 20 },
            .gap = 12,
            .children = &.{
                ui.text(std.fs.path.basename(self.path), .{ .size = 13, .weight = .medium, .max_lines = 1 }),
                ui.pdfPreview(.{
                    .key = "pdf",
                    .src = self.path,
                    .page = self.page,
                    .width = 560,
                    .height = 720,
                    .background = ui.currentTheme().color_bg_container,
                    .on_page_change = ui.indexAction(self, setPage),
                }),
            },
        });
    }
};

pub fn main(init: std.process.Init) !void {
    const args = try init.minimal.args.toSlice(init.arena.allocator());
    if (args.len < 2) {
        std.debug.print("usage: zant-pdf <file.pdf>\n", .{});
        return;
    }
    var state = State{ .path = try init.arena.allocator().dupeSentinel(u8, args[1], 0) };
    // ZANT_PDF_PROBE=1: print page count and text sizes without a window
    if (init.environ_map.get("ZANT_PDF_PROBE") != null) {
        const count = zant.pdf.pageCount(state.path);
        var buf: [64 * 1024]u8 = undefined;
        const first = if (count > 0) zant.pdf.pageText(state.path, 0, &buf) else null;
        const first_len = if (first) |t| t.len else 0;
        const middle = if (count > 1) zant.pdf.pageText(state.path, count / 2, &buf) else null;
        const middle_len = if (middle) |t| t.len else 0;
        std.debug.print("pages={d} text0={d} text_mid={d}\n", .{ count, first_len, middle_len });
        return;
    }
    if (init.environ_map.get("ZANT_PDF_PAGE")) |page| {
        if (std.mem.eql(u8, page, "last")) {
            const count = zant.pdf.pageCount(state.path);
            state.page = if (count > 0) count - 1 else 0;
        } else {
            state.page = std.fmt.parseInt(usize, page, 10) catch 0;
        }
    }
    try zant.launch.run(init.gpa, state, State.render, .{
        .title = "Zant PDF",
        .window = .{ .width = 640, .height = 860, .min_width = 480, .min_height = 600 },
        .theme = zant.tokens.antLight(),
    });
}
