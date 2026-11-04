use std::ffi::OsString;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::super::disk;
use crate::desktop::Scratch;
use crate::platform;

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

pub(super) fn finished_within(command: &mut Command, limit: Duration) -> Result<(), String> {
    let spawned = command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    let mut child = spawned.map_err(|e| format!("could not start: {e}"))?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => return Err(format!("ended with {status}")),
            Ok(None) if start.elapsed() < limit => std::thread::sleep(POLL),
            _ => break,
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    Err(format!("did not finish within {} s", limit.as_secs()))
}

pub(super) fn picture(png: &Path, cap: u64) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(png).map_err(|_| "made no picture")?;
    let mut bytes = Vec::new();
    file.take(cap + 1).read_to_end(&mut bytes).map_err(|e| format!("made a picture that cannot be read: {e}"))?;
    if bytes.len() as u64 > cap {
        return Err(format!("made a picture over {}", disk::human_size(cap)));
    }
    Ok(bytes)
}

pub(super) fn thumbnail(path: &Path) -> Option<Vec<u8>> {
    let out = Scratch::folder(SCRATCH).ok()?;
    let finished = finished_within(platform::command("qlmanage").args(args(path, out.path())), WITHIN).is_ok();
    made(path, out.path()).filter(|_| finished).and_then(|png| picture(&png, disk::MAX_IMAGE_BYTES).ok())
}

#[cfg(test)]
mod tests;
