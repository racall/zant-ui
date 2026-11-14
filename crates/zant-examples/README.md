# zant-examples

Runnable Zant UI examples, ported from the Zig `examples/` / `src/` entry
points (docs/rust-port/plan.md WP-2.9). The binaries keep the Zig executable
names so `scripts/parity/` and the Windows desktop scripts find them.

| binary | Zig source | Rust source |
|---|---|---|
| `zant-demo` | `examples/declarative.zig` | `src/bin/zant-demo.rs` (WP-1.5) |
| `zant-components` | `examples/components.zig` | `src/components_app/` + `src/bin/zant-components.rs` |
| `zant-gallery` | `src/main.zig` + `src/app.zig` + `src/gallery.zig` (`examples/gallery.zig` re-exports `zant.gallery.GalleryState`) | `src/gallery_app/` + `src/bin/zant-gallery.rs` |

Build and run:

```bash
./scripts/cargo.sh build --release -p zant-examples
./target/release/zant-components            # ZANT_DARK=1, ZANT_AUTOPLAY=1 / quiet
./target/release/zant-gallery
```

`ZANT_*` automation switches are the Zig ones: `ZANT_DARK`, `ZANT_AUTOPLAY`
(`quiet`: no `[event]` stderr lines), `ZANT_SNAPSHOT` / `ZANT_SNAPSHOT_EXIT`,
`ZANT_LAYOUT_FILE` (declarative runtime), `ZANT_WINDOW_SIZE`, `ZANT_FRAME_STATS`,
`ZANT_NO_MOTION`.

## Parity with the Zig baseline

```bash
cargo build --release -p zant-examples
ZANT_ZIG_BIN=/path/to/zig-out-baseline/bin scripts/parity/compare.sh zant-demo
```

`zant-demo` passes `compare.sh` 6/6 (light/dark × 600x620 / 900x640 /
1200x800; 8 layout keys, 0 pixels differ).

For the two WP-2.9 binaries `compare.sh` cannot report a full pass on its own,
for reasons on the Zig side plus one unported widget:

- `zant-components`: the Zig `ui.pdfViewer` writes its outer layout key
  `pdf-2002` **twice**, and `scripts/parity/layoutdiff.py` rejects duplicate
  keys, so the layout check never runs (`.../layout.txt:78: duplicate key
  'pdf-2002'`). The PDF viewer itself is WP-3.9 and is still a stub in Rust, so
  the example renders a placeholder in its slot (same key / size / border).
  `parity/components_parity.sh` compares what is portable: the screenshot with
  `imgdiff.py` (6/6 pass, 0 pixels differ) and the layout dump with the
  duplicate and the `pdf-2002*` keys filtered out (6/6 pass, 74 keys identical
  in every combination).
- `zant-gallery`: the Zig imperative app never writes `ZANT_LAYOUT_FILE` (that
  is declarative-runtime code) and its hand-built tree has no keys, so
  `capture.sh` fails with "produced no layout.txt". The screenshots are
  compared directly: `parity/gallery_image_parity.sh` (6/6 pass, 0 pixels
  differ in every combination). The gallery has no dark mode in the Zig app,
  so its `dark-*` combinations capture the same light UI on both sides;
  `ZANT_DARK` / `ZANT_THEME` are ignored by both, exactly like Zig.

Both scripts take the Zig baseline directory from `ZANT_ZIG_BIN` (default
`<repo>/zig-out-baseline/bin`).

## Deviations from the Zig sources

- `zant-components`: `ui.pdfViewer` is not ported yet (WP-3.9; `pdf/viewer/` is
  a stub on this branch). The documents section keeps the widget's slot as a
  bordered placeholder and the `on_change` callback that logs "pdf changed" is
  omitted until the widget lands.
- `zant-components`: Zig stores the last event in a fixed 96-byte buffer with
  `bufPrint` and drops over-long messages entirely; the port keeps that rule.
  `ui.textFmt` has no Rust counterpart, so call sites format with `format!`.
- `zant-gallery`: the app keeps the Zig imperative shape (hand-built retained
  tree, own event loop) instead of `ui::App` / `runtime::Runtime`, because the
  declarative runtime rebuilds the tree from `render()` and would free the
  manually built nodes. It implements `platform::HostHandler` itself, like the
  Zig `makeHost`.
- `zant-gallery`: the app holds the window only weakly (`Rc<Window>` behind a
  `Weak`) because the host handler owns the `Rc<RefCell<GalleryApp>>`; Zig kept
  a raw `*Window`.
- `zant-gallery`: Zig's `onChange` callback receives the node id; Rust's
  `TextAction` has no node-id parameter, so each input's action captures its
  own id when the callbacks are wired after the tree is built.
- `zant-gallery`: the Rust tree only changes input state, the runtime notifies
  the input's `text_action` after a successful edit
  (`runtime/keyboard.rs:notify_text_action`); the gallery app does the same
  after insert / delete / paste, like Zig `Tree.inputChanged`.
