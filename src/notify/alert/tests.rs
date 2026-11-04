use super::*;

fn only(tools: &'static [&'static str]) -> impl Fn(&str) -> Option<PathBuf> {
    move |name| tools.contains(&name).then(|| PathBuf::from("/usr/bin").join(name))
}

#[test]
fn a_mac_notifies_through_osascript_with_the_text_as_arguments() {
    let argv = notification(Os::Mac, ("Claude", "done; \"rm\""), &only(&[])).expect("osascript is always there");
    assert_eq!(argv.first().map(String::as_str), Some("osascript"));
    assert_eq!(argv[argv.len() - 2..], ["Claude", "done; \"rm\""]);
}

#[test]
fn linux_notifies_through_notify_send_with_the_text_after_its_options() {
    let argv = notification(Os::Linux, ("-claude", "--help"), &only(&["notify-send"])).expect("notify-send is there");
    let expected =
        ["/usr/bin/notify-send", "--app-name=cornercase", "--icon=utilities-terminal", "--", "-claude", "--help"];
    assert_eq!(argv, expected);
    assert_eq!(notification(Os::Linux, ("a", "b"), &only(&[])), Err(NO_NOTIFY_SEND));
}

#[test]
fn a_mac_plays_its_system_sounds_with_afplay() {
    assert_eq!(system_sound(Os::Mac, "finished", &[]), Some("/System/Library/Sounds/Glass.aiff".into()));
    assert_eq!(system_sound(Os::Mac, "needs-you", &[]), Some("/System/Library/Sounds/Ping.aiff".into()));
    assert_eq!(sound(Os::Mac, Path::new("/s/a.aiff"), &only(&[])).expect("afplay"), ["afplay", "/s/a.aiff"]);
}

#[test]
fn linux_plays_the_freedesktop_theme_found_in_the_data_folders() {
    let root = std::env::temp_dir().join(format!("cc-gpui-sounds-{}", std::process::id()));
    let stereo = root.join("share").join(FREEDESKTOP);
    std::fs::create_dir_all(&stereo).expect("the temporary files are writable");
    std::fs::write(stereo.join("complete.oga"), "").expect("the temporary files are writable");
    let data = [root.join("empty"), root.join("share")];
    assert_eq!(system_sound(Os::Linux, "finished", &data), Some(stereo.join("complete.oga")));
    assert_eq!(system_sound(Os::Linux, "needs-you", &data), None);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn linux_plays_with_paplay_then_pw_play_then_aplay_for_wav_only() {
    let file = Path::new("/s/complete.oga");
    let both = only(&["paplay", "pw-play", "aplay"]);
    assert_eq!(sound(Os::Linux, file, &both).expect("a player"), ["/usr/bin/paplay", "/s/complete.oga"]);
    let pipewire = only(&["pw-play", "aplay"]);
    assert_eq!(sound(Os::Linux, file, &pipewire).expect("a player"), ["/usr/bin/pw-play", "/s/complete.oga"]);
    let alsa = only(&["aplay"]);
    assert_eq!(sound(Os::Linux, file, &alsa), Err(NO_PLAYER));
    let wav = Path::new("/s/ding.WAV");
    assert_eq!(sound(Os::Linux, wav, &alsa).expect("aplay reads wav"), ["/usr/bin/aplay", "-q", "/s/ding.WAV"]);
}
