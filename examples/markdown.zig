//! Markdown gallery: `zig build run-markdown` (ZANT_DARK=1 for dark).
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const md = zant.markdown;

const sample =
    \\# Streamdown for Zant
    \\
    \\Native Markdown with **bold**, *italic*, ~~strike~~, ==highlight==, `inline code`, [links](https://github.com/vercel/streamdown), autolinks like https://zant.dev, footnotes[^1] and inline math $$e^{i\pi} + 1 = 0$$ that sits on the baseline.
    \\
    \\> [!TIP]
    \\> Callouts, quotes and **nested** content are supported.
    \\
    \\## Lists
    \\
    \\1. Ordered item
    \\2. Second item
    \\   - [x] finished task
    \\   - [ ] open task
    \\     - deeper bullet
    \\
    \\## Math
    \\
    \\$$
    \\\int_{-\infty}^{\infty} e^{-x^2}\,dx = \sqrt{\pi} \qquad A = \begin{pmatrix} 1 & 2 \\ 3 & 4 \end{pmatrix}
    \\$$
    \\
    \\## Table
    \\
    \\| Feature | Status | Notes |
    \\| :--- | :---: | ---: |
    \\| remend | ✅ | 522 upstream cases |
    \\| parse-blocks | ✅ | incremental |
    \\
    \\## Code
    \\
    \\```zig
    \\const std = @import("std");
    \\pub fn main() void {
    \\    std.debug.print("hello {s}\n", .{"zant"}); // greet
    \\}
    \\```
    \\
    \\## Diagram
    \\
    \\```mermaid
    \\graph LR
    \\  A[Source] --> B(remend)
    \\  B --> C{parse-blocks}
    \\  C -->|changed| D[Parse]
    \\  C -->|stable| E[Reuse]
    \\  D --> F([Render])
    \\  E --> F
    \\```
    \\
    \\---
    \\
    \\[^1]: Footnotes render at the end of the document.
;

const State = struct {
    cache: md.Cache,
    dark: bool,
};

fn render(s: *State) ui.Node {
    const theme = if (s.dark) md.theme.dark() else md.theme.light();
    // the root node never scrolls itself: wrap the content in a scroll column
    return ui.column(.{
        .grow = 1,
        .background = theme.background,
        .align_items = .stretch,
        .children = &.{ui.column(.{
            .grow = 1,
            .scroll = true,
            .align_items = .center,
            .padding = .{ .top = 36, .bottom = 48 },
            .children = &.{ui.column(.{
                .width = 720,
                .align_items = .stretch,
                .children = &.{md.streamdown(&s.cache, sample, .{ .render = .{ .theme = theme, .content_width = 720 } })},
            })},
        })},
    });
}

pub fn main(init: std.process.Init) !void {
    const dark = init.environ_map.get("ZANT_DARK") != null;
    try zant.launch.run(init.gpa, State{ .cache = md.Cache.init(init.gpa), .dark = dark }, render, .{
        .title = "Zant Markdown",
        .window = .{ .width = 860, .height = 1500 },
        .theme = if (dark) zant.tokens.antDark() else zant.tokens.antLight(),
    });
}
