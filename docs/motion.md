# Motion

Zant UI animates in the spirit of **Ant Design motion** (durations, easing
curves, per-component transitions) using two ideas borrowed from **Framer
Motion**: *presence* (enter / exit transitions) and *layout* transitions
(FLIP). Everything is plain Rust in `crates/zant/src/motion/`; no dependency.

Because the whole window is painted by Skia on one surface (a `CAMetalLayer`
on macOS, a Direct3D 12 swap chain on Windows), the system compositor cannot
animate individual nodes. The platform contributes only frame pacing while
something animates (`CADisplayLink` on macOS; the swap chain's vsync /
`DwmFlush` on Windows) and the system's reduced-motion setting (macOS
**Reduce motion**, Windows **Animation effects**).

Measure frame times on a release build with `ZANT_FRAME_STATS=1`
(`cargo run -p zant-examples --bin zant-components` with `ZANT_AUTOPLAY=quiet` animates on its own);
see `docs/development.md`.

## Rules

- **Animation frames repaint, they never rebuild.** `render_fn` runs only
  when app state changes; transitions are advanced by the runtime between
  builds. Do not drive animations from app state.
- Every interactive component has antd-style state transitions (hover,
  press, focus, check). New components must too: keep the boolean state as
  the source of truth and ease a presented value towards it with
  `motion::motion::Animated` (see `crates/zant/src/components/button.rs` `animate`).
- Popups / overlays use a presence preset; give them a stable `key` so a
  re-added node revives instead of being duplicated.
- Node ids are recycled (nodes are freed, exiting nodes after their
  transition). Never keep a `NodeId` outside the framework.

## Presence (enter / exit)

```rust
// the key belongs to the node itself, e.g. ui::column(BoxOptions { key: Some("menu".into()), .. })
ui::styled(panel, ui::Style {
    motion: Some(zant::motion::motion::Presence::SlideUp),
    motion_origin: Some(zant::motion::motion::Origin::TopRight),
    ..Default::default()
})
```

| preset          | effect                               | enter / exit (antd)                     |
|-----------------|--------------------------------------|-----------------------------------------|
| `fade`          | opacity                              | 200ms linear                            |
| `zoom_big`      | scale 0.8→1 + opacity (modals)       | 200ms ease-out-circ / ease-in-out-circ  |
| `zoom_big_fast` | same, tooltips                       | 100ms                                   |
| `slide_up`      | vertical scale from the top edge     | 200ms ease-out-quint / ease-in-quint    |
| `slide_down`    | same from the bottom edge            | 200ms                                   |
| `move_up`       | 8px drop-in + opacity (messages)     | 200ms ease-out / ease-in-out            |

When a node with a presence leaves the spec it keeps painting while its
exit transition plays, but it is already out of layout, hit testing and
focus. `DropdownMenu` (slide-up) and the Markdown link-safety modal
(fade + zoom) use presences by default.

## Layout transitions (FLIP) and anchors

```rust
ui::styled(
    ui::column(ui::BoxOptions { key: Some("thumb".into()), ..Default::default() }),
    ui::Style {
        absolute: Some(true),
        anchor: Some(selected), // follow the n-th in-flow sibling's box
        layout_transition: Some(zant::motion::motion::Transition {
            duration_ms: 300,
            easing: zant::motion::easing::Easing::EaseInOut,
            ..Default::default()
        }),
        ..Default::default()
    },
)
```

After a layout pass a node with `layout_transition` whose parent-relative
box moved animates from the old box to the new one (its own background,
border and shadow). Supported on leaf containers. A window resize never
starts a transition. `Segmented` (thumb) and `Tabs` (ink bar) use
this.

## Built-in component motion

- Button: hover 200ms, press 100ms (ease-in-out); click wave 600ms on
  primary / default buttons.
- Input / Select: border colour and focus ring 200ms; Select popup
  slide-up.
- Checkbox: fill 300ms ease, tick pops in (ease-out-back, 100ms delay).
- Radio: dot scale 300ms ease-in-out-circ. Switch: 200ms, handle stretches
  while pressed.
- Tooltip: 450ms show delay, zoom-big-fast.

## Turning motion off

`motion::motion::set_enabled(false)` (the runtime's flag) completes every transition immediately. The
runtime sets it in its frame loop from the system setting (macOS *Reduce
motion*, Windows *Animation effects*), `ZANT_NO_MOTION=1` and
`ZANT_SNAPSHOT` (snapshots are static).

## Testing

Inject a fake clock: `runtime.clock = fake_clock`, advance a module-level
timestamp between `renderFrame` calls and assert on the presented values
(`hover_t`, `NodeAnim.fx`, `flip_delta`). See `crates/zant/tests/motion_test.rs`.
Never sleep in tests.
