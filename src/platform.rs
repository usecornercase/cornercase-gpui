use std::ffi::OsStr;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Os {
    Mac,
    Linux,
    Windows,
}

impl Os {
    pub(crate) const CURRENT: Os = if cfg!(target_os = "macos") {
        Os::Mac
    } else if cfg!(windows) {
        Os::Windows
    } else {
        Os::Linux
    };
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(not(windows))]
#[expect(clippy::disallowed_methods, reason = "the one place that makes a Command")]
pub(crate) fn command(program: impl AsRef<OsStr>) -> Command {
    Command::new(program)
}

#[cfg(windows)]
#[expect(clippy::disallowed_methods, reason = "the one place that makes a Command")]
pub(crate) fn command(program: impl AsRef<OsStr>) -> Command {
    use std::os::windows::process::CommandExt as _;
    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}
