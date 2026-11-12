use super::*;
use crate::desktop::Scratch;

#[test]
fn the_note_lives_beside_the_settings() {
    assert_eq!(path(Path::new("/x/gpui/settings.json")), Path::new("/x/gpui/relaunch.json"));
    assert_eq!(path(Path::new("/x/gpui/relaunch.json")), Path::new("/x/gpui/window-relaunch.json"));
}

#[test]
fn the_awaited_version_is_read_once() {
    let scratch = Scratch::folder("arrival").expect("a folder");
    let file = scratch.path().join("relaunch.json");
    let awaited = Awaited { version: "0.1.65".into(), program: PathBuf::from("/Applications/cornercase.app") };
    remember(&file, &awaited).expect("written");
    assert_eq!(take(&file), Some(awaited));
    assert_eq!(take(&file), None);
    assert!(!file.exists());
}

#[test]
fn a_broken_note_is_dropped() {
    let scratch = Scratch::folder("arrival-broken").expect("a folder");
    let file = scratch.path().join("relaunch.json");
    std::fs::write(&file, "{ not json").expect("written");
    assert_eq!(take(&file), None);
    assert!(!file.exists());
}

#[test]
fn the_same_version_says_it_took() {
    let toast = toast("0.1.65", "0.1.65", "~/a", "~/a");
    assert_eq!((toast.text.as_str(), toast.kind), ("cornercase-gpui is now 0.1.65", ToastKind::Done));
}

#[test]
fn a_newer_window_also_took() {
    assert_eq!(toast("0.1.65", "0.1.66", "~/a", "~/a").kind, ToastKind::Done);
}

#[test]
fn an_older_window_in_the_same_place_says_so() {
    let toast = toast("0.1.65", "0.1.64", "/Applications/cornercase.app", "/Applications/cornercase.app");
    assert_eq!(toast.kind, ToastKind::Error);
    assert_eq!(
        toast.text,
        "This window runs cornercase-gpui 0.1.64, not 0.1.65, which the update installed in /Applications/cornercase.app"
    );
}

#[test]
fn another_copy_says_where_it_started() {
    let toast = toast("0.1.65", "0.1.64", "~/Downloads/cornercase.app", "/Applications/cornercase.app");
    assert_eq!(
        toast.text,
        "This window runs cornercase-gpui 0.1.64 from ~/Downloads/cornercase.app, not 0.1.65, which the update installed in /Applications/cornercase.app"
    );
}
