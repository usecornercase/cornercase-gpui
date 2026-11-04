use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::desktop::Scratch;

const PIXELS: u32 = 900;
const WITHIN: Duration = Duration::from_secs(8);
const POLL: Duration = Duration::from_millis(20);
pub(super) const SCRATCH: &str = "peek";

pub(super) fn args(path: &Path, out: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["-t", "-s", &PIXELS.to_string(), "-o"].iter().map(OsString::from).collect();
    args.extend([out.as_os_str().to_owned(), path.as_os_str().to_owned()]);
    args
}

pub(super) fn made(path: &Path, out: &Path) -> Option<PathBuf> {
    let name = path.file_name()?;
    let mut png = name.to_owned();
    png.push(".png");
    Some(out.join(png))
}

pub(super) fn finished_within(command: &mut Command, limit: Duration) -> bool {
    let spawned = command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    let Ok(mut child) = spawned else { return false };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if start.elapsed() < limit => std::thread::sleep(POLL),
            _ => break,
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    false
}

pub(super) fn thumbnail(path: &Path) -> Option<Vec<u8>> {
    let out = Scratch::folder(SCRATCH).ok()?;
    let finished = finished_within(Command::new("qlmanage").args(args(path, out.path())), WITHIN);
    made(path, out.path()).filter(|_| finished).and_then(|png| std::fs::read(png).ok())
}

#[cfg(test)]
mod tests;
