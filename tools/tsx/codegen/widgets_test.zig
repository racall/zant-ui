//! Widget diagnostics use the normal reflected prop list and source positions.
const expectDiagnostic = @import("errors_test.zig").expectDiagnostic;
const App = "export default function App() { return ";
const markdown_text = "Markdown takes its text as `source` or as its children";

test "document widget diagnostics" {
    const cases = [_][2][]const u8{
        .{ App ++ "~<Markdown/>; }", "<Markdown> is missing required prop `source`" },
        .{ App ++ "~<PdfViewer/>; }", "<PdfViewer> is missing required prop `src`" },
        .{ App ++ "<Markdown>~<Text>Hi</Text></Markdown>; }", markdown_text },
        .{ App ++ "<Markdown>{~<Text>Hi</Text>}</Markdown>; }", markdown_text },
        .{ App ++ "<Markdown>{~12}</Markdown>; }", markdown_text },
        .{ App ++ "~<Markdown source=\"x\">y</Markdown>; }", "cannot have both `source` and text children" },
        .{ App ++ "<PdfViewer src=\"\">~<Text>Hi</Text></PdfViewer>; }", "<PdfViewer> does not accept children" },
        .{ App ++ "<PdfViewer src=\"\">~text</PdfViewer>; }", "<PdfViewer> does not accept children" },
        .{ App ++ "<PdfViewer src=\"\">{~\"text\"}</PdfViewer>; }", "<PdfViewer> does not accept children" },
        .{
            App ++ "<Markdown source=\"\" ~unknown/>; }",
            "unknown prop `unknown` on <Markdown>; valid props: source, key, streaming, parseIncomplete",
        },
        .{
            App ++ "<PdfViewer src=\"\" ~unknown/>; }",
            "unknown prop `unknown` on <PdfViewer>; valid props: src, password, key, toolbar",
        },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}
