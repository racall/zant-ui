# Third-party notices

## Streamdown

`src/markdown/` (notably `remend/`, `blocks.zig`, `table_utils.zig` and the
component behaviour in `render/`) is a Zig port of
[Streamdown](https://github.com/vercel/streamdown), and
`tests/fixtures/remend_cases.json` is extracted from its test suite.

Copyright 2023 Vercel, Inc. Licensed under the Apache License, Version 2.0.
You may obtain a copy of the License at http://www.apache.org/licenses/LICENSE-2.0

## KaTeX

`src/math/layout.zig` follows the layout algorithms and font metric values of
[KaTeX](https://github.com/KaTeX/KaTeX). Copyright (c) 2013-2020 Khan Academy
and other contributors. Licensed under the MIT License.

## Skia

Rendering uses [Skia](https://skia.org) (milestone 157), linked from the
prebuilt archives of [rust-skia](https://github.com/rust-skia/skia-binaries)
(`scripts/fetch_skia.sh`, `scripts/fetch_skia_windows.ps1`; on Windows inside
`zant_skia.dll`). Copyright 2011 Google Inc. Licensed under the BSD 3-Clause
License. The archives also contain libraries Skia depends on, each under its
own license, notably:

- **HarfBuzz** (text shaping) — "Old MIT" License.
- **ICU** (Unicode, line breaking; on Windows its data file `icudtl.dat` is
  embedded in `zant_skia.dll`) — Unicode License V3.
- **libpng**, **libjpeg-turbo**, **zlib**, **expat** — their respective
  permissive licenses.

## PDFium (Windows)

On Windows `ui.pdfPreview` loads `pdfium.dll` from
[pdfium-binaries](https://github.com/bblanchon/pdfium-binaries)
(`scripts/fetch_skia_windows.ps1`), shipped next to the app by `zant package`.
PDFium: Copyright 2014 The PDFium Authors, BSD 3-Clause License (portions under
Apache License 2.0); pdfium-binaries build scripts: MIT License.

## STIX Two Math (Windows)

macOS ships the STIX Two Math font; Windows does not, so `zant_skia.dll`
embeds `STIXTwoMath-Regular.otf` (from CTAN, `fonts/stix2-otf`) for math
layout. Copyright 2001-2021 The STIX Fonts Project Authors. Licensed under the
SIL Open Font License, Version 1.1.

## Microsoft Visual C++ runtime (Windows)

`zant package` on Windows copies `vcruntime140.dll`, `vcruntime140_1.dll` and
`msvcp140.dll` from the build machine, which `zant_skia.dll` (built with
`/MD`) needs. They are redistributable files of Visual Studio, subject to the
Microsoft Software License Terms of the installed Visual Studio / Build Tools.
