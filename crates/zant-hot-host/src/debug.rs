//! Opt-in reload evidence. No timers or file I/O when debugging is disabled.
use std::{cell::RefCell, ffi::CString, path::PathBuf, time::Instant};
use zant_sys::platform::{self as ffi, ZantWindow};

#[derive(Default)]
struct Debug {
    generation: u32,
    frame: Option<Instant>,
}
thread_local! { static DEBUG: RefCell<Debug> = RefCell::new(Debug::default()); }

fn directory() -> Option<PathBuf> {
    std::env::var_os("ZANT_HOT_SNAPSHOT").map(PathBuf::from)
}

pub fn loaded(window: *mut ZantWindow) {
    DEBUG.with(|slot| {
        let mut debug = slot.borrow_mut();
        if std::env::var_os("ZANT_HOT_TRACE").is_some() {
            debug.frame = Some(Instant::now());
        }
        if let Some(dir) = directory() {
            let result = std::fs::create_dir_all(&dir).and_then(|()| {
                let path = dir.join(format!(
                    "reload-{}-{:04}.png",
                    std::process::id(),
                    debug.generation
                ));
                let path = CString::new(path.to_string_lossy().as_bytes())?;
                // SAFETY: live host window; native API copies the path.
                unsafe { ffi::zant_window_schedule_snapshot(window, path.as_ptr(), 0) };
                Ok(())
            });
            if let Err(error) = result {
                eprintln!("[zant] snapshot: {error}");
            }
        }
        debug.generation += 1;
    });
}

pub fn state(state: Option<&[u8]>) {
    if let (Some(dir), Some(state)) = (directory(), state) {
        DEBUG.with(|slot| {
            let path = dir.join(format!(
                "state-{}-{:04}.json",
                std::process::id(),
                slot.borrow().generation
            ));
            if let Err(error) = std::fs::write(path, state) {
                eprintln!("[zant] state snapshot: {error}");
            }
        });
    }
}

pub fn frame() {
    DEBUG.with(|slot| {
        if let Some(started) = slot.borrow_mut().frame.take() {
            eprintln!(
                "[zant-hot] first_frame_ms={:.3}",
                started.elapsed().as_secs_f64() * 1000.0
            );
        }
    });
}
