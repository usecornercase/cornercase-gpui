use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::desktop;
use crate::platform::{self, Os};

#[cfg(windows)]
mod win32;
#[cfg(not(windows))]
#[path = "alert/no_win32.rs"]
mod win32;

const APP: &str = "cornercase";
const ICON: &str = "utilities-terminal";
const FREEDESKTOP: &str = "sounds/freedesktop/stereo";
const MEDIA: &str = "Media";

pub(super) const NO_NOTIFY_SEND: &str =
    "No desktop notifications: install notify-send (libnotify-bin on Debian and Ubuntu, libnotify elsewhere)";
pub(super) const NO_PLAYER: &str = "No sound: install paplay (pulseaudio-utils) or pw-play (pipewire-bin)";
pub(super) const NO_THEME: &str =
    "No sound: install the freedesktop sound theme (sound-theme-freedesktop), or choose sound files in Settings";
pub(super) const NO_MEDIA: &str =
    "No sound: Windows' own sounds are missing from its Media folder; choose sound files in Settings";
pub(super) const ONLY_WAV: &str = "No sound: Windows plays only WAV files; choose a .wav file in Settings";
pub(super) const SOUND_FAILED: &str = "No sound";
pub(super) const NOTIFICATION_FAILED: &str = "No desktop notification";

const PLAYERS: [(&str, &[&str]); 3] = [("paplay", &[]), ("pw-play", &[]), ("aplay", &["-q"])];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Helper {
    Command(Vec<OsString>),
    PlaySound(PathBuf),
    Toast(String),
    Center { id: String, title: String, body: String },
}

pub(super) fn notification(
    os: Os,
    id: Option<String>,
    (title, body): (&str, &str),
    find: &dyn Fn(&str) -> Option<PathBuf>,
) -> Result<Helper, &'static str> {
    match (os, id) {
        (Os::Mac, Some(id)) => Ok(Helper::Center { id, title: title.into(), body: body.into() }),
        (Os::Mac, None) => Ok(Helper::Command(osascript(title, body))),
        (Os::Linux, _) => notify_send(title, body, find).map(Helper::Command),
        (Os::Windows, _) => Ok(Helper::Toast(toast_xml(title, body))),
    }
}

fn osascript(title: &str, body: &str) -> Vec<OsString> {
    let script = ["on run argv", "display notification (item 2 of argv) with title (item 1 of argv)", "end run"];
    let mut argv = vec![OsString::from("osascript")];
    argv.extend(script.iter().flat_map(|line| [OsString::from("-e"), OsString::from(line)]));
    argv.extend([OsString::from(title), OsString::from(body)]);
    argv
}

fn notify_send(title: &str, body: &str, find: &dyn Fn(&str) -> Option<PathBuf>) -> Result<Vec<OsString>, &'static str> {
    let send = find("notify-send").ok_or(NO_NOTIFY_SEND)?;
    let flags = [format!("--app-name={APP}"), format!("--icon={ICON}"), "--".to_string(), title.into(), body.into()];
    Ok(std::iter::once(send.into_os_string()).chain(flags.map(OsString::from)).collect())
}

pub(super) fn toast_xml(title: &str, body: &str) -> String {
    let (title, body) = (xml_text(title), xml_text(body));
    let texts = format!("<text>{title}</text><text>{body}</text>");
    format!(
        "<toast><visual><binding template=\"ToastGeneric\">{texts}</binding></visual><audio silent=\"true\"/></toast>"
    )
}

fn xml_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars().filter(|&c| xml_char(c)) {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

fn xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r') || (c >= ' ' && c != '\u{fffe}' && c != '\u{ffff}')
}

pub(super) fn app_key(id: &str) -> String {
    format!(r"Software\Classes\AppUserModelId\{id}")
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
        Os::Windows => {
            let name = if finished { "tada.wav" } else { "Windows Notify Messaging.wav" };
            data.iter().map(|dir| dir.join(MEDIA).join(name)).find(|path| path.is_file())
        }
    }
}

pub(super) const fn no_system_sound(os: Os) -> &'static str {
    match os {
        Os::Windows => NO_MEDIA,
        Os::Mac | Os::Linux => NO_THEME,
    }
}

pub(super) fn cue_sound(os: Os, cue: &str, own: Option<PathBuf>, data: &[PathBuf]) -> Option<PathBuf> {
    own.or_else(|| system_sound(os, cue, data))
}

pub(super) fn sound(os: Os, file: &Path, find: &dyn Fn(&str) -> Option<PathBuf>) -> Result<Vec<Helper>, &'static str> {
    match os {
        Os::Mac => Ok(vec![Helper::Command(vec!["afplay".into(), file.as_os_str().to_owned()])]),
        Os::Linux => players(file, find),
        Os::Windows if is_wav(file) => Ok(vec![Helper::PlaySound(file.to_path_buf())]),
        Os::Windows => Err(ONLY_WAV),
    }
}

fn is_wav(file: &Path) -> bool {
    file.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav"))
}

fn players(file: &Path, find: &dyn Fn(&str) -> Option<PathBuf>) -> Result<Vec<Helper>, &'static str> {
    let wav = is_wav(file);
    let file = file.as_os_str();
    let tries: Vec<Helper> = PLAYERS
        .iter()
        .filter(|(name, _)| wav || *name != "aplay")
        .filter_map(|(name, flags)| {
            let player = find(name)?.into_os_string();
            let argv = [player].into_iter().chain(flags.iter().map(OsString::from)).chain([file.to_owned()]);
            Some(Helper::Command(argv.collect()))
        })
        .collect();
    if tries.is_empty() { Err(NO_PLAYER) } else { Ok(tries) }
}

pub(super) fn first_that_works(tries: &[Helper], run: impl Fn(&Helper) -> Result<(), String>) -> Result<(), String> {
    let mut failures = Vec::new();
    for helper in tries {
        match run(helper) {
            Ok(()) => return Ok(()),
            Err(why) => failures.push(why),
        }
    }
    Err(failures.join("; "))
}

pub(super) fn run(helper: &Helper) -> Result<(), String> {
    let id = desktop::APP_USER_MODEL_ID;
    match helper {
        Helper::Command(argv) => run_command(argv),
        Helper::PlaySound(file) => win32::play(file),
        Helper::Toast(xml) => win32::toast((id, &app_key(id), APP), xml),
        Helper::Center { id, title, body } => super::center::post((id, title, body)),
    }
}

pub(super) fn run_command(argv: &[OsString]) -> Result<(), String> {
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
