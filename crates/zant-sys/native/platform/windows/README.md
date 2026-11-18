# Windows native backend

Windows 10/11 x64 implementation of `platform/platform.h`, compiled into the
executable by `crates/zant-sys/build.rs` (MSVC, C11):

| File | Purpose |
|---|---|
| `app.c` | Process setup, message loop, single instance, tray icon and menus, global hotkeys |
| `window_lifecycle.c` | Window creation and teardown, cursor updates, DWM title bar styling |
| `window_events.c` | Window procedure, pointer and keyboard input, `WM_CHAR` surrogate pairs, app-drawn title bar messages (`WM_NCCALCSIZE` / `WM_NCHITTEST`, `HTMAXBUTTON` for snap layouts), close-request veto |
| `window_geometry.c` | Per-monitor DPI, window sizing, visibility, and persistent placement |
| `window_ime.c` | IMM32 composition and candidate placement |
| `window_render.c` | Frame pacing, drawing, and snapshots |
| `clipboard.c` | Clipboard text, images, files, and change notifications |
| `desktop_services.c` | Native dialogs (`IFileOpenDialog`, `IFileSaveDialog`, `TaskDialogIndirect`), shell actions (`ShellExecuteW`), launch at login, and system settings |
| `pdf_win.cpp` | PDF backend: PDFium (loaded at runtime), pages rendered to memory pixels |
| `zant.manifest` | Common-Controls v6 manifest embedded by `zant-build` |
| `platform.h` / `pdf.h` (in `..`) | The C ABI both platforms implement |

Rendering lives in `skia/skia_bridge_win.cpp` (DirectWrite fonts, embedded ICU
data and STIX Two Math, Direct3D 12 Ganesh swap chain). The native code is
linked statically; there is no `zant_skia.dll`, `.def` or `dlltool` import
library any more.
