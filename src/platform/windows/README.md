# Windows backend (phase 2)

This directory is intentionally empty in phase 1. Windows support is deferred
by project direction and has not been compiled or run.

Planned implementation:

- Win32 `HWND`, `WM_SIZE`, `WM_MOUSEMOVE`, `WM_LBUTTONDOWN/UP`, `WM_KEY*`,
  DPI awareness and system cursor integration.
- TSF (or another verified system text-service integration) for composition,
  candidates and commit; key events alone must not emulate IME text.
- Skia Ganesh Direct3D 12 bridge behind the same `src/renderer/skia/skia_bridge.h`
  C ABI, plus a CPU raster fallback.
- A pinned Windows Skia archive from the same rust-skia release, fetched by a
  platform-specific script.

Zig core modules (`src/ui.zig`, `src/ui_runtime.zig`, `src/core/`,
`src/components/`, `src/layout/`, `src/theme/`) are platform-neutral. The
macOS-only dependencies are isolated in `src/platform/macos/` and the build
module's Objective-C++/Metal settings.
