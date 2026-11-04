use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::desktop;
use crate::platform::{self, Os};

const APP: &str = "cornercase";
const ICON: &str = "utilities-terminal";
const FREEDESKTOP: &str = "sounds/freedesktop/stereo";

pub(super) const NO_NOTIFY_SEND: &str =
    "No desktop notifications: install notify-send (libnotify-bin on Debian and Ubuntu, libnotify elsewhere)";
pub(super) const NO_PLAYER: &str = "No sound: install paplay (pulseaudio-utils) or pw-play (pipewire-bin)";
pub(super) const NO_THEME: &str =
    "No sound: install the freedesktop sound theme (sound-theme-freedesktop), or choose sound files in Settings";
pub(super) const SOUND_FAILED: &str = "No sound";
pub(super) const NOTIFICATION_FAILED: &str = "No desktop notification";

const PLAYERS: [(&str, &[&str]); 3] = [("paplay", &[]), ("pw-play", &[]), ("aplay", &["-q"])];

pub(super) fn notification(
    os: Os,
    (title, body): (&str, &str),
    find: &dyn Fn(&str) -> Option<PathBuf>,
) -> Result<Vec<OsString>, &'static str> {
    if os == Os::Mac {
        let script = ["on run argv", "display notification (item 2 of argv) with title (item 1 of argv)", "end run"];
        let mut argv = vec![OsString::from("osascript")];
        argv.extend(script.iter().flat_map(|line| [OsString::from("-e"), OsString::from(line)]));
        argv.extend([OsString::from(title), OsString::from(body)]);
        return Ok(argv);
    }
    let send = find("notify-send").ok_or(NO_NOTIFY_SEND)?;
    let flags = [format!("--app-name={APP}"), format!("--icon={ICON}"), "--".to_string(), title.into(), body.into()];
    Ok(std::iter::once(send.into_os_string()).chain(flags.map(OsString::from)).collect())
}

pub(super) fn system_sound(os: Os, cue: &str, data: &[PathBuf]) -> Option<PathBuf> {
    let finished = cue == "finished";
    match os {
        Os::Mac => {
            let name = if finished { "Glass" } else { "Ping" };
            Some(PathBuf::from(format!("/System/Library/Sounds/{name}.aiff")))
        }
        Os::Linux => {
            let name = if finished { "complete.oga" } else { "message-new-instant.oga" };
            data.iter().map(|dir| dir.join(FREEDESKTOP).join(name)).find(|path| path.is_file())
        }
        Os::Windows => None,
    }
}

pub(super) fn cue_sound(os: Os, cue: &str, own: Option<PathBuf>, data: &[PathBuf]) -> Option<PathBuf> {
    own.or_else(|| system_sound(os, cue, data))
}

pub(super) fn sound(
    os: Os,
    file: &Path,
    find: &dyn Fn(&str) -> Option<PathBuf>,
) -> Result<Vec<Vec<OsString>>, &'static str> {
    let file = file.as_os_str();
    if os == Os::Mac {
        return Ok(vec![vec!["afplay".into(), file.to_owned()]]);
    }
    let wav = Path::new(file).extension().is_some_and(|e| e.eq_ignore_ascii_case("wav"));
    let tries: Vec<Vec<OsString>> = PLAYERS
        .iter()
        .filter(|(name, _)| wav || *name != "aplay")
        .filter_map(|(name, flags)| {
            let player = find(name)?.into_os_string();
            Some([player].into_iter().chain(flags.iter().map(OsString::from)).chain([file.to_owned()]).collect())
        })
        .collect();
    if tries.is_empty() { Err(NO_PLAYER) } else { Ok(tries) }
}

pub(super) fn first_that_works(
    tries: &[Vec<OsString>],
    run: impl Fn(&[OsString]) -> Result<(), String>,
) -> Result<(), String> {
    let mut failures = Vec::new();
    for argv in tries {
        match run(argv) {
            Ok(()) => return Ok(()),
            Err(why) => failures.push(why),
        }
    }
    Err(failures.join("; "))
}

pub(super) fn run(argv: &[OsString]) -> Result<(), String> {
    let Some((program, rest)) = argv.split_first() else { return Ok(()) };
    let name = Path::new(program).file_name().unwrap_or(program).to_string_lossy().into_owned();
    let out = platform::command(program)
        .args(rest)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("could not start {name}: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    Err(desktop::failure(&name, &out.status.to_string(), &String::from_utf8_lossy(&out.stderr)))
}

#[cfg(test)]
mod tests;
