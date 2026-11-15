//! `zant-hot-host <ui library>`: the process `zant run` keeps alive.
//!
//! It owns the native window and the shared native layer. The app's UI (the
//! generated TSX code and its `native/` modules) is a dynamic library loaded
//! at start and replaced on every rebuild; the Rust framework is shared. The host
//! saves the state of the running version, stops it, loads the new library
//! and starts it on the same window with that state. The window never closes.
//!
//! Commands arrive on stdin, one per line:
//!
//! - `load <path>`: switch to the UI library at `path`;
//! - `notice <text>`: show a message over the UI (a build error; `\n` and
//!   `\\` escaped);
//! - `clear`: hide the message.

mod checkpoint;
mod control;
mod debug;
mod library;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::c_void;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use zant_sys::hot::ZantHotConfig;
use zant_sys::platform::{self as ffi, ZantCanvas, ZantEvent, ZantHost, ZantWindow};

use library::Library;

/// Commands read from stdin, drained on the UI thread.
static COMMANDS: Mutex<VecDeque<Command>> = Mutex::new(VecDeque::new());
/// The window, for the stdin thread's wake-ups (thread-safe per platform.h).
static WINDOW: std::sync::atomic::AtomicPtr<ZantWindow> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

enum Command {
    Load(Result<Library, String>, std::time::Duration),
    Notice(String),
    Clear,
    Checkpoint(PathBuf),
    Quit,
    Control(control::Action),
}

/// The running version of the UI.
struct Current {
    library: Library,
    instance: *mut c_void,
    /// The callbacks of the running instance; the host's own table forwards
    /// every native callback here.
    table: ZantHost,
}

thread_local! {
    static CURRENT: RefCell<Option<Current>> = const { RefCell::new(None) };
}

fn main() {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: zant-hot-host <ui library> [--deps <Rust dylib directory>]");
        std::process::exit(2);
    };
    let mut args = std::env::args_os().skip(2);
    while let Some(arg) = args.next() {
        if arg == "--deps" {
            let result = args
                .next()
                .ok_or_else(|| "missing --deps directory".to_string())
                .and_then(|path| library::dependencies(Path::new(&path)));
            if let Err(message) = result {
                eprintln!("[zant] {message}");
                std::process::exit(1);
            }
        }
    }
    let library = match Library::load(&path) {
        Ok(library) => library,
        Err(message) => {
            eprintln!("[zant] {message}");
            std::process::exit(1);
        }
    };
    zant_sys::init();

    // the window the app asks for
    let mut config = ZantHotConfig {
        title: [0; 256],
        // SAFETY: plain C struct; `config` overwrites every field.
        options: unsafe { std::mem::zeroed() },
    };
    // SAFETY: valid out pointer.
    unsafe { (library.api.config)(&mut config) };
    let table = host_table();
    // SAFETY: NUL-terminated title (the library writes one), valid options and
    // table; the native side copies the table.
    let window =
        unsafe { ffi::zant_window_create(config.title.as_ptr().cast(), &config.options, &table) };
    if window.is_null() {
        eprintln!("[zant] window creation failed");
        std::process::exit(1);
    }
    WINDOW.store(window, std::sync::atomic::Ordering::Release);

    let state = checkpoint::resume();
    match start(library, window, state.as_deref()) {
        Ok(current) => CURRENT.with(|slot| *slot.borrow_mut() = Some(current)),
        Err(message) => {
            eprintln!("[zant] {message}");
            std::process::exit(1);
        }
    }
    debug::loaded(window);
    std::thread::spawn(read_commands);
    control::start();

    // SAFETY: live window; returns when the app's last window closed.
    unsafe { ffi::zant_window_run(window) };

    if let Some(current) = CURRENT.with(|slot| slot.borrow_mut().take()) {
        // SAFETY: live instance of its own library; the app ends.
        unsafe { (current.library.api.destroy)(current.instance, 1) };
    }
    WINDOW.store(std::ptr::null_mut(), std::sync::atomic::Ordering::Release);
    // SAFETY: the window is ours; no instance uses it any more.
    unsafe { ffi::zant_window_destroy(window) };
}

/// Starts the library's app on `window`.
fn start(
    library: Library,
    window: *mut ZantWindow,
    state: Option<&[u8]>,
) -> Result<Current, String> {
    // SAFETY: the zeroed table is filled in by `create`.
    let mut table: ZantHost = unsafe { std::mem::zeroed() };
    let (data, len) = state.map_or((std::ptr::null(), 0), |state| (state.as_ptr(), state.len()));
    // SAFETY: live window, valid state buffer and out pointer.
    let instance = unsafe { (library.api.create)(window, data, len, &mut table) };
    if instance.is_null() {
        return Err("the app failed to start (it panicked; see the output above)".into());
    }
    Ok(Current {
        library,
        instance,
        table,
    })
}

/// Reads stdin commands and wakes the UI thread for each.
fn read_commands() {
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let command = if let Some(path) = line.strip_prefix("load ") {
            let started = Instant::now();
            let library = Library::load(Path::new(path));
            let elapsed = started.elapsed();
            #[cfg(target_os = "macos")]
            if elapsed > std::time::Duration::from_millis(250)
                && std::env::var_os("ZANT_HOT_QUIET").is_none_or(|value| value != "1")
            {
                static HINT: std::sync::Once = std::sync::Once::new();
                HINT.call_once(|| {
                    eprintln!(
                        "[zant] 提示：macOS 每次加载新编译的库要做一次安全扫描（本次 {} ms）。在「系统设置 › 隐私与安全性 › 开发者工具」中加入你的终端，可省去这段时间。",
                        elapsed.as_millis()
                    );
                });
            }
            Command::Load(library, elapsed)
        } else if let Some(text) = line.strip_prefix("notice ") {
            Command::Notice(unescape(text))
        } else if let Some(path) = line.strip_prefix("checkpoint ") {
            Command::Checkpoint(path.into())
        } else if line == "clear" {
            Command::Clear
        } else {
            continue;
        };
        enqueue(command);
    }
    enqueue(Command::Quit);
}

fn enqueue(command: Command) {
    COMMANDS.lock().expect("commands").push_back(command);
    let window = WINDOW.load(std::sync::atomic::Ordering::Acquire);
    if !window.is_null() {
        // SAFETY: thread-safe per platform.h.
        unsafe { ffi::zant_window_post_wake(window) };
    }
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// Runs the queued commands (UI thread, from the wake callback).
fn run_commands() {
    loop {
        let Some(command) = COMMANDS.lock().expect("commands").pop_front() else {
            return;
        };
        match command {
            Command::Load(library, elapsed) => reload(library, elapsed),
            Command::Control(action) => control::dispatch(action),
            Command::Notice(text) => notice(&text),
            Command::Clear => notice(""),
            Command::Checkpoint(path) => {
                let state = CURRENT.with(|slot| slot.borrow().as_ref().and_then(save));
                checkpoint::write(&path, state.as_deref());
            }
            // SAFETY: command dispatch runs on the UI thread.
            Command::Quit => unsafe { ffi::zant_app_quit() },
        }
    }
}

fn notice(text: &str) {
    CURRENT.with(|slot| {
        if let Some(current) = slot.borrow().as_ref() {
            // SAFETY: live instance; the library copies the text.
            unsafe { (current.library.api.notice)(current.instance, text.as_ptr(), text.len()) };
        }
    });
}

/// Replaces the running UI with the library at `path`, carrying the state.
fn reload(loaded: Result<Library, String>, t_load: std::time::Duration) {
    let begin = Instant::now();
    let library = match loaded {
        Ok(library) => library,
        Err(message) => {
            eprintln!("[zant] {message}");
            notice(&message);
            return;
        }
    };
    let Some(old) = CURRENT.with(|slot| slot.borrow_mut().take()) else {
        return;
    };
    let window = WINDOW.load(std::sync::atomic::Ordering::Acquire);
    let state = save(&old);
    debug::state(state.as_deref());
    let t_save = begin.elapsed();
    // the old version goes first: both share the native app-event handler,
    // tray and hotkeys, which the new version installs again
    // SAFETY: live instance; it is a reload, not an exit.
    unsafe { (old.library.api.destroy)(old.instance, 0) };
    let t_destroy = begin.elapsed();
    let current = match start(library, window, state.as_deref()) {
        Ok(current) => {
            eprintln!("[zant] reloaded in {} ms", begin.elapsed().as_millis());
            if std::env::var_os("ZANT_HOT_TRACE").is_some() {
                eprintln!(
                    "[zant-hot] load_ms={:.3} save_ms={:.3} destroy_ms={:.3} create_ms={:.3}",
                    t_load.as_secs_f64() * 1000.0,
                    t_save.as_secs_f64() * 1000.0,
                    (t_destroy - t_save).as_secs_f64() * 1000.0,
                    (begin.elapsed() - t_destroy).as_secs_f64() * 1000.0
                );
            }
            current
        }
        Err(message) => {
            eprintln!("[zant] {message}; keeping the previous version");
            match start(old.library, window, state.as_deref()) {
                Ok(current) => {
                    let current_ref = &current;
                    // SAFETY: live instance.
                    unsafe {
                        (current_ref.library.api.notice)(
                            current_ref.instance,
                            message.as_ptr(),
                            message.len(),
                        )
                    };
                    current
                }
                Err(again) => {
                    eprintln!("[zant] {again}");
                    std::process::exit(1);
                }
            }
        }
    };
    CURRENT.with(|slot| *slot.borrow_mut() = Some(current));
    debug::loaded(window);
    // SAFETY: live window.
    unsafe { ffi::zant_window_request_redraw(window, 0) };
}

/// The state snapshot of `current`.
fn save(current: &Current) -> Option<Vec<u8>> {
    unsafe extern "C" fn sink(ctx: *mut c_void, data: *const u8, len: usize) {
        // SAFETY: `ctx` is the `Vec` below; `data` holds `len` bytes.
        let out = unsafe { &mut *(ctx as *mut Vec<u8>) };
        out.extend_from_slice(unsafe { std::slice::from_raw_parts(data, len) });
    }
    let mut out: Vec<u8> = Vec::new();
    // SAFETY: live instance; `sink` copies during the call.
    unsafe {
        (current.library.api.save)(current.instance, sink, (&mut out as *mut Vec<u8>).cast())
    };
    (!out.is_empty()).then_some(out)
}

// ---- the host's callback table: forwards to the running instance ---------

fn host_table() -> ZantHost {
    ZantHost {
        userdata: std::ptr::null_mut(),
        draw: Some(on_draw),
        event: Some(on_event),
        ime_rect: Some(on_ime_rect),
        next_frame_delay: Some(on_next_frame_delay),
        titlebar_hit: Some(on_titlebar_hit),
        wake: Some(on_wake),
    }
}

/// The running instance's table (copied out, so no borrow is held while the
/// instance runs; it may reenter the host through a nested native loop).
fn table() -> Option<ZantHost> {
    CURRENT.with(|slot| {
        slot.borrow().as_ref().map(|current| ZantHost {
            userdata: current.table.userdata,
            draw: current.table.draw,
            event: current.table.event,
            ime_rect: current.table.ime_rect,
            next_frame_delay: current.table.next_frame_delay,
            titlebar_hit: current.table.titlebar_hit,
            wake: current.table.wake,
        })
    })
}

unsafe extern "C" fn on_draw(
    _userdata: *mut c_void,
    canvas: *mut ZantCanvas,
    width: f32,
    height: f32,
    dpr: f32,
) {
    if let Some(table) = table()
        && let Some(draw) = table.draw
    {
        // SAFETY: forwarding the native call to the instance's own callback.
        unsafe { draw(table.userdata, canvas, width, height, dpr) };
        debug::frame();
    }
}

unsafe extern "C" fn on_event(_userdata: *mut c_void, event: *const ZantEvent) -> i32 {
    match table() {
        // SAFETY: as above.
        Some(ZantHost {
            userdata,
            event: Some(callback),
            ..
        }) => unsafe { callback(userdata, event) },
        _ => 0,
    }
}

unsafe extern "C" fn on_ime_rect(
    _userdata: *mut c_void,
    x: *mut f32,
    y: *mut f32,
    width: *mut f32,
    height: *mut f32,
) -> i32 {
    match table() {
        // SAFETY: as above.
        Some(ZantHost {
            userdata,
            ime_rect: Some(callback),
            ..
        }) => unsafe { callback(userdata, x, y, width, height) },
        _ => 0,
    }
}

unsafe extern "C" fn on_next_frame_delay(_userdata: *mut c_void) -> i32 {
    match table() {
        // SAFETY: as above.
        Some(ZantHost {
            userdata,
            next_frame_delay: Some(callback),
            ..
        }) => unsafe { callback(userdata) },
        _ => -1,
    }
}

unsafe extern "C" fn on_titlebar_hit(_userdata: *mut c_void, x: f32, y: f32) -> i32 {
    match table() {
        // SAFETY: as above.
        Some(ZantHost {
            userdata,
            titlebar_hit: Some(callback),
            ..
        }) => unsafe { callback(userdata, x, y) },
        _ => ffi::ZANT_HIT_CAPTION as i32,
    }
}

unsafe extern "C" fn on_wake(_userdata: *mut c_void) {
    run_commands();
    if let Some(table) = table()
        && let Some(wake) = table.wake
    {
        // SAFETY: as above.
        unsafe { wake(table.userdata) };
    }
}
