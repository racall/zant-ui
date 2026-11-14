# Windows native backend

Windows 10/11 x64 implementation of `platform/platform.h`, compiled into the
executable by `crates/zant-sys/build.rs` (MSVC, C11):

| File | Purpose |
|---|---|
| `window.c` | Win32 window: message loop, per-monitor DPI v2, mouse / keyboard / `WM_CHAR` (surrogate pairs), IMM32 composition and candidate placement, cursors, clipboard, app-drawn title bar (`WM_NCCALCSIZE` / `WM_NCHITTEST`, `HTMAXBUTTON` for snap layouts), DWM dark title bar, close-request veto, `IFileOpenDialog` / `IFileSaveDialog`, `TaskDialogIndirect`, `ShellExecuteW`, snapshots |
| `pdf_win.cpp` | PDF backend: PDFium (loaded at runtime), pages rendered to memory pixels |
| `zant.manifest` | Common-Controls v6 manifest embedded by `zant-build` |
| `platform.h` / `pdf.h` (in `..`) | The C ABI both platforms implement |

Rendering lives in `skia/skia_bridge_win.cpp` (DirectWrite fonts, embedded ICU
data and STIX Two Math, Direct3D 12 Ganesh swap chain). The native code is
linked statically; there is no `zant_skia.dll`, `.def` or `dlltool` import
library any more.
