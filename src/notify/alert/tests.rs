use super::*;

fn only(tools: &'static [&'static str]) -> impl Fn(&str) -> Option<PathBuf> {
    move |name| tools.contains(&name).then(|| PathBuf::from("/usr/bin").join(name))
}

#[test]
fn a_mac_notifies_through_osascript_with_the_text_as_arguments() {
    let argv = notification(Os::Mac, ("Claude", "done; \"rm\""), &only(&[])).expect("osascript is always there");
    assert_eq!(argv.first().and_then(|program| program.to_str()), Some("osascript"));
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
    assert_eq!(sound(Os::Mac, Path::new("/s/a.aiff"), &only(&[])).expect("afplay"), [["afplay", "/s/a.aiff"]]);
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
fn linux_tries_paplay_then_pw_play_then_aplay_for_wav_only() {
    let file = Path::new("/s/complete.oga");
    let all = only(&["paplay", "pw-play", "aplay"]);
    let both = [["/usr/bin/paplay", "/s/complete.oga"], ["/usr/bin/pw-play", "/s/complete.oga"]];
    assert_eq!(sound(Os::Linux, file, &all).expect("a player"), both);
    let pipewire = only(&["pw-play", "aplay"]);
    assert_eq!(sound(Os::Linux, file, &pipewire).expect("a player"), [["/usr/bin/pw-play", "/s/complete.oga"]]);
    let alsa = only(&["aplay"]);
    assert_eq!(sound(Os::Linux, file, &alsa), Err(NO_PLAYER));
    let wav = Path::new("/s/ding.WAV");
    let tries = sound(Os::Linux, wav, &all).expect("a player");
    assert_eq!(tries.len(), 3);
    assert_eq!(tries[2], ["/usr/bin/aplay", "-q", "/s/ding.WAV"]);
}

#[test]
fn a_player_that_fails_hands_the_sound_to_the_next_and_one_that_plays_ends_the_tries() {
    let tries = sound(Os::Linux, Path::new("/s/ding.wav"), &only(&["paplay", "pw-play", "aplay"])).expect("players");
    let ran = std::cell::RefCell::new(Vec::new());
    let pulse_is_down = |argv: &[OsString]| {
        ran.borrow_mut().push(argv[0].clone());
        if argv[0] == "/usr/bin/paplay" { Err("paplay: Connection refused".to_string()) } else { Ok(()) }
    };
    assert_eq!(first_that_works(&tries, pulse_is_down), Ok(()));
    assert_eq!(*ran.borrow(), ["/usr/bin/paplay", "/usr/bin/pw-play"]);
}

#[test]
fn when_every_helper_fails_each_says_why() {
    let tries = [vec![OsString::from("paplay")], vec![OsString::from("pw-play")]];
    let failing = |argv: &[OsString]| Err(format!("{} failed", argv[0].to_string_lossy()));
    assert_eq!(first_that_works(&tries, failing), Err("paplay failed; pw-play failed".to_string()));
}

#[test]
fn a_helper_that_fails_is_known_by_its_last_words_or_its_status() {
    let sh = |script: &str| run(&["/bin/sh", "-c", script].map(OsString::from));
    assert_eq!(sh("exit 0"), Ok(()));
    assert_eq!(
        sh("echo 'sh: Connection failure: Connection refused' >&2; exit 1"),
        Err("sh: Connection failure: Connection refused".into())
    );
    assert_eq!(sh("exit 3"), Err("sh ended with exit status: 3".into()));
    let missing = run(&[OsString::from("/nonexistent/pw-play")]).expect_err("there is no such program");
    assert!(missing.starts_with("could not start pw-play: "), "{missing}");
}

#[cfg(unix)]
#[test]
fn a_sound_or_a_player_whose_path_is_not_utf8_is_passed_as_it_is() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt as _;

    let file = Path::new(OsStr::from_bytes(b"/s/caf\xe9.oga"));
    let player = |name: &str| (name == "paplay").then(|| PathBuf::from(OsStr::from_bytes(b"/opt/b\xe9/paplay")));
    let argv = &sound(Os::Linux, file, &player).expect("a player")[0];
    assert_eq!(argv[0].as_bytes(), b"/opt/b\xe9/paplay");
    assert_eq!(argv[1].as_bytes(), b"/s/caf\xe9.oga");
    assert_eq!(sound(Os::Mac, file, &only(&[])).expect("afplay")[0][1].as_bytes(), b"/s/caf\xe9.oga");
}

#[test]
fn without_a_sound_of_its_own_a_cue_plays_the_system_sound() {
    for (cue, system) in [("needs-you", "Ping"), ("finished", "Glass")] {
        let played = cue_sound(Os::Mac, cue, None, &[]);
        assert_eq!(played, Some(PathBuf::from(format!("/System/Library/Sounds/{system}.aiff"))));
        assert_eq!(cue_sound(Os::Mac, cue, Some("/s/own.wav".into()), &[]), Some("/s/own.wav".into()));
    }
}

#[test]
fn on_linux_a_cue_without_a_sound_of_its_own_plays_the_theme() {
    let root = std::env::temp_dir().join(format!("cc-gpui-cues-{}", std::process::id()));
    let stereo = root.join(FREEDESKTOP);
    std::fs::create_dir_all(&stereo).expect("the temporary files are writable");
    for name in ["complete.oga", "message-new-instant.oga"] {
        std::fs::write(stereo.join(name), "").expect("the temporary files are writable");
    }
    let data = [root.clone()];
    assert_eq!(cue_sound(Os::Linux, "finished", None, &data), Some(stereo.join("complete.oga")));
    assert_eq!(cue_sound(Os::Linux, "needs-you", None, &data), Some(stereo.join("message-new-instant.oga")));
    let _ = std::fs::remove_dir_all(&root);
}
