use super::*;
use crate::test_util::in_usr_bin as only;

fn command(argv: &[&str]) -> Helper {
    Helper::Command(argv.iter().map(OsString::from).collect())
}

fn argv(helper: &Helper) -> &[OsString] {
    match helper {
        Helper::Command(argv) => argv,
        other => panic!("not a command: {other:?}"),
    }
}

#[test]
fn a_mac_without_a_bundle_notifies_through_osascript_with_the_text_as_arguments() {
    let helper =
        notification(Os::Mac, None, ("Claude", "done; \"rm\""), &only(&[])).expect("osascript is always there");
    let argv = argv(&helper);
    assert_eq!(argv.first().and_then(|program| program.to_str()), Some("osascript"));
    assert_eq!(argv[argv.len() - 2..], ["Claude", "done; \"rm\""]);
}

#[test]
fn a_mac_app_posts_the_notices_text_through_its_own_center() {
    let helper =
        notification(Os::Mac, Some("4242-7".into()), ("claude finished", "claude finished in shop › main"), &only(&[]));
    assert_eq!(
        helper,
        Ok(Helper::Center {
            id: "4242-7".into(),
            title: "claude finished".into(),
            body: "claude finished in shop › main".into()
        })
    );
}

#[test]
fn only_a_mac_uses_a_notifications_id() {
    let find = only(&["notify-send"]);
    for os in [Os::Linux, Os::Windows] {
        let text = ("claude finished", "in shop › main");
        assert_eq!(notification(os, Some("1-1".into()), text, &find), notification(os, None, text, &find));
    }
}

#[test]
fn linux_notifies_through_notify_send_with_the_text_after_its_options() {
    let helper =
        notification(Os::Linux, None, ("-claude", "--help"), &only(&["notify-send"])).expect("notify-send is there");
    let expected =
        ["/usr/bin/notify-send", "--app-name=cornercase", "--icon=utilities-terminal", "--", "-claude", "--help"];
    assert_eq!(helper, command(&expected));
    assert_eq!(notification(Os::Linux, None, ("a", "b"), &only(&[])), Err(NO_NOTIFY_SEND));
}

#[test]
fn windows_notifies_with_a_silent_toast_whatever_the_path_holds() {
    let helper = notification(Os::Windows, None, ("claude finished", "in shop › main"), &only(&[])).expect("a toast");
    let expected = "<toast><visual><binding template=\"ToastGeneric\"><text>claude finished</text>\
        <text>in shop › main</text></binding></visual><audio silent=\"true\"/></toast>";
    assert_eq!(helper, Helper::Toast(expected.to_string()));
}

#[test]
fn a_toasts_text_is_escaped_so_it_cannot_change_the_xml() {
    let xml = toast_xml("<b>&'\"", "</text><audio src=\"x\"/>\u{7}\u{1b}\ttab\nline");
    assert!(xml.contains("<text>&lt;b&gt;&amp;&apos;&quot;</text>"), "{xml}");
    assert!(xml.contains("<text>&lt;/text&gt;&lt;audio src=&quot;x&quot;/&gt;\ttab\nline</text>"), "{xml}");
    assert_eq!(xml.matches("<text>").count(), 2);
    assert!(!xml.contains('\u{7}') && !xml.contains('\u{1b}'));
}

#[test]
fn windows_knows_the_app_by_a_key_of_the_users_own_classes() {
    assert_eq!(app_key("Cornercase.Gpui"), r"Software\Classes\AppUserModelId\Cornercase.Gpui");
}

#[test]
fn a_mac_plays_its_system_sounds_with_afplay() {
    assert_eq!(system_sound(Os::Mac, "finished", &[]), Some("/System/Library/Sounds/Glass.aiff".into()));
    assert_eq!(system_sound(Os::Mac, "needs-you", &[]), Some("/System/Library/Sounds/Ping.aiff".into()));
    assert_eq!(
        sound(Os::Mac, Path::new("/s/a.aiff"), &only(&[])).expect("afplay"),
        [command(&["afplay", "/s/a.aiff"])]
    );
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
    assert_eq!(no_system_sound(Os::Linux), NO_THEME);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn windows_plays_its_own_media_sounds_from_the_windows_folder() {
    let root = std::env::temp_dir().join(format!("cc-gpui-media-{}", std::process::id()));
    let media = root.join(MEDIA);
    std::fs::create_dir_all(&media).expect("the temporary files are writable");
    for name in ["tada.wav", "Windows Notify Messaging.wav"] {
        std::fs::write(media.join(name), "").expect("the temporary files are writable");
    }
    let data = [root.clone()];
    assert_eq!(cue_sound(Os::Windows, "finished", None, &data), Some(media.join("tada.wav")));
    assert_eq!(cue_sound(Os::Windows, "needs-you", None, &data), Some(media.join("Windows Notify Messaging.wav")));
    assert_eq!(system_sound(Os::Windows, "finished", &[root.join("empty")]), None);
    assert_eq!(no_system_sound(Os::Windows), NO_MEDIA);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn windows_plays_a_wav_itself_and_says_it_cannot_play_anything_else() {
    let all = only(&["paplay", "pw-play", "aplay", "afplay"]);
    let wav = Path::new(r"C:\Users\me\ding.WAV");
    assert_eq!(sound(Os::Windows, wav, &all), Ok(vec![Helper::PlaySound(wav.to_path_buf())]));
    assert_eq!(sound(Os::Windows, Path::new(r"C:\Users\me\ding.mp3"), &all), Err(ONLY_WAV));
    assert_eq!(sound(Os::Windows, Path::new(r"C:\Users\me\ding"), &all), Err(ONLY_WAV));
}

#[test]
fn linux_tries_paplay_then_pw_play_then_aplay_for_wav_only() {
    let file = Path::new("/s/complete.oga");
    let all = only(&["paplay", "pw-play", "aplay"]);
    let both = [command(&["/usr/bin/paplay", "/s/complete.oga"]), command(&["/usr/bin/pw-play", "/s/complete.oga"])];
    assert_eq!(sound(Os::Linux, file, &all).expect("a player"), both);
    let pipewire = only(&["pw-play", "aplay"]);
    assert_eq!(
        sound(Os::Linux, file, &pipewire).expect("a player"),
        [command(&["/usr/bin/pw-play", "/s/complete.oga"])]
    );
    let alsa = only(&["aplay"]);
    assert_eq!(sound(Os::Linux, file, &alsa), Err(NO_PLAYER));
    let wav = Path::new("/s/ding.WAV");
    let tries = sound(Os::Linux, wav, &all).expect("a player");
    assert_eq!(tries.len(), 3);
    assert_eq!(tries[2], command(&["/usr/bin/aplay", "-q", "/s/ding.WAV"]));
}

#[test]
fn a_player_that_fails_hands_the_sound_to_the_next_and_one_that_plays_ends_the_tries() {
    let tries = sound(Os::Linux, Path::new("/s/ding.wav"), &only(&["paplay", "pw-play", "aplay"])).expect("players");
    let ran = std::cell::RefCell::new(Vec::new());
    let pulse_is_down = |helper: &Helper| {
        let program = argv(helper)[0].clone();
        ran.borrow_mut().push(program.clone());
        if program == "/usr/bin/paplay" { Err("paplay: Connection refused".to_string()) } else { Ok(()) }
    };
    assert_eq!(first_that_works(&tries, pulse_is_down), Ok(()));
    assert_eq!(*ran.borrow(), ["/usr/bin/paplay", "/usr/bin/pw-play"]);
}

#[test]
fn when_every_helper_fails_each_says_why() {
    let tries = [command(&["paplay"]), Helper::PlaySound("ding.wav".into())];
    let failing = |helper: &Helper| match helper {
        Helper::Command(argv) => Err(format!("{} failed", argv[0].to_string_lossy())),
        _ => Err("PlaySound failed".to_string()),
    };
    assert_eq!(first_that_works(&tries, failing), Err("paplay failed; PlaySound failed".to_string()));
}

#[cfg(unix)]
#[test]
fn a_helper_that_fails_is_known_by_its_last_words_or_its_status() {
    let sh = |script: &str| run_command(&["/bin/sh", "-c", script].map(OsString::from));
    assert_eq!(sh("exit 0"), Ok(()));
    assert_eq!(
        sh("echo 'sh: Connection failure: Connection refused' >&2; exit 1"),
        Err("sh: Connection failure: Connection refused".into())
    );
    assert_eq!(sh("exit 3"), Err("sh ended with exit status: 3".into()));
}

#[cfg(windows)]
#[test]
fn a_windows_helper_that_fails_is_known_by_its_last_words_or_its_exit_code() {
    let cmd = |script: &str| run_command(&["cmd", "/c", script].map(OsString::from));
    assert_eq!(cmd("exit 0"), Ok(()));
    assert_eq!(cmd("echo cmd: it failed 1>&2 & exit 1"), Err("cmd: it failed".into()));
    assert_eq!(cmd("exit 3"), Err("cmd ended with exit code: 3".into()));
}

#[test]
fn a_helper_that_cannot_start_says_so() {
    let missing = run_command(&[OsString::from("/nonexistent/pw-play")]).expect_err("there is no such program");
    assert!(missing.starts_with("could not start pw-play: "), "{missing}");
}

#[cfg(not(windows))]
#[test]
fn off_windows_its_sound_and_toast_say_they_cannot_run() {
    assert!(run(&Helper::PlaySound("ding.wav".into())).is_err());
    assert!(run(&Helper::Toast(toast_xml("a", "b"))).is_err());
}

#[cfg(unix)]
#[test]
fn a_sound_or_a_player_whose_path_is_not_utf8_is_passed_as_it_is() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt as _;

    let file = Path::new(OsStr::from_bytes(b"/s/caf\xe9.oga"));
    let player = |name: &str| (name == "paplay").then(|| PathBuf::from(OsStr::from_bytes(b"/opt/b\xe9/paplay")));
    let tries = sound(Os::Linux, file, &player).expect("a player");
    let args = argv(&tries[0]);
    assert_eq!(args[0].as_bytes(), b"/opt/b\xe9/paplay");
    assert_eq!(args[1].as_bytes(), b"/s/caf\xe9.oga");
    let mac = sound(Os::Mac, file, &only(&[])).expect("afplay");
    assert_eq!(argv(&mac[0])[1].as_bytes(), b"/s/caf\xe9.oga");
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
