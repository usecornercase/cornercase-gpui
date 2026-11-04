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

fn lossy(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub(super) fn notification(
    os: Os,
    (title, body): (&str, &str),
    find: &dyn Fn(&str) -> Option<PathBuf>,
) -> Result<Vec<String>, &'static str> {
    if os == Os::Mac {
        let script = ["on run argv", "display notification (item 2 of argv) with title (item 1 of argv)", "end run"];
        let mut argv = vec!["osascript".to_string()];
        argv.extend(script.iter().flat_map(|line| ["-e".to_string(), (*line).to_string()]));
        argv.extend([title.to_string(), body.to_string()]);
        return Ok(argv);
    }
    let send = find("notify-send").ok_or(NO_NOTIFY_SEND)?;
    let flags = [format!("--app-name={APP}"), format!("--icon={ICON}"), "--".to_string()];
    Ok([lossy(&send)].into_iter().chain(flags).chain([title.to_string(), body.to_string()]).collect())
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

pub(super) fn sound(os: Os, file: &Path, find: &dyn Fn(&str) -> Option<PathBuf>) -> Result<Vec<String>, &'static str> {
    if os == Os::Mac {
        return Ok(vec!["afplay".to_string(), lossy(file)]);
    }
    if let Some(player) = find("paplay").or_else(|| find("pw-play")) {
        return Ok(vec![lossy(&player), lossy(file)]);
    }
    let wav = file.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav"));
    match find("aplay") {
        Some(aplay) if wav => Ok(vec![lossy(&aplay), "-q".to_string(), lossy(file)]),
        _ => Err(NO_PLAYER),
    }
}

#[cfg(test)]
mod tests;
