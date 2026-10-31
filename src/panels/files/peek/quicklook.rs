use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const PIXELS: u32 = 900;
const WITHIN: Duration = Duration::from_secs(8);
const POLL: Duration = Duration::from_millis(20);

fn scratch() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("cornercase-gpui-peek-{}-{n}", std::process::id()))
}

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

fn finished_within(command: &mut Command, limit: Duration) -> bool {
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
    let out = scratch();
    std::fs::create_dir_all(&out).ok()?;
    let finished = finished_within(Command::new("qlmanage").args(args(path, &out)), WITHIN);
    let png = made(path, &out).filter(|_| finished).and_then(|png| std::fs::read(png).ok());
    let _ = std::fs::remove_dir_all(&out);
    png
}

#[cfg(test)]
mod tests;
