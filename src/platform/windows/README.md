# Windows backend

Windows 10/11 x64 implementation of `src/platform/platform.h`.

| File | Role |
|---|---|
| `window.c` | Win32 window (compiled by Zig): message loop, per-monitor DPI v2, mouse / keyboard / `WM_CHAR` (surrogate pairs), IMM32 composition and candidate placement, cursors, clipboard, app-drawn title bar (`WM_NCCALCSIZE` / `WM_NCHITTEST`, `HTMAXBUTTON` for snap layouts), DWM dark title bar, close-request veto, `IFileOpenDialog` / `IFileSaveDialog`, `TaskDialogIndirect`, `ShellExecuteW`, snapshots |
| `pdf_win.cpp` | `pdf.h` on PDFium (loaded from `pdfium.dll` at first use) |
| `zant.manifest` | PerMonitorV2, UTF-8 code page, Common Controls v6, long paths |
| `winsqlite3.def` | import definitions for the system SQLite (`zant.db`) |

Rendering lives in `src/renderer/skia/skia_bridge_win.cpp` (DirectWrite fonts, ICU data
and STIX Two Math as DLL resources, Direct3D 12 swap chain). The prebuilt Skia is built with
MSVC's STL and `/MD`, so the bridge, `pdf_win.cpp` and Skia are linked by MSVC into
`zant_skia.dll` (`scripts/win/build_bridge.ps1`, run by `build.zig` on Windows hosts and
cached by Zig). Zig code links that DLL only through the C ABI, using an import library made
by `zig dlltool` from `src/renderer/skia/zant_skia.def`; that also lets
`zig build -Dtarget=x86_64-windows` type-check on macOS.

Frames are drawn from `WM_PAINT`: redraw requests invalidate the window, so they work in any
message loop (live resize, modal dialogs) and input is always handled before the next frame.
Animation frames are paced by the swap chain (frame latency 1) or by `DwmFlush` on raster.

Known gaps: IMM32 rather than TSF (no reconversion), x64 only.
