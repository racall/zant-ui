//! Headless performance benchmark (no window): drives the runtime with a
//! raster canvas and measures whole frames (rebuild + layout + paint) for
//! typical heavy scenarios. Deterministic, so runs before / after a change
//! (and on macOS / Windows) are comparable.
//!
//! The whole frame is timed with `platform::monotonic_us`; the final summary
//! prints `frames`, average, p95 (index `len*95/100` after sorting) and max.
//!
//! Scenarios: open (first frame of a long chat), stream (reply streaming into
//! it), typing (an input above a long document), hover (pointer moving over a
//! long list of rows), resize (window width changing), scroll (wheel scrolling
//! the chat), caret (idle frames of a blinking caret). `ZANT_BENCH_MESSAGES`
//! sets the chat length (default 30); `ZANT_BENCH_LIST=1` renders the chat with
//! a virtual list.
//!
//! With `ZANT_FRAME_STATS=1` the runtime's per-second `[frames]` /
//! `[slow-frame]` lines are printed too (frame phases rebuild / layout /
//! paint plus the text layouts created in that frame), which is how a scenario
//! over budget is broken down into its hotspot.
#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::cmp::Ordering;
use std::rc::Rc;
use std::sync::atomic::{AtomicI64, Ordering as AtomicOrdering};

use zant::Str;
use zant::core::events::{Event, Modifiers, PointerEvent, Scroll};
use zant::core::node::ButtonSize;
use zant::flex;
use zant::markdown;
use zant::markdown::cache::{Cache, Mode};
use zant::markdown::render::options::{Caret, Options as RenderOptions};
use zant::markdown::render::view::Props as ViewProps;
use zant::platform::platform::{self, WindowOptions};
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant::ui::{
    self, App, BoxOptions, ButtonOptions, InputOptions, ListAlignment, ListOptions, ListState,
    ParagraphOptions, RunOptions, TextOptions,
};

/// The runtime clock injected into every scenario (motion is off, so this
/// only drives the caret / wake timers).
static FAKE_NOW: AtomicI64 = AtomicI64::new(1_000_000);

fn fake_clock() -> i64 {
    FAKE_NOW.load(AtomicOrdering::Relaxed)
}

fn advance_clock(ms: i64) {
    FAKE_NOW.fetch_add(ms, AtomicOrdering::Relaxed);
}

/// One chat message; `{d}` is the per-copy placeholder filled in by
/// [`format_messages`].
const MESSAGE_TEMPLATE: &str = r#"## 第 {d} 条：二次函数的最值

设 $f(x) = ax^2 + bx + c$（$a \neq 0$），配方得 $f(x) = a\left(x + \frac{b}{2a}\right)^2 + \frac{4ac-b^2}{4a}$。当 $a > 0$ 时在顶点取得**最小值**，当 $a < 0$ 时取得**最大值**；这一段文字比较长，用来测试长段落在不同宽度下的换行与排版性能，包含中文、English words 和 `inline code`。

$$
h(t) = \begin{cases} 1, & t < 0 \\ 1 - t^2, & 0 \le t \le 1 \\ 2 - 2t, & t > 1 \end{cases}
$$

1. 对称轴 $x = -\frac{b}{2a}$；
2. 比较端点 $f(m)$、$f(n)$；
3. 写出结论。

| $x$ | 0 | 1 | 2 | 3 |
| :-: | :-: | :-: | :-: | :-: |
| $f(x)$ | 3 | -3 | -5 | -3 |

```python
def f(x):
    return 2 * x**2 - 8 * x + 3

for x in range(0, 6):
    print(x, f(x))  # value {d}
```
"#;

/// One copy of the template per value; both `{d}` placeholders of a copy
/// get that value.
fn format_messages(values: &[usize]) -> String {
    let mut out = String::new();
    for &value in values {
        out.push_str(&MESSAGE_TEMPLATE.replace("{d}", &value.to_string()));
    }
    out
}

/// The long conversation, the streaming reply and the input above it. The
/// app owns it through an `Rc<RefCell<..>>` so the virtual list's item closure
/// can build messages while the scenario mutates the reply buffer.
pub struct Chat {
    texts: Vec<String>,
    caches: Vec<Cache>,
    reply: Cache,
    reply_text: String,
    input_value: String,
    with_input: bool,
    use_list: bool,
    list: Rc<RefCell<ListState>>,
}

impl Chat {
    /// `n` messages of ~5 KB each (the template four times, like real
    /// assistant replies).
    fn new(n: usize, use_list: bool) -> Chat {
        let mut texts = Vec::with_capacity(n);
        let mut caches = Vec::with_capacity(n);
        for i in 0..n {
            texts.push(format_messages(&[i, i + 1, i + 2, i + 3]));
            caches.push(Cache::new());
        }
        Chat {
            texts,
            caches,
            reply: Cache::new(),
            reply_text: String::new(),
            input_value: String::new(),
            with_input: false,
            use_list,
            list: Rc::new(RefCell::new(ListState::new())),
        }
    }

    /// Change callback of the input above the chat.
    fn on_input(&mut self, value: &str) {
        self.input_value.clear();
        self.input_value.push_str(value);
    }

    /// The Markdown body of history message `i`, without the bubble.
    fn history_content(&mut self, i: usize) -> ui::Node {
        let key = format!("m{i}");
        let caches = &mut self.caches;
        let texts = &self.texts;
        markdown::streamdown(
            &mut caches[i],
            &texts[i],
            ViewProps {
                render: RenderOptions {
                    key_prefix: Str::copy(&key),
                    content_width: 680.0,
                    font_size: 15.0,
                    ..RenderOptions::default()
                },
                ..ViewProps::default()
            },
        )
    }

    /// The Markdown body of the streaming reply (streaming mode, animating,
    /// block caret).
    fn reply_content(&mut self) -> ui::Node {
        markdown::streamdown(
            &mut self.reply,
            &self.reply_text,
            ViewProps {
                mode: Mode::Streaming,
                render: RenderOptions {
                    key_prefix: "reply".into(),
                    content_width: 680.0,
                    font_size: 15.0,
                    is_animating: true,
                    caret: Caret::Block,
                    ..RenderOptions::default()
                },
                ..ViewProps::default()
            },
        )
    }

    /// One virtual-list item (history bubble or reply).
    fn message(&mut self, i: usize) -> ui::Node {
        if i == self.texts.len() {
            return ui::column(BoxOptions {
                padding: ui::inset_xy(16.0, 12.0),
                align_items: flex::Align::Stretch,
                children: vec![self.reply_content()],
                ..BoxOptions::default()
            });
        }
        ui::column(BoxOptions {
            padding: ui::inset_xy(16.0, 12.0),
            radius: 12.0,
            background: Some(0xffff_ffff),
            align_items: flex::Align::Stretch,
            children: vec![self.history_content(i)],
            ..BoxOptions::default()
        })
    }
}

/// The application state of the chat scenarios (`Chat` plus its `App` impl).
pub struct ChatApp {
    chat: Rc<RefCell<Chat>>,
}

impl ChatApp {
    fn new(chat: Rc<RefCell<Chat>>) -> ChatApp {
        ChatApp { chat }
    }

    /// The input above the chat.
    fn input_node(chat: &Rc<RefCell<Chat>>) -> ui::Node {
        let value = chat.borrow().input_value.clone();
        ui::input(InputOptions {
            key: Some("input".into()),
            value: Some(Str::copy(&value)),
            placeholder: "Type…".into(),
            on_change: Some(ui::text_action(|app: &mut ChatApp, value: &str| {
                app.chat.borrow_mut().on_input(value);
            })),
            ..InputOptions::default()
        })
    }
}

impl App for ChatApp {
    fn render(&mut self) -> ui::Node {
        let chat = Rc::clone(&self.chat);
        let (use_list, with_input, n, reply_len) = {
            let c = chat.borrow();
            (c.use_list, c.with_input, c.texts.len(), c.reply_text.len())
        };

        if use_list {
            let mut top: Vec<ui::Node> = Vec::new();
            if with_input {
                top.push(ChatApp::input_node(&chat));
            }
            let list_state = { chat.borrow().list.clone() };
            let count = n + if reply_len > 0 { 1 } else { 0 };
            let item_chat = Rc::clone(&chat);
            top.push(ui::list(
                &list_state,
                ListOptions {
                    key: "chat".into(),
                    count,
                    item: Rc::new(move |index| item_chat.borrow_mut().message(index)),
                    alignment: ListAlignment::Bottom,
                    follow_tail: true,
                    gap: 16.0,
                    padding: ui::inset(20.0),
                    ..ListOptions::default()
                },
            ));
            return ui::column(BoxOptions {
                grow: 1.0,
                align_items: flex::Align::Stretch,
                background: Some(0xfff5_f5f5),
                children: top,
                ..BoxOptions::default()
            });
        }

        let mut rows: Vec<ui::Node> = Vec::with_capacity(n + 1);
        for i in 0..n {
            let key = format!("m{i}");
            let content = chat.borrow_mut().history_content(i);
            rows.push(ui::column(BoxOptions {
                key: Some(Str::copy(&key)),
                padding: ui::inset_xy(16.0, 12.0),
                radius: 12.0,
                background: Some(0xffff_ffff),
                align_items: flex::Align::Stretch,
                children: vec![content],
                ..BoxOptions::default()
            }));
        }
        if reply_len > 0 {
            let content = chat.borrow_mut().reply_content();
            rows.push(ui::column(BoxOptions {
                key: Some("reply-box".into()),
                padding: ui::inset_xy(16.0, 12.0),
                align_items: flex::Align::Stretch,
                children: vec![content],
                ..BoxOptions::default()
            }));
        }
        let mut top: Vec<ui::Node> = Vec::new();
        if with_input {
            top.push(ChatApp::input_node(&chat));
        }
        top.push(ui::column(BoxOptions {
            key: Some("chat".into()),
            grow: 1.0,
            shrink: 1.0,
            basis: Some(0.0),
            scroll: true,
            padding: ui::inset(20.0),
            gap: 16.0,
            align_items: flex::Align::Stretch,
            children: rows,
            ..BoxOptions::default()
        }));
        ui::column(BoxOptions {
            grow: 1.0,
            align_items: flex::Align::Stretch,
            background: Some(0xfff5_f5f5),
            children: top,
            ..BoxOptions::default()
        })
    }
}

/// 400 rows (text, paragraph, button) in a scroll container, for
/// the pointer-hover scenario.
struct Rows;

impl App for Rows {
    fn render(&mut self) -> ui::Node {
        let mut rows: Vec<ui::Node> = Vec::with_capacity(400);
        for i in 0..400 {
            rows.push(ui::row(BoxOptions {
                key: Some(Str::copy(&format!("r{i}"))),
                gap: 8.0,
                padding: ui::inset_xy(12.0, 6.0),
                align_items: flex::Align::Center,
                children: vec![
                    ui::text(
                        format!("Row {i}"),
                        TextOptions {
                            size: 13.0,
                            ..TextOptions::default()
                        },
                    ),
                    ui::paragraph(
                        "A description that is long enough to wrap in narrow windows and exercise text layout.",
                        ParagraphOptions {
                            size: 12.0,
                            ..ParagraphOptions::default()
                        },
                    ),
                    ui::button(ButtonOptions {
                        text: "Open".into(),
                        size: ButtonSize::Small,
                        ..ButtonOptions::default()
                    }),
                ],
                ..BoxOptions::default()
            }));
        }
        ui::column(BoxOptions {
            grow: 1.0,
            align_items: flex::Align::Stretch,
            children: vec![ui::column(BoxOptions {
                key: Some("list".into()),
                grow: 1.0,
                basis: Some(0.0),
                scroll: true,
                align_items: flex::Align::Stretch,
                children: rows,
                ..BoxOptions::default()
            })],
            ..BoxOptions::default()
        })
    }
}

/// Per-scenario frame samples, sorted for the p95.
struct Stats {
    samples: Vec<f64>,
}

impl Stats {
    fn new() -> Stats {
        Stats {
            samples: Vec::new(),
        }
    }

    fn add(&mut self, ms: f64) {
        self.samples.push(ms);
    }

    /// Prints the `<name> frames=…  avg=… ms  p95=…` summary line.
    fn report(&mut self, name: &str) {
        let s = &mut self.samples;
        if s.is_empty() {
            return;
        }
        s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
        let sum: f64 = s.iter().sum();
        let p95 = s[usize::min(s.len() - 1, (s.len() * 95) / 100)];
        eprintln!(
            "{name:<8} frames={:>5}  avg={:>7.2} ms  p95={:>7.2} ms  max={:>7.2} ms",
            s.len(),
            sum / s.len() as f64,
            p95,
            s[s.len() - 1]
        );
    }
}

const W: f32 = 900.0;
const H: f32 = 900.0;

/// Advances the fake clock, then times one full frame
/// (`begin` → `render_frame` → `end`) in milliseconds.
fn frame(rt: &mut Runtime, renderer: &mut Renderer, w: f32, h: f32) -> f64 {
    advance_clock(16);
    let t0 = platform::monotonic_us();
    let canvas = renderer.begin(1.0).expect("a raster canvas");
    rt.render_frame(canvas, w, h, 1.0);
    renderer.end();
    (platform::monotonic_us() - t0) as f64 / 1000.0
}

/// A headless runtime at 900×900 with the fake clock.
fn new_runtime(app: impl App) -> Runtime {
    let mut rt = Runtime::init(
        app,
        RunOptions {
            window: WindowOptions {
                width: W,
                height: H,
                ..WindowOptions::default()
            },
            ..RunOptions::default()
        },
    );
    rt.clock = fake_clock;
    // The benchmark never calls `Runtime::run` (which reads the env var), so
    // `ZANT_FRAME_STATS=1` is wired in here: the per-second `[frames]` /
    // `[slow-frame]` lines are the phase breakdown used to investigate a
    // scenario over budget.
    if frame_stats_enabled() {
        rt.enable_frame_stats();
    }
    rt
}

/// Whether to run scenario `name`: with no scenario arguments, all of them.
fn want(args: &[String], name: &str) -> bool {
    args.is_empty() || args.iter().any(|arg| arg == name)
}

/// `ZANT_FRAME_STATS` set to a value starting with `1`.
fn frame_stats_enabled() -> bool {
    std::env::var("ZANT_FRAME_STATS")
        .map(|value| value.starts_with('1'))
        .unwrap_or(false)
}

pub fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let n = std::env::var("ZANT_BENCH_MESSAGES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(30);
    zant::motion::motion::set_enabled(false);
    let bench_list = std::env::var_os("ZANT_BENCH_LIST").is_some();
    let mut renderer = Renderer::create_raster(1400, 1000).expect("a raster renderer");
    eprintln!(
        "zant-bench: {n} messages, raster canvas {W}x{H}{}",
        if bench_list { ", virtual list" } else { "" }
    );

    if want(&args, "open") {
        let mut st = Stats::new();
        for _ in 0..5 {
            let chat = Rc::new(RefCell::new(Chat::new(n, bench_list)));
            let mut rt = new_runtime(ChatApp::new(chat));
            st.add(frame(&mut rt, &mut renderer, W, H));
            drop(rt);
        }
        st.report("open");
    }

    if want(&args, "stream") {
        let mut st = Stats::new();
        let chat = Rc::new(RefCell::new(Chat::new(n, bench_list)));
        let mut rt = new_runtime(ChatApp::new(Rc::clone(&chat)));
        let _ = frame(&mut rt, &mut renderer, W, H);
        let reply = format_messages(&[99, 98]);
        let bytes = reply.as_bytes();
        let mut i: usize = 0;
        while i < bytes.len() {
            let mut end = usize::min(bytes.len(), i + 8);
            while end < bytes.len() && (bytes[end] & 0xC0) == 0x80 {
                end += 1;
            }
            chat.borrow_mut().reply_text.push_str(&reply[i..end]);
            i = end;
            rt.invalidate();
            ui::scroll_to_end("chat");
            st.add(frame(&mut rt, &mut renderer, W, H));
        }
        drop(rt);
        st.report("stream");
    }

    if want(&args, "typing") {
        let mut st = Stats::new();
        let chat = Rc::new(RefCell::new(Chat::new(n, bench_list)));
        chat.borrow_mut().with_input = true;
        let mut rt = new_runtime(ChatApp::new(Rc::clone(&chat)));
        let _ = frame(&mut rt, &mut renderer, W, H);
        if let Some(b) = rt.tree.bounds_of_key("input") {
            let x = b.x + 10.0;
            let y = b.y + b.height / 2.0;
            let _ = rt.dispatch(Event::PointerDown(PointerEvent {
                x,
                y,
                ..PointerEvent::default()
            }));
            let _ = rt.dispatch(Event::PointerUp(PointerEvent {
                x,
                y,
                ..PointerEvent::default()
            }));
        }
        let _ = frame(&mut rt, &mut renderer, W, H);
        for _ in 0..200 {
            let _ = rt.dispatch(Event::TextInput(String::from("a")));
            st.add(frame(&mut rt, &mut renderer, W, H));
        }
        drop(rt);
        st.report("typing");
    }

    if want(&args, "caret") {
        let mut st = Stats::new();
        let chat = Rc::new(RefCell::new(Chat::new(n, bench_list)));
        chat.borrow_mut().with_input = true;
        let mut rt = new_runtime(ChatApp::new(Rc::clone(&chat)));
        let _ = frame(&mut rt, &mut renderer, W, H);
        if let Some(b) = rt.tree.bounds_of_key("input") {
            let _ = rt.dispatch(Event::PointerDown(PointerEvent {
                x: b.x + 10.0,
                y: b.y + b.height / 2.0,
                ..PointerEvent::default()
            }));
        }
        let _ = frame(&mut rt, &mut renderer, W, H);
        for _ in 0..100 {
            // one blink
            advance_clock(530);
            st.add(frame(&mut rt, &mut renderer, W, H));
        }
        drop(rt);
        st.report("caret");
    }

    if want(&args, "hover") {
        let mut st = Stats::new();
        let mut rt = new_runtime(Rows);
        let _ = frame(&mut rt, &mut renderer, W, H);
        let mut y: f32 = 10.0;
        for k in 0..300 {
            let x: f32 = if k % 2 == 0 { 860.0 } else { 300.0 };
            let _ = rt.dispatch(Event::PointerMove(PointerEvent {
                x,
                y,
                ..PointerEvent::default()
            }));
            y += 2.5;
            st.add(frame(&mut rt, &mut renderer, W, H));
        }
        drop(rt);
        st.report("hover");
    }

    if want(&args, "resize") {
        let mut st = Stats::new();
        let chat = Rc::new(RefCell::new(Chat::new(n, bench_list)));
        let mut rt = new_runtime(ChatApp::new(Rc::clone(&chat)));
        let _ = frame(&mut rt, &mut renderer, W, H);
        for k in 0..120 {
            let w = 700.0 + (k % 60) as f32 * 8.0;
            st.add(frame(&mut rt, &mut renderer, w, H));
        }
        drop(rt);
        st.report("resize");
    }

    if want(&args, "scroll") {
        let mut st = Stats::new();
        let chat = Rc::new(RefCell::new(Chat::new(n, bench_list)));
        let mut rt = new_runtime(ChatApp::new(Rc::clone(&chat)));
        let _ = frame(&mut rt, &mut renderer, W, H);
        for _ in 0..300 {
            let _ = rt.dispatch(Event::Scroll(Scroll {
                x: 400.0,
                y: 400.0,
                dx: 0.0,
                dy: -60.0,
                modifiers: Modifiers::default(),
            }));
            st.add(frame(&mut rt, &mut renderer, W, H));
        }
        drop(rt);
        st.report("scroll");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Four template copies per message, Chinese + English + math + table +
    /// code block all present.
    #[test]
    fn message_template_is_filled_in_per_copy() {
        let message = format_messages(&[7, 8, 9, 10]);
        assert_eq!(message.matches("## 第 ").count(), 4);
        assert!(message.contains("第 7 条"));
        assert!(message.contains("第 8 条"));
        assert!(message.contains("第 9 条"));
        assert!(message.contains("第 10 条"));
        // braces in the template are literal (no `{{` / `}}` escapes)
        assert!(message.contains(r"\frac{b}{2a}"));
        assert!(message.contains(r"\begin{cases}"));
        assert!(message.contains("# value 10"));
        assert!(!message.contains("{{"));
        assert!(!message.contains("{d}"));
    }

    /// p95 is `s[min(len - 1, len * 95 / 100)]` on the sorted list.
    #[test]
    fn p95_uses_the_clamped_95_percent_index() {
        let mut stats = Stats::new();
        for i in 1..=100 {
            stats.add(f64::from(i));
        }
        let s = &mut stats.samples;
        s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
        // 0-based index `len * 95 / 100` in the sorted list: the 96th of 100.
        assert_eq!(s[usize::min(s.len() - 1, (s.len() * 95) / 100)], 96.0);
    }

    /// The chat has `n` messages of ~5 KB (`message_template` × 4).
    #[test]
    fn chat_builds_five_kilobyte_messages() {
        let chat = Chat::new(3, false);
        assert_eq!(chat.texts.len(), 3);
        for (i, text) in chat.texts.iter().enumerate() {
            assert!(
                text.len() > 2_500,
                "message {i} is only {} bytes",
                text.len()
            );
            assert!(text.len() < 4_000, "message {i} is {} bytes", text.len());
        }
    }
}
