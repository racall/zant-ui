//! PDF backend of ui.pdfPreview: PDFKit on macOS, PDFium on Windows.
const std = @import("std");
const zant = @import("zant");

/// One page "Hello PDF" document with a correct xref table.
fn minimalPdf(buf: []u8) ![]const u8 {
    const objects = [_][]const u8{
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
        "<< /Length 39 >>\nstream\nBT /F1 24 Tf 20 40 Td (Hello PDF) Tj ET\nendstream",
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    };
    var w: std.Io.Writer = .fixed(buf);
    try w.writeAll("%PDF-1.4\n");
    var offsets: [objects.len]usize = undefined;
    for (objects, 0..) |body, i| {
        offsets[i] = w.end;
        try w.print("{d} 0 obj\n{s}\nendobj\n", .{ i + 1, body });
    }
    const xref = w.end;
    try w.print("xref\n0 {d}\n0000000000 65535 f \n", .{objects.len + 1});
    for (offsets) |off| try w.print("{d:0>10} 00000 n \n", .{off});
    try w.print("trailer\n<< /Size {d} /Root 1 0 R >>\nstartxref\n{d}\n%%EOF\n", .{ objects.len + 1, xref });
    return w.buffered();
}

test "pdf: page count, rendered page and page text" {
    var buf: [2048]u8 = undefined;
    const data = try minimalPdf(&buf);
    const path = "zig-out/test-minimal.pdf";
    std.Io.Dir.cwd().createDirPath(std.testing.io, "zig-out") catch {};
    try zant.dialogs.writeFile(path, data);

    try std.testing.expectEqual(@as(usize, 1), zant.pdf.pageCount(path));

    var out: [1024]u8 = undefined;
    const png = zant.pdf.renderPage(path, 0, 400, 200, &out) orelse return error.RenderFailed;
    try std.testing.expect(std.mem.endsWith(u8, png, ".png"));
    try std.Io.Dir.cwd().access(std.testing.io, png, .{});

    var text_buf: [256]u8 = undefined;
    const text = zant.pdf.pageText(path, 0, &text_buf) orelse return error.TextFailed;
    try std.testing.expect(std.mem.indexOf(u8, text, "Hello PDF") != null);
}
