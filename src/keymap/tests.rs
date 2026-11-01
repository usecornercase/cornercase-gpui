use super::{Command, Group, SHORTCUTS, find, in_group, keys_label, keys_of, tab_index, with_keys};
use crate::panels::PanelKind;

fn command(key: &str, shift: bool) -> Option<Command> {
    find(key, shift, false).map(|s| s.command)
}

fn in_browser(key: &str, shift: bool) -> Option<Command> {
    find(key, shift, true).map(|s| s.command)
}

#[test]
fn a_shifted_key_needs_shift() {
    assert_eq!(command("c", false), Some(Command::Copy));
    assert_eq!(command("c", true), Some(Command::Panel(PanelKind::Changes)));
    assert_eq!(command("w", false), Some(Command::Close));
    assert_eq!(command("w", true), Some(Command::CloseTab));
    assert_eq!(command("[", false), Some(Command::PreviousPane));
    assert_eq!(command("[", true), Some(Command::PreviousTab));
}

#[test]
fn an_unshifted_key_also_answers_with_shift_when_nothing_else_does() {
    assert_eq!(command("i", true), Some(Command::Panel(PanelKind::Issues)));
    assert_eq!(command("=", true), Some(Command::ZoomIn));
    assert_eq!(command("?", true), Some(Command::Shortcuts));
    assert_eq!(command("k", false), None);
}

#[test]
fn shift_p_goes_to_a_file_and_shift_m_shows_processes() {
    assert_eq!(command("p", true), Some(Command::FindFile));
    assert_eq!(command("m", true), Some(Command::Panel(PanelKind::Processes)));
    assert_eq!(keys_of(Command::FindFile).as_deref(), Some("⇧⌘P"));
}

#[test]
fn p_without_shift_shows_the_pull_request() {
    assert_eq!(command("p", false), Some(Command::PullRequest));
    assert_eq!(keys_of(Command::PullRequest).as_deref(), Some("⌘P"));
}

#[test]
fn no_shortcut_takes_a_key_macos_or_the_app_menu_keeps() {
    let kept = [("q", true), ("3", true), ("4", true), ("5", true), ("q", false), ("h", false), ("m", false)];
    for (key, shift) in kept {
        assert!(!SHORTCUTS.iter().any(|s| s.shift == shift && s.takes(key)), "{} is kept", keys_label(key, shift));
    }
}

#[test]
fn every_panel_has_keys() {
    for kind in PanelKind::ALL {
        assert!(keys_of(Command::Panel(kind)).is_some(), "{kind:?} has no keys");
    }
}

#[test]
fn no_two_shortcuts_share_a_chord() {
    for (i, a) in SHORTCUTS.iter().enumerate() {
        for b in &SHORTCUTS[i + 1..] {
            let shared = a.keys.iter().any(|k| b.keys.contains(k)) && a.in_browser() == b.in_browser();
            assert!(!(shared && a.shift == b.shift), "{:?} and {:?} share a chord", a.command, b.command);
        }
    }
}

#[test]
fn every_shortcut_belongs_to_a_shown_group() {
    let shown: usize = Group::ALL.into_iter().map(|g| in_group(g).count()).sum();
    assert_eq!(shown, SHORTCUTS.len());
}

#[test]
fn a_tooltip_names_the_keys() {
    assert_eq!(with_keys("Changes", Command::Panel(PanelKind::Changes)), "Changes  ⇧⌘C");
    assert_eq!(with_keys("Usage", Command::Panel(PanelKind::Usage)), "Usage  ⌘U");
    assert_eq!(with_keys("Bigger", Command::ZoomIn), "Bigger  ⌘+");
    assert_eq!(keys_of(Command::Tab).as_deref(), Some("⌘1…9"));
}

#[test]
fn nine_goes_to_the_last_tab() {
    assert_eq!(tab_index("1", 3), Some(0));
    assert_eq!(tab_index("3", 3), Some(2));
    assert_eq!(tab_index("4", 3), None);
    assert_eq!(tab_index("9", 3), Some(2));
    assert_eq!(tab_index("9", 0), None);
    assert_eq!(tab_index("x", 3), None);
}

#[test]
fn in_the_browser_brackets_go_back_and_forward() {
    assert_eq!(in_browser("[", false), Some(Command::Back));
    assert_eq!(in_browser("]", false), Some(Command::Forward));
    assert_eq!(command("[", false), Some(Command::PreviousPane));
}

#[test]
fn shifted_brackets_still_switch_tabs_in_the_browser() {
    assert_eq!(in_browser("[", true), Some(Command::PreviousTab));
    assert_eq!(in_browser("]", true), Some(Command::NextTab));
}

#[test]
fn browser_keys_answer_only_in_the_browser() {
    assert_eq!(in_browser("l", false), Some(Command::FocusAddress));
    assert_eq!(in_browser("r", false), Some(Command::Reload));
    assert_eq!(in_browser("x", false), Some(Command::Cut));
    assert_eq!(command("l", false), None);
    assert_eq!(command("r", false), None);
}

#[test]
fn the_browser_keeps_the_window_shortcuts() {
    assert_eq!(in_browser("c", false), Some(Command::Copy));
    assert_eq!(in_browser("w", false), Some(Command::Close));
    assert_eq!(in_browser("b", true), Some(Command::Panel(PanelKind::Browser)));
}
