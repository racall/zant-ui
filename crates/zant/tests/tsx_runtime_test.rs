//! Contract tests of the TSX runtime support API (`zant::tsx`): the shapes
//! generated Rust code compiles against, driven through the public crate API
//! only.
//!
//! They also cover the behavior that needs a real caller: copy-on-write arrays,
//! the JS string / number edge cases, child ids pinned to fixed values,
//! the timer scheduler with a fake clock, the `Shell` around a root component
//! (wake → timers → root hook) and the desktop helpers with the real system
//! disabled.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use zant::Str;
use zant::tsx;
use zant::ui;

// ---------------------------------------------------------------- the API

#[test]
fn the_generated_code_api_has_the_frozen_shapes() {
    // type mapping (tsx-codegen.md §2)
    let _: tsx::List<u32> = tsx::List::from_vec(vec![1, 2]);
    let _: Str = "text".into();
    let _: Option<Str> = None;
    let _: tsx::Any = tsx::Any::from(3u32);
    let _: tsx::Bytes = Rc::from(vec![1u8, 2]);
    let _: tsx::Ref<u32> = tsx::new_ref(1);
    let _: tsx::Callback<dyn Fn(u32)> = Rc::new(|_value: u32| {});

    // child ids / hooks (§6.2, §6.1)
    let _: fn(u64, &str, u64) -> u64 = tsx::child_id::<u64>;
    let _: fn(u64, &str, u32) -> u64 = tsx::site_id;
    let _: tsx::Memo<u32, (u32,)> = tsx::Memo::new();
    let _: tsx::Deps<(Str,)> = tsx::Deps::new();
    let _: tsx::Effect<()> = tsx::Effect::new();
    let store: tsx::SharedStore<u32> = tsx::create_store(1);
    assert_eq!(*store.borrow().get(), 1);
    let _: tsx::Store<u32> = tsx::Store::new(0);

    // strings / builtins / timers (§8)
    let _: fn(&str, f64, Option<f64>) -> Str = tsx::strings::slice;
    let _: fn(&str, &str) -> tsx::List<Str> = tsx::strings::split;
    let _: fn(f64) -> Str = tsx::builtins::string::<f64>;
    let _: fn(&str) -> f64 = tsx::builtins::parse_float;
    let _: fn(&str, f64) -> f64 = tsx::builtins::parse_int;
    let _: fn(f64, f64) -> Str = tsx::builtins::to_fixed;
    let _: fn(tsx::builtins::Unary, f64) -> f64 = tsx::builtins::unary::<f64>;
    let _: fn(&[tsx::Any]) = tsx::log;
    let _: fn(i64, &mut ui::ActionCx<'_>) -> bool = tsx::timers::fire;
    let _: fn(f64, ui::Action) -> f64 = tsx::timers::set_timeout;
    let _: fn(f64) = tsx::timers::clear_timeout;

    // platform / desktop / native / text (§9)
    let _: fn() -> Str = tsx::platform::clipboard::text;
    let _: fn() = tsx::platform::main_window::show;
    let _: fn(&str) -> Str = tsx::platform::key_label;
    let _: fn(tsx::desktop::TrayOptions) -> bool = tsx::desktop::update;
    let _: fn(&str) -> Option<tsx::desktop::Shortcut> = tsx::desktop::parse_shortcut;
    let _: fn(tsx::text::SpanStyle) -> zant::core::node::RichSpan =
        |style| tsx::text::span("x", style);
    let _ = tsx::native::registered("missing");
    let _ = tsx::platform::IS_MACOS;
    let _ = tsx::platform::IS_WINDOWS;
}

// ------------------------------------------------------------------- list

#[test]
fn list_is_copy_on_write_and_supports_the_array_methods() {
    let mut rows = tsx::List::from_vec(vec![1, 2]);
    let snapshot = rows.clone();
    rows.push(3);
    assert_eq!(rows.as_slice(), &[1, 2, 3]);
    assert_eq!(snapshot.as_slice(), &[1, 2], "the clone kept its value");
    assert_eq!(rows.pop(), Some(3));
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1], 2);
    assert_eq!(rows.slice(-1.0, None).as_slice(), &[2]);
    let removed = rows.splice(0.0, 1, vec![9]);
    assert_eq!(removed.as_slice(), &[1]);
    assert_eq!(rows.as_slice(), &[9, 2]);
    let mapped: tsx::List<i32> = rows.iter().map(|value| value * 2).collect();
    assert_eq!(mapped.as_slice(), &[18, 4]);
    assert_eq!(
        tsx::join(&[Str::from("a"), Str::from("b")], "-").as_str(),
        "a-b"
    );
}

// --------------------------------------------------------------- child ids

#[test]
fn child_ids_are_pinned_to_fixed_values() {
    assert_eq!(tsx::child_id(0, "Child", 0u64), 0x4232_64cd_89a6_5067);
    assert_eq!(tsx::child_id(0, "Child", "row"), 0x7240_374a_9028_02dd);
    assert_eq!(tsx::child_id(42, "Row", 7u32), 0x2a3f_103b_05e3_5c30);
    assert_eq!(tsx::child_id(0xDEAD, "Item", 1.5f64), 0x5a06_0303_9823_7823);
    assert_eq!(tsx::site_id(0, "Child", 0), 0x59bf_6044_b947_4071);
    assert_eq!(tsx::site_id(0, "Child", 3), 0x7da4_9531_c58b_7616);
    assert_ne!(tsx::site_id(0, "Child", 0), tsx::child_id(0, "Child", 0u64));
}

// ------------------------------------------------------------------ strings

#[test]
fn js_string_methods_use_utf16_positions() {
    assert_eq!(tsx::utf16_length("📌 "), 3);
    assert_eq!(
        tsx::strings::slice("A中😀Z", 1.0, Some(-1.0)).as_str(),
        "中😀"
    );
    assert_eq!(tsx::strings::slice("😀", 0.0, Some(1.0)).as_str(), "");
    assert_eq!(
        tsx::strings::substring("abcd", 3.0, Some(1.0)).as_str(),
        "bc"
    );
    assert_eq!(tsx::strings::pad_start("x", 5.0, "ab").as_str(), "ababx");
    assert_eq!(tsx::strings::pad_end("中😀", 5.0, "ab").as_str(), "中😀ab");
    assert_eq!(tsx::strings::repeat("a中😀", 2.9).as_str(), "a中😀a中😀");
    assert_eq!(
        tsx::strings::replace("abc", "b", "$$-$&-$`-$'").as_str(),
        "a$-b-a-cc"
    );
    assert_eq!(tsx::strings::replace_all("😀", "", "-").as_str(), "--😀-");
    let parts = tsx::strings::split("A中😀", "");
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[2].as_str(), "😀");
    assert_eq!(tsx::strings::char_at("A中😀", -1.0).as_str(), "");
    assert_eq!(tsx::strings::at("A中😀", -1.0).as_deref(), Some("😀"));
    assert_eq!(tsx::strings::char_code_at("😀", 0.0), 0xd83d as f64);
    assert_eq!(tsx::strings::char_code_at("😀", 1.0), 0xde00 as f64);
    assert_eq!(tsx::strings::code_point_at("😀", 0.0), Some(0x1f600 as f64));
    assert!(tsx::strings::char_code_at("a", 3.0).is_nan());
    assert_eq!(tsx::strings::index_of("中😀abc", "abc"), 3);
}

// ----------------------------------------------------------------- builtins

#[test]
fn js_number_builtins_follow_javascript() {
    use tsx::builtins::{self, Predicate, Unary};
    assert_eq!(builtins::number(" 0x1f "), 31.0);
    assert!(builtins::number("1tail").is_nan());
    assert_eq!(builtins::parse_int("-0x1f", 0.0), -31.0);
    assert_eq!(
        builtins::parse_int("9007199254740993", 10.0),
        9_007_199_254_740_992.0
    );
    assert_eq!(builtins::parse_float(" 1.25e2tail"), 125.0);
    assert_eq!(builtins::string(1e21), Str::from("1e+21"));
    assert_eq!(builtins::string(-0.0), Str::from("0"));
    assert_eq!(builtins::to_fixed(1.005, 2.0), Str::from("1.00"));
    assert_eq!(builtins::to_fixed(1.25, 1.0), Str::from("1.3"));
    assert_eq!(builtins::to_precision(123.456, 4.0), Str::from("123.5"));
    assert_eq!(builtins::to_string_radix(255.0, 16.0), Str::from("ff"));
    assert_eq!(builtins::unary(Unary::Round, -0.5), -0.0);
    assert!(builtins::unary(Unary::Round, -0.5).is_sign_negative());
    assert_eq!(builtins::unary(Unary::Abs, -9i32), 9);
    assert_eq!(builtins::pow(-0.0, -3.0), f64::NEG_INFINITY);
    assert_eq!(
        builtins::extrema(true, &[0.0, -0.0]).to_bits(),
        (-0.0f64).to_bits()
    );
    assert!(builtins::predicate(Predicate::IsNaN, f64::NAN));
    assert!(!builtins::predicate(Predicate::IsNaN, "NaN"));
    assert_eq!(format!("{}", tsx::Any::from(1.0)), "1");
}

// ------------------------------------------------------------------- hooks

#[test]
fn memo_and_effect_helpers_behave() {
    let mut memo: tsx::Memo<Vec<u32>, (u32,)> = tsx::Memo::new();
    assert!(memo.hit((1,)).is_none());
    assert_eq!(memo.store(vec![1, 2]), vec![1, 2]);
    assert_eq!(memo.hit((1,)), Some(vec![1, 2]));
    assert!(memo.hit((2,)).is_none());

    let cleaned = Rc::new(Cell::new(0));
    let counter = Rc::clone(&cleaned);
    ui::begin_frame();
    let mut effect: tsx::Effect<()> = tsx::Effect::new();
    assert!(effect.changed(()));
    effect.set_cleanup(ui::Action::new(move |_| counter.set(counter.get() + 1)));
    assert!(!effect.changed(()));
    effect.queue(ui::Action::new(|_| {}));
    let mut root = ();
    let mut cx = ui::ActionCx::new(&mut root);
    ui::run_effects(&mut cx);
    assert_eq!(cleaned.get(), 1, "the previous cleanup ran first");
    let unmounted = Rc::clone(&cleaned);
    effect.set_cleanup(ui::Action::new(move |_| {
        unmounted.set(unmounted.get() + 1);
    }));
    drop(effect);
    ui::run_effects(&mut cx);
    assert_eq!(cleaned.get(), 2, "unmounting queues the cleanup");
    ui::end_frame();
}

// ------------------------------------------------------------------ timers

static FAKE_NOW: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

fn fake_clock() -> i64 {
    FAKE_NOW.load(std::sync::atomic::Ordering::SeqCst)
}

fn set_fake_now(value: i64) {
    FAKE_NOW.store(value, std::sync::atomic::Ordering::SeqCst);
}

#[test]
fn timers_run_on_the_frame_clock() {
    tsx::timers::set_clock(Some(fake_clock));
    tsx::timers::set_running(false);
    set_fake_now(0);
    assert_eq!(tsx::timers::pending(), 0);
    let hits = Rc::new(RefCell::new(Vec::<u8>::new()));
    let first = Rc::clone(&hits);
    let second = Rc::clone(&hits);
    let a = tsx::timers::set_timeout(0.0, ui::Action::new(move |_| first.borrow_mut().push(1)));
    let b = tsx::timers::set_timeout(50.0, ui::Action::new(move |_| second.borrow_mut().push(2)));
    assert!(a > 0.0 && b > a);
    let mut root = ();
    let mut cx = ui::ActionCx::new(&mut root);
    assert!(
        tsx::timers::fire(0, &mut cx),
        "the 0 ms timer is due at once"
    );
    assert_eq!(*hits.borrow(), vec![1]);
    assert_eq!(tsx::timers::pending(), 1);
    set_fake_now(1);
    assert!(!tsx::timers::fire(1, &mut cx), "the 50 ms timer is not due");
    assert!(tsx::timers::fire(50, &mut cx));
    assert_eq!(*hits.borrow(), vec![1, 2]);
    tsx::timers::clear_timeout(b); // already ran: ignored
    assert_eq!(tsx::timers::pending(), 0);
    set_fake_now(0);
    tsx::timers::set_clock(None);
}

// --------------------------------------------------------------- root/shell

#[derive(Default)]
struct AppState {
    renders: Rc<Cell<u32>>,
    ticks: Rc<Cell<u32>>,
    fired: Rc<Cell<u32>>,
}

impl ui::App for AppState {
    fn render(&mut self) -> ui::Node {
        self.renders.set(self.renders.get() + 1);
        ui::column(ui::BoxOptions {
            children: zant::nodes![ui::text("hello", ui::TextOptions::default())],
            ..Default::default()
        })
    }

    fn on_wake(&mut self) {
        self.ticks.set(self.ticks.get() + 1);
    }

    fn on_key(&mut self, event: &zant::core::events::KeyEvent) -> bool {
        tsx::app::key_event(event).key.as_str() == "Escape"
    }
}

#[test]
fn the_shell_renders_and_fires_timers_on_wake() {
    tsx::timers::set_clock(Some(fake_clock));
    tsx::timers::set_running(false);
    set_fake_now(0);
    let state = AppState::default();
    let fired = Rc::clone(&state.fired);
    let timer_fired = Rc::clone(&state.fired);
    let renders = Rc::clone(&state.renders);
    let ticks = Rc::clone(&state.ticks);
    // the timer captures nothing from the shell: it writes shared state, like a
    // generated timer that sets a field of the root component
    let _timer = tsx::timers::set_timeout(
        10.0,
        ui::Action::new(move |_| {
            assert!(timer_fired.get() < 1);
            timer_fired.set(timer_fired.get() + 1);
        }),
    );
    // a root-state timer, the shape generated code emits: `ui::action`
    let _root_timer = tsx::timers::set_timeout(
        20.0,
        ui::action(|s: &mut AppState| s.ticks.set(s.ticks.get() + 10)),
    );
    let shell = tsx::Shell::new(state);
    let mut runtime = zant::runtime::Runtime::init(shell, ui::RunOptions::default());
    runtime.rebuild_for_test();
    assert_eq!(renders.get(), 1);
    set_fake_now(10);
    runtime.handle_wake();
    assert_eq!(fired.get(), 1, "the due timer ran on wake");
    assert_eq!(ticks.get(), 1, "then the root's useWake hook");
    assert!(runtime.needs_build, "the wake schedules a rebuild");
    runtime.rebuild_for_test();
    assert_eq!(renders.get(), 2, "the next frame rebuilt the app");
    tsx::timers::set_clock(None);
    tsx::timers::set_running(false);
}

// ----------------------------------------------------------------- desktop

#[test]
fn tray_and_hotkeys_dispatch_with_the_system_disabled() {
    tsx::desktop::set_system_enabled(false);
    let hits = Rc::new(Cell::new(0));
    let counter = Rc::clone(&hits);
    let mut hotkey = tsx::desktop::Hotkey::default();
    let id = {
        assert!(hotkey.update(
            "ctrl+alt+k",
            ui::Action::new(move |_| counter.set(counter.get() + 1))
        ));
        hotkey.id()
    };
    let mut root = ();
    let mut cx = ui::ActionCx::new(&mut root);
    assert!(tsx::desktop::handle_app_event(
        &mut cx,
        &zant::desktop::AppEvent::Hotkey(id)
    ));
    assert_eq!(hits.get(), 1);
    // a tray menu item always routes (an unknown id does nothing)
    assert!(tsx::desktop::handle_app_event(
        &mut cx,
        &zant::desktop::AppEvent::TrayMenu(9)
    ));
    assert!(!tsx::desktop::handle_app_event(
        &mut cx,
        &zant::desktop::AppEvent::Clipboard
    ));
    let shortcut = tsx::desktop::parse_shortcut("ctrl+alt+k").expect("valid");
    assert_eq!(
        tsx::desktop::desktop_shortcut(shortcut).format(),
        "ctrl+alt+k"
    );
    assert!(tsx::desktop::parse_shortcut("").is_none());
    // the tray is applied while rendering (no system tray in tests)
    assert!(tsx::desktop::update(tsx::desktop::TrayOptions::default()));
    tsx::desktop::set_system_enabled(!cfg!(test));
}

// -------------------------------------------------------------- window/text

struct Panel {
    local: Rc<RefCell<u32>>,
}

impl tsx::WindowComponent for Panel {
    type Local = u32;

    fn with_local(local: Rc<RefCell<u32>>) -> Panel {
        Panel { local }
    }

    fn render(&mut self) -> ui::Node {
        *self.local.borrow_mut() += 1;
        ui::text("panel", ui::TextOptions::default())
    }
}

#[test]
fn window_components_keep_a_local_state_and_open_lazily() {
    use tsx::WindowComponent as _;
    let mut window = tsx::Window::<Panel>::new(zant::desktop::OpenOptions {
        title: String::from("Panel"),
        ..Default::default()
    });
    assert!(!window.visible());
    window.toggle(); // cannot open without a running app: no panic
    assert!(!window.visible());
    let mut panel = <Panel as tsx::WindowComponent>::with_local(Rc::clone(window.local()));
    let _ = panel.render();
    assert_eq!(*window.local().borrow(), 1);
    assert_eq!(window.theme(), tsx::app::current_theme());
}

#[test]
fn rich_text_spans_and_links_are_built_for_the_codegen() {
    ui::begin_frame();
    let plain = tsx::text::span("a", tsx::text::SpanStyle::default());
    assert_eq!(plain.text.as_str(), "a");
    assert_eq!(plain.font_size, 14.0);
    assert_eq!(plain.link.as_str(), "");
    let bold = tsx::text::span(
        "b",
        tsx::text::SpanStyle {
            weight: Some(zant::renderer::text::Weight::Bold),
            link: Some(2),
            ..Default::default()
        },
    );
    assert_eq!(bold.link.as_str(), "2");
    assert_eq!(
        bold.color,
        ui::current_theme().color_primary,
        "links use the primary colour"
    );
    let hits = Rc::new(Cell::new(0));
    let counter = Rc::clone(&hits);
    let on_link = tsx::text::links(vec![
        None,
        Some(ui::Action::new(move |_| counter.set(counter.get() + 1))),
    ]);
    let mut root = ();
    let mut cx = ui::ActionCx::new(&mut root);
    on_link.call(&mut cx, "1");
    assert_eq!(hits.get(), 1);
    ui::end_frame();
}

// ------------------------------------------------------------------ native

#[test]
fn native_modules_register_and_resolve_by_name() {
    struct Demo;
    impl tsx::native::NativeModule for Demo {
        const NAME: &'static str = "demo";
    }
    tsx::native::clear();
    assert!(tsx::native::register(Demo));
    assert!(tsx::native::registered("demo"));
    assert!(tsx::native::get::<Demo>("demo").is_some());
    assert!(tsx::native::get::<String>("demo").is_none());
    tsx::native::clear();
    assert!(!tsx::native::registered("demo"));
}

// ----------------------------------------------------------------- helpers

#[test]
fn number_and_index_helpers_follow_js_semantics() {
    assert_eq!(tsx::to_int::<usize>(2.9), 2);
    assert_eq!(tsx::to_int::<u8>(1e9), 255);
    assert_eq!(tsx::list_index(-1.0), None);
    assert_eq!(tsx::list_index(4.9), Some(4));
    assert_eq!(tsx::list_key(1.5), 1.5f64.to_bits());
    assert_eq!(tsx::index(2.0), 2);
    assert_eq!(tsx::slice_index(-1.0, 4), 3);
    assert_eq!(tsx::slice_index(f64::NAN, 4), 0);
    assert_eq!(tsx::null_as::<Str>(None), None);
    assert!(!tsx::null_equal(true, None::<u8>, Some("x")));
    let nodes = tsx::nodes([
        Some(ui::text("a", ui::TextOptions::default())),
        None,
        Some(ui::text("b", ui::TextOptions::default())),
    ]);
    assert_eq!(nodes.len(), 2);
}
