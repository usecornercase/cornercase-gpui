use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::platform::Os;

const APP: &str = "cornercase";
const ICON: &str = "utilities-terminal";
const FREEDESKTOP: &str = "sounds/freedesktop/stereo";

pub(super) const NO_NOTIFY_SEND: &str =
    "No desktop notifications: install notify-send (libnotify-bin on Debian and Ubuntu, libnotify elsewhere)";
pub(super) const NO_PLAYER: &str = "No sound: install paplay (pulseaudio-utils) or pw-play (pipewire-bin)";
pub(super) const NO_THEME: &str =
    "No sound: install the freedesktop sound theme (sound-theme-freedesktop), or choose sound files in Settings";

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
    }
}

pub(super) fn cue_sound(os: Os, cue: &str, own: Option<PathBuf>, data: &[PathBuf]) -> Option<PathBuf> {
    own.or_else(|| system_sound(os, cue, data))
}

pub(super) fn sound(
    os: Os,
    file: &Path,
    find: &dyn Fn(&str) -> Option<PathBuf>,
) -> Result<Vec<OsString>, &'static str> {
    let file = file.as_os_str().to_owned();
    if os == Os::Mac {
        return Ok(vec!["afplay".into(), file]);
    }
    if let Some(player) = find("paplay").or_else(|| find("pw-play")) {
        return Ok(vec![player.into_os_string(), file]);
    }
    let wav = Path::new(&file).extension().is_some_and(|e| e.eq_ignore_ascii_case("wav"));
    match find("aplay") {
        Some(aplay) if wav => Ok(vec![aplay.into_os_string(), "-q".into(), file]),
        _ => Err(NO_PLAYER),
    }
}

#[cfg(test)]
mod tests;
