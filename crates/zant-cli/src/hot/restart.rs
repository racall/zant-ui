//! Replace the CLI itself: no cached project/configuration state survives.
use std::path::Path;
use std::process::Command;

pub(super) fn execute(state: &Path) -> std::io::Result<()> {
    let mut command = Command::new(std::env::current_exe()?);
    command.args(std::env::args_os().skip(1));
    // The handoff is consumed once by the first host of the new session.
    command.env("ZANT_HOT_RESTART_STATE", state);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(command.exec())
    }
    #[cfg(windows)]
    {
        // Windows has no exec: run the new session in the same console with
        // inherited stdio and wait for it, so the shell keeps waiting too
        // (it would otherwise print its prompt while the app still runs).
        let status = command.status()?;
        std::process::exit(status.code().unwrap_or(1));
    }
}
