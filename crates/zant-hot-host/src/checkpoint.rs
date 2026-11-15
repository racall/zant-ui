//! Single-use state handoff when the CLI replaces itself.
use std::path::{Path, PathBuf};

pub fn resume() -> Option<Vec<u8>> {
    let mut args = std::env::args_os().skip(2);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--state")) {
        return None;
    }
    consume(&PathBuf::from(args.next()?))
}

fn consume(path: &Path) -> Option<Vec<u8>> {
    let result = std::fs::read(path);
    // Consume even invalid data, so a subsequent startup cannot restore stale state.
    if let Err(error) = std::fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        eprintln!("[zant] cannot remove restart state: {error}");
    }
    match result {
        Ok(state) => {
            eprintln!("[zant] restored restart state: {}", path.display());
            Some(state)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            eprintln!("[zant] cannot read restart state: {error}");
            None
        }
    }
}

pub fn write(path: &Path, state: Option<&[u8]>) {
    let Some(state) = state else { return };
    let temporary = path.with_extension("json.tmp");
    let result = std::fs::write(&temporary, state).and_then(|()| std::fs::rename(&temporary, path));
    match result {
        Ok(()) => eprintln!("[zant] saved restart state: {}", path.display()),
        Err(error) => {
            let _ = std::fs::remove_file(temporary);
            eprintln!("[zant] cannot save restart state: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn state_is_consumed_once() {
        let path = std::env::temp_dir().join(format!("zant-restart-{}.json", std::process::id()));
        write(&path, Some(br#"{"app":{"count":3},"view":{}}"#));
        assert!(consume(&path).is_some());
        assert!(!path.exists());
        assert!(consume(&path).is_none());
    }
}
