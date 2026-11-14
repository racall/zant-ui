# zant-examples

Runnable Zant UI examples written in Rust against the `zant` crate. The TSX
example apps live in [`tsx/`](tsx/README.md).

| binary | source | what it shows |
|---|---|---|
| `zant-demo` | `src/bin/zant-demo.rs` | a small declarative `ui::App` |
| `zant-components` | `src/components_app/` + `src/bin/zant-components.rs` | the component showcase (inputs, menus, modal, list, PDF viewer, …) |
| `zant-gallery` | `src/gallery_app/` + `src/bin/zant-gallery.rs` | the retained-mode gallery: a hand-built `core::tree::Tree` with its own event loop |
| `zant-markdown` | `src/markdown_app.rs` + `src/bin/zant-markdown.rs` | Markdown rendering |
| `zant-math` | `src/math_app.rs` + `src/bin/zant-math.rs` | math typesetting |
| `zant-md-stream` | `src/stream_app.rs` + `src/bin/zant-md-stream.rs` | streaming Markdown (frame-rate benchmark, `--sse` replays real SSE) |
| `zant-pdf` | `src/pdf_app.rs` + `src/bin/zant-pdf.rs` | the PDF reader |
| `zant-bench` | `src/bin/zant_bench/` + `src/bin/zant-bench.rs` | headless frame benchmark (`scripts/bench.sh`) |
| `zant-tsx-bench` | `src/bin/zant-tsx-bench.rs` | TSX compile benchmark (`scripts/tsx/examples-check.sh`) |
| `zant-tsx-frame-bench` | `src/bin/zant_tsx_frame_bench/` + `src/bin/zant-tsx-frame-bench.rs` | TSX vs hand-written frame benchmark (`scripts/tsx/bench-tsx.sh`) |

Build and run:

```bash
./scripts/cargo.sh build --release -p zant-examples
./target/release/zant-components            # ZANT_DARK=1, ZANT_AUTOPLAY=1 / quiet
./target/release/zant-gallery
```

`ZANT_*` automation switches: `ZANT_DARK` / `ZANT_THEME`, `ZANT_AUTOPLAY`
(`quiet`: no `[event]` stderr lines), `ZANT_SNAPSHOT` / `ZANT_SNAPSHOT_EXIT`,
`ZANT_LAYOUT_FILE` (declarative runtime), `ZANT_WINDOW_SIZE`, `ZANT_FRAME_STATS`,
`ZANT_NO_MOTION`.

## Notes

- `zant-components` keeps the last event in a 96-byte budget and drops
  over-long messages entirely.
- `zant-gallery` is imperative on purpose (hand-built retained tree, own event
  loop) instead of `ui::App` / `runtime::Runtime`: the declarative runtime
  rebuilds the tree from `render()` and would free the manually built nodes.
  It implements `platform::HostHandler` itself and holds the window only
  weakly (`Rc<Window>` behind a `Weak`), because the host handler owns the
  `Rc<RefCell<GalleryApp>>`.
- `zant-gallery` has no keyed layout dump (`ZANT_LAYOUT_FILE` is
  declarative-runtime code) and no dark mode.
- `zant-gallery`: `TextAction` has no node-id parameter, so each input's action
  captures its own id when the callbacks are wired after the tree is built; the
  tree only changes input state and the app notifies the input's `text_action`
  after insert / delete / paste, like `runtime/keyboard.rs:notify_text_action`.
