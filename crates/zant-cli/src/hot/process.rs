//! Keep compiler processes below the interactive app's scheduling priority.
use std::process::Command;

pub(super) fn cargo() -> Command {
    #[cfg(unix)]
    {
        let mut command = Command::new("nice");
        command.args(["-n", "10", crate::cargo_exe()]);
        command
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut command = Command::new(crate::cargo_exe());
        command.creation_flags(0x0000_4000); // BELOW_NORMAL_PRIORITY_CLASS
        command
    }
}
