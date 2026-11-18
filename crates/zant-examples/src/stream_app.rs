//! `zant-md-stream`: the streaming Markdown benchmark, a chat with finished
//! history messages and one assistant reply that streams in, like an AI chat
//! over SSE.
//!
//! ```text
//! zant-md-stream                         # built-in long reply, simulated deltas
//! zant-md-stream reply.md                # stream a Markdown file
//! curl -N ... /chat | zant-md-stream --sse        # real SSE from stdin
//! zant-md-stream --sse-replay recorded.sse        # recorded SSE, one event per ZANT_STREAM_MS
//! ```
//!
//! Environment: `ZANT_STREAM_CHARS`,
//! `ZANT_STREAM_MS`, `ZANT_STREAM_HISTORY`, `ZANT_STREAM_EXIT`,
//! `ZANT_STREAM_WIDTH`, `ZANT_FRAME_STATS`, `ZANT_DARK`.
//!
//! The stream runs on a worker thread (like a network reader); it appends to a
//! shared buffer and calls `ui::post_wake()`; each build renders the whole buffer
//! with `markdown::streamdown` (block cache + remend). The virtual list only
//! builds the messages in view and follows the reply while it streams.
#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use zant::Str;
use zant::flex;
use zant::markdown::cache::{Cache, Mode};
use zant::markdown::parser::Options as ParseOptions;
use zant::markdown::render::options::{Caret, Options as RenderOptions};
use zant::markdown::render::view::Props as ViewProps;
use zant::markdown::{self};
use zant::platform::platform::{self, WindowOptions};
use zant::renderer::text::Weight;
use zant::theme::tokens::{self, Theme};
use zant::ui::{self, BoxOptions, ListAlignment, ListOptions, ListState, RunOptions, TextOptions};

/// The built-in long reply that is streamed when no source is given.
pub const BUILTIN_REPLY: &str = r#"## 二次函数的最值问题

设函数 $f(x) = ax^2 + bx + c$（其中 $a \neq 0$），我们来系统地分析它的**最大值与最小值**。首先回顾配方：

$$
f(x) = a\left(x + \frac{b}{2a}\right)^2 + \frac{4ac - b^2}{4a}
$$

由此可得顶点坐标为 $\left(-\frac{b}{2a}, \frac{4ac-b^2}{4a}\right)$。当 $a > 0$ 时抛物线开口向上，函数在顶点处取得**最小值**；当 $a < 0$ 时开口向下，取得**最大值**。

### 例题

> 已知 $f(x) = 2x^2 - 8x + 3$，求 $f(x)$ 在区间 $[0, 5]$ 上的最值。

1. 对称轴 $x = -\frac{b}{2a} = 2$，在区间 $[0,5]$ 内；
2. 最小值 $f(2) = 2 \cdot 4 - 16 + 3 = -5$；
3. 比较端点：$f(0) = 3$，$f(5) = 50 - 40 + 3 = 13$，所以最大值是 $13$。

| $x$ | 0 | 1 | 2 | 3 | 4 | 5 |
| :-: | :-: | :-: | :-: | :-: | :-: | :-: |
| $f(x)$ | 3 | -3 | -5 | -3 | 3 | 13 |

下面用 Python 验证结果：

```python
import numpy as np

def f(x):
    return 2 * x**2 - 8 * x + 3

xs = np.linspace(0, 5, 501)
ys = f(xs)
print("min:", ys.min(), "at", xs[ys.argmin()])
print("max:", ys.max(), "at", xs[ys.argmax()])
# 输出: min: -5.0 at 2.0, max: 13.0 at 5.0
```

### 一般结论

对于闭区间 $[m, n]$ 上的二次函数，最值只可能出现在**顶点**或**端点**处。设对称轴为 $x_0 = -\frac{b}{2a}$，分三种情况讨论：

- 若 $x_0 < m$：函数在区间上单调，最值在两个端点取得；
- 若 $m \le x_0 \le n$：一个最值在顶点，另一个在离顶点较远的端点；
- 若 $x_0 > n$：同样在端点取得。

这个结论可以推广到**含参数的二次函数**：例如 $g(x) = x^2 - 2tx + 1$ 在 $[0, 1]$ 上的最小值 $h(t)$ 是一个分段函数：

$$
h(t) = \begin{cases} 1, & t < 0 \\ 1 - t^2, & 0 \le t \le 1 \\ 2 - 2t, & t > 1 \end{cases}
$$

掌握了“定轴动区间”与“动轴定区间”两类问题，就能解决绝大多数二次函数最值题。下一步可以练习几道综合题，巩固这种**分类讨论**的思路。
"#;

/// The finished history messages (user / assistant alternating).
pub const HISTORY_SAMPLES: [&str; 4] = [
    "帮我讲一下二次函数的最值问题，最好带例题。",
    "当然可以！二次函数 $y = ax^2+bx+c$ 的图像是抛物线，**开口方向**由 $a$ 决定，对称轴是 $x=-\\frac{b}{2a}$。\n\n- $a>0$：开口向上，有最小值\n- $a<0$：开口向下，有最大值\n\n我们先看一个例子：$y = x^2 - 4x + 3 = (x-2)^2 - 1$，最小值为 $-1$。",
    "那如果区间不包含顶点呢？",
    "这时函数在区间上是**单调**的，最值在端点取得。例如 $y=(x-2)^2-1$ 在 $[3,5]$ 上递增：\n\n| x | 3 | 4 | 5 |\n|---|---|---|---|\n| y | 0 | 3 | 8 |\n\n所以最小值 $0$，最大值 $8$。",
];

/// Worker-shared buffer: appended under the mutex, read by the UI thread.
#[derive(Default)]
pub struct SharedInner {
    pub text: Vec<u8>,
    pub done: bool,
    pub deltas: usize,
}

/// The mutex-guarded shared buffer.
pub type Shared = Arc<Mutex<SharedInner>>;

fn lock(shared: &Shared) -> MutexGuard<'_, SharedInner> {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn env_present(name: &str) -> bool {
    std::env::var_os(name).is_some()
}

fn env_parse<T: std::str::FromStr>(name: &str) -> Option<T> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
}

/// The stream source.
enum Source {
    /// Simulated deltas from a document.
    Text(Vec<u8>),
    /// Server-sent events on stdin: `data: {"type":"text_delta","content":"…"}`.
    Sse,
    /// A recorded SSE stream, replayed one event per interval.
    SseReplay(Vec<u8>),
}

fn read_file(path: &str) -> zant::Result<Vec<u8>> {
    Ok(std::fs::read(path)?)
}

/// Appends one delta to the shared buffer and wakes the UI.
pub fn append_delta(shared: &Shared, bytes: &[u8]) {
    {
        let mut inner = lock(shared);
        inner.text.extend_from_slice(bytes);
        inner.deltas += 1;
    }
    ui::post_wake();
}

/// Marks the stream as finished and wakes the UI.
pub fn finish(shared: &Shared) {
    {
        let mut inner = lock(shared);
        inner.done = true;
    }
    ui::post_wake();
}

fn sleep_ms(ms: u32) {
    std::thread::sleep(Duration::from_millis(u64::from(ms)));
}

/// Waits for the window, then appends `chars` bytes at a time.
fn simulate(shared: Shared, text: Vec<u8>, chars: usize, ms: u32) {
    sleep_ms(400);
    let mut i: usize = 0;
    while i < text.len() {
        let mut end = text.len().min(i + chars);
        // never split a UTF-8 sequence (deltas are whole characters)
        while end < text.len() && (text[end] & 0xC0) == 0x80 {
            end += 1;
        }
        append_delta(&shared, &text[i..end]);
        i = end;
        sleep_ms(ms);
    }
    finish(&shared);
}

/// Reads SSE from stdin and appends the `content` of `text_delta` events.
fn read_sse(shared: Shared) {
    let mut line: Vec<u8> = Vec::new();
    let mut buf = [0u8; 4096];
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    loop {
        let n = match std::io::Read::read(&mut input, &mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        for &byte in &buf[..n] {
            if byte != b'\n' {
                line.push(byte);
                continue;
            }
            let text = String::from_utf8_lossy(&line);
            handle_sse_line(&shared, text.trim_matches(|c| c == ' ' || c == '\r'));
            line.clear();
        }
    }
    finish(&shared);
}

/// Replays recorded SSE lines, one event per `ms`.
fn replay_sse(shared: Shared, recorded: Vec<u8>, ms: u32) {
    sleep_ms(400);
    for raw in recorded.split(|&byte| byte == b'\n') {
        let text = String::from_utf8_lossy(raw);
        let line = text.trim_matches(|c| c == ' ' || c == '\r');
        if !line.starts_with("data:") {
            continue;
        }
        handle_sse_line(&shared, line);
        sleep_ms(ms);
    }
    finish(&shared);
}

/// Handles one SSE line: a `data:` line with a `text_delta` appends its content.
pub fn handle_sse_line(shared: &Shared, line: &str) {
    let Some(rest) = line.strip_prefix("data:") else {
        return;
    };
    let json = rest.trim_matches(' ');
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return;
    };
    let Some(object) = value.as_object() else {
        return;
    };
    let Some(kind) = object.get("type").and_then(|value| value.as_str()) else {
        return;
    };
    if kind == "text_delta" {
        if let Some(content) = object.get("content").and_then(|value| value.as_str()) {
            append_delta(shared, content.as_bytes());
        }
    } else if kind == "turn_reset" {
        {
            let mut inner = lock(shared);
            inner.text.clear();
        }
        ui::post_wake();
    }
}

/// The data the render/list keeps: the copy of the shared text the UI renders
/// (taken under the lock), the reply cache and the history caches/texts.
pub struct StreamView {
    text: Vec<u8>,
    reply: Cache,
    history: Vec<Cache>,
    history_text: Vec<String>,
}

impl StreamView {
    fn history_len(&self) -> usize {
        self.history.len()
    }
}

/// The chat application state.
pub struct StreamApp {
    shared: Shared,
    view: Rc<RefCell<StreamView>>,
    dark: bool,
    exit_when_done: bool,
    started_ms: i64,
    finished_ms: i64,
    builds: usize,
    build_us_total: i64,
    build_us_max: i64,
    streaming: bool,
    exited: bool,
    done_now: bool,
    list: Rc<RefCell<ListState>>,
}

impl StreamApp {
    pub fn new(shared: Shared, view: StreamView, dark: bool, exit_when_done: bool) -> StreamApp {
        StreamApp {
            shared,
            view: Rc::new(RefCell::new(view)),
            dark,
            exit_when_done,
            started_ms: 0,
            finished_ms: 0,
            builds: 0,
            build_us_total: 0,
            build_us_max: 0,
            streaming: true,
            exited: false,
            done_now: false,
            list: Rc::new(RefCell::new(ListState::new())),
        }
    }

    /// Prints the end-of-stream timing summary.
    fn summary(&self, deltas: usize) {
        let elapsed = self.finished_ms - self.started_ms;
        let avg = if self.builds > 0 {
            self.build_us_total as f64 / self.builds as f64 / 1000.0
        } else {
            0.0
        };
        eprintln!(
            "[stream] done: {} bytes, {} deltas, {} ms, {} builds, build avg {:.2} ms, max {:.2} ms",
            self.view.borrow().text.len(),
            deltas,
            elapsed,
            self.builds,
            avg,
            self.build_us_max as f64 / 1000.0,
        );
    }

    /// Headless constructor of the smoke test: `history` messages and a reply
    /// buffer seeded with `reply` (no worker thread; the test appends through
    /// [`append_delta`] / [`finish`]).
    pub fn headless(history: Vec<String>, reply: &str) -> (StreamApp, Shared) {
        let shared: Shared = Arc::new(Mutex::new(SharedInner {
            text: reply.as_bytes().to_vec(),
            done: false,
            deltas: 0,
        }));
        let caches = history.iter().map(|_| Cache::new()).collect();
        let view = StreamView {
            text: Vec::new(),
            reply: Cache::new(),
            history: caches,
            history_text: history,
        };
        (StreamApp::new(shared.clone(), view, false, false), shared)
    }

    /// The window tree, without the build timing (the caller in
    /// [`StreamApp::render`] measures around it).
    fn build(&mut self) -> ui::Node {
        if self.started_ms == 0 {
            self.started_ms = ui::now_ms();
        }
        let (done, deltas) = {
            let shared = lock(&self.shared);
            let mut view = self.view.borrow_mut();
            if shared.text.len() != view.text.len() {
                view.text.clear();
                view.text.extend_from_slice(&shared.text);
            }
            (shared.done, shared.deltas)
        };
        if done && self.streaming {
            self.streaming = false;
            self.finished_ms = ui::now_ms();
            self.summary(deltas);
        }
        if done && self.exit_when_done && !self.exited {
            self.exited = true;
            std::process::exit(0);
        }
        let theme = ui::current_theme();
        self.done_now = done;
        let bytes = self.view.borrow().text.len();
        let dark = self.dark;
        let done_now = self.done_now;
        let view = Rc::clone(&self.view);
        let count = self.view.borrow().history_len() + 1;
        ui::column(BoxOptions {
            grow: 1.0,
            align_items: flex::Align::Stretch,
            background: Some(theme.color_bg_layout),
            children: vec![
                ui::row(BoxOptions {
                    padding: ui::inset_xy(20.0, 10.0),
                    gap: 12.0,
                    align_items: flex::Align::Center,
                    children: vec![
                        ui::text(
                            "Streaming Markdown",
                            TextOptions {
                                size: 14.0,
                                weight: Weight::Semibold,
                                ..TextOptions::default()
                            },
                        ),
                        ui::text(
                            format!(
                                "{bytes} bytes · {deltas} deltas{}",
                                if done { " · done" } else { "" }
                            ),
                            TextOptions {
                                size: 12.0,
                                color: Some(theme.color_text_secondary),
                                ..TextOptions::default()
                            },
                        ),
                    ],
                    ..BoxOptions::default()
                }),
                // a virtual list: only the messages in view are built and laid
                // out; it follows the reply while it streams
                ui::list(
                    &self.list,
                    ListOptions {
                        key: "chat".into(),
                        count,
                        item: Rc::new(move |index| {
                            let mut view = view.borrow_mut();
                            message(&mut view, dark, done_now, index)
                        }),
                        alignment: ListAlignment::Bottom,
                        follow_tail: true,
                        gap: 16.0,
                        padding: ui::inset_xy(24.0, 16.0),
                        selectable: true,
                        ..ListOptions::default()
                    },
                ),
            ],
            ..BoxOptions::default()
        })
    }
}

impl ui::App for StreamApp {
    fn render(&mut self) -> ui::Node {
        let start = platform::monotonic_us();
        let node = self.build();
        let elapsed = platform::monotonic_us() - start;
        self.builds += 1;
        self.build_us_total += elapsed;
        self.build_us_max = self.build_us_max.max(elapsed);
        node
    }

    fn theme(&self) -> Option<Theme> {
        Some(if self.dark {
            tokens::ant_dark()
        } else {
            tokens::ant_light()
        })
    }
}

/// One history message or the streaming reply.
fn message(view: &mut StreamView, dark: bool, done_now: bool, index: usize) -> ui::Node {
    let theme = ui::current_theme();
    let markdown_theme = if dark {
        markdown::theme::dark()
    } else {
        markdown::theme::light()
    };
    if index < view.history.len() {
        let key = format!("h{index}");
        let props = ViewProps {
            parse: ParseOptions {
                latex_delimiters: true,
                ..ParseOptions::default()
            },
            render: RenderOptions {
                theme: markdown_theme,
                key_prefix: Str::copy(&key),
                content_width: 680.0,
                font_size: 15.0,
                selectable: false,
                ..RenderOptions::default()
            },
            ..ViewProps::default()
        };
        let content =
            markdown::streamdown(&mut view.history[index], &view.history_text[index], props);
        return bubble(&key, index.is_multiple_of(2), theme, content);
    }
    let props = ViewProps {
        mode: Mode::Streaming,
        // Sia's replies (like most model output) use \( \) and \[ \]
        parse: ParseOptions {
            latex_delimiters: true,
            ..ParseOptions::default()
        },
        render: RenderOptions {
            theme: markdown_theme,
            key_prefix: "reply".into(),
            content_width: 680.0,
            font_size: 15.0,
            is_animating: !done_now,
            caret: Caret::Block,
            selectable: false,
            ..RenderOptions::default()
        },
        ..ViewProps::default()
    };
    // disjoint field borrows: the reply cache and the text buffer it renders
    let content = if let Ok(source) = std::str::from_utf8(&view.text) {
        markdown::streamdown(&mut view.reply, source, props)
    } else {
        let source = String::from_utf8_lossy(&view.text).into_owned();
        markdown::streamdown(&mut view.reply, &source, props)
    };
    bubble("reply", false, theme, content)
}

/// A chat bubble aligned at the start (assistant) or end (user).
fn bubble(key: &str, user: bool, theme: Theme, content: ui::Node) -> ui::Node {
    ui::row(BoxOptions {
        key: Some(Str::copy(key)),
        width_percent: Some(100.0),
        justify: if user {
            flex::Justify::End
        } else {
            flex::Justify::Start
        },
        children: vec![ui::column(BoxOptions {
            max_width: Some(760.0),
            grow: if user { 0.0 } else { 1.0 },
            shrink: 1.0,
            padding: ui::inset_xy(16.0, 12.0),
            radius: 12.0,
            background: Some(if user {
                theme.color_primary_bg
            } else {
                theme.color_bg_container
            }),
            align_items: flex::Align::Stretch,
            children: vec![content],
            ..BoxOptions::default()
        })],
        ..BoxOptions::default()
    })
}

/// Window title, size (`width`×900) and minimum size (600×500).
pub fn run_options(width: f32) -> RunOptions {
    RunOptions {
        title: String::from("Zant streaming Markdown"),
        window: WindowOptions {
            width,
            height: 900.0,
            min_width: 600.0,
            min_height: 500.0,
            ..WindowOptions::default()
        },
        ..RunOptions::default()
    }
}

/// The example entry point: picks the source from the arguments and runs.
pub fn run() -> zant::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut source = Source::Text(BUILTIN_REPLY.as_bytes().to_vec());
    let mut arg = 0;
    while arg < args.len() {
        let value = &args[arg];
        if value == "--sse" {
            source = Source::Sse;
        } else if value == "--sse-replay" && arg + 1 < args.len() {
            arg += 1;
            source = Source::SseReplay(read_file(&args[arg])?);
        } else {
            source = Source::Text(read_file(value)?);
        }
        arg += 1;
    }
    let chars = env_parse::<usize>("ZANT_STREAM_CHARS").unwrap_or(6);
    let ms = env_parse::<u32>("ZANT_STREAM_MS").unwrap_or(16);
    let history_n = env_parse::<usize>("ZANT_STREAM_HISTORY").unwrap_or(8);
    let has_ms_env = env_present("ZANT_STREAM_MS");
    let dark = env_present("ZANT_DARK");
    let exit_when_done = env_present("ZANT_STREAM_EXIT");
    let width = env_parse::<f32>("ZANT_STREAM_WIDTH").unwrap_or(900.0);

    // the source text of the odd (assistant) history messages for `.text`
    let source_text = match &source {
        Source::Text(text) => Some(String::from_utf8_lossy(text).into_owned()),
        Source::Sse | Source::SseReplay(_) => None,
    };
    let mut history: Vec<Cache> = Vec::with_capacity(history_n);
    let mut history_text: Vec<String> = Vec::with_capacity(history_n);
    for i in 0..history_n {
        history.push(Cache::new());
        // long finished replies interleaved with short user messages
        let text = if i.is_multiple_of(2) {
            HISTORY_SAMPLES[(i / 2 * 2) % HISTORY_SAMPLES.len()].to_string()
        } else {
            match &source_text {
                Some(text) => text.clone(),
                None => BUILTIN_REPLY.to_string(),
            }
        };
        history_text.push(text);
    }

    let shared: Shared = Arc::new(Mutex::new(SharedInner::default()));
    match source {
        Source::Text(text) => {
            let worker = Arc::clone(&shared);
            let _ = std::thread::spawn(move || simulate(worker, text, chars, ms));
        }
        Source::Sse => {
            let worker = Arc::clone(&shared);
            let _ = std::thread::spawn(move || read_sse(worker));
        }
        Source::SseReplay(recorded) => {
            let worker = Arc::clone(&shared);
            let interval = if has_ms_env { ms } else { 25 };
            let _ = std::thread::spawn(move || replay_sse(worker, recorded, interval));
        }
    }
    let view = StreamView {
        text: Vec::new(),
        reply: Cache::new(),
        history,
        history_text,
    };
    ui::run(
        StreamApp::new(shared, view, dark, exit_when_done),
        run_options(width),
    )
}
