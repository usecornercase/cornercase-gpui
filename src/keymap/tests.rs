use gpui::{Keystroke, Modifiers};

use super::{
    Command, Group, SHORTCUTS, chord, find, in_group, keys_label, keys_of, labelled, left_to_system, opens_with_system,
    other_keys, shortcut, system_keys, tab_index, typed_with_alt_gr,
};
use crate::panels::PanelKind;
use crate::platform::Os;

fn command(key: &str, shift: bool) -> Option<Command> {
    find(key, shift, false, Os::Mac).map(|s| s.command)
}

fn in_browser(key: &str, shift: bool) -> Option<Command> {
    find(key, shift, true, Os::Mac).map(|s| s.command)
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
    assert_eq!(keys_of(Command::FindFile, Os::Mac).as_deref(), Some("⇧⌘P"));
}

#[test]
fn p_without_shift_shows_the_pull_request() {
    assert_eq!(command("p", false), Some(Command::PullRequest));
    assert_eq!(keys_of(Command::PullRequest, Os::Mac).as_deref(), Some("⌘P"));
}

#[test]
fn no_shortcut_takes_a_key_macos_or_the_app_menu_keeps() {
    let kept = [("q", true), ("3", true), ("4", true), ("5", true), ("q", false), ("h", false), ("m", false)];
    for (key, shift) in kept {
        assert!(
            !SHORTCUTS.iter().any(|s| s.shift == shift && s.takes(key)),
            "{} is kept",
            keys_label(key, shift, Os::Mac)
        );
    }
}

#[test]
fn every_panel_has_keys() {
    for kind in PanelKind::ALL {
        assert!(keys_of(Command::Panel(kind), Os::Mac).is_some(), "{kind:?} has no keys");
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
    assert_eq!(labelled("Changes", Command::Panel(PanelKind::Changes), Os::Mac), "Changes  ⇧⌘C");
    assert_eq!(labelled("Usage", Command::Panel(PanelKind::Usage), Os::Mac), "Usage  ⌘U");
    assert_eq!(labelled("Bigger", Command::ZoomIn, Os::Mac), "Bigger  ⌘+");
    assert_eq!(keys_of(Command::Tab, Os::Mac).as_deref(), Some("⌘1…9"));
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

const CTRL_SHIFT: Modifiers = Modifiers { control: true, shift: true, alt: false, platform: false, function: false };
const CTRL_ALT_SHIFT: Modifiers = Modifiers { alt: true, ..CTRL_SHIFT };
const CTRL: Modifiers = Modifiers { shift: false, ..CTRL_SHIFT };
const CTRL_ALT: Modifiers = Modifiers { alt: true, ..CTRL };
const ALT: Modifiers = Modifiers { control: false, ..CTRL_ALT };
const NONE: Modifiers = Modifiers { alt: false, ..ALT };
const SUPER: Modifiers = Modifiers { platform: true, ..NONE };

fn pressed(key: &str, modifiers: Modifiers) -> Keystroke {
    Keystroke { modifiers, key: key.into(), key_char: None }
}

fn on_linux(key: &str, modifiers: Modifiers) -> Option<Command> {
    shortcut(&pressed(key, modifiers), modifiers.shift, Os::Linux, false).map(|s| s.command)
}

fn shift_held(key: &str, modifiers: Modifiers) -> Option<Command> {
    shortcut(&pressed(key, modifiers), true, Os::Linux, false).map(|s| s.command)
}

fn in_a_page(key: &str, modifiers: Modifiers) -> Option<Command> {
    shortcut(&pressed(key, modifiers), modifiers.shift, Os::Linux, true).map(|s| s.command)
}

#[test]
fn on_linux_ctrl_shift_is_the_command_key() {
    assert_eq!(on_linux("t", CTRL_SHIFT), Some(Command::NewTab));
    assert_eq!(on_linux("w", CTRL_SHIFT), Some(Command::Close));
    assert_eq!(on_linux("c", CTRL_SHIFT), Some(Command::Copy));
    assert_eq!(on_linux("v", CTRL_SHIFT), Some(Command::Paste));
    assert_eq!(on_linux("f", CTRL_SHIFT), Some(Command::Panel(PanelKind::Search)));
    assert_eq!(on_linux("o", CTRL_SHIFT), Some(Command::OpenProject));
}

#[test]
fn on_linux_ctrl_alt_shift_is_shift_and_the_command_key() {
    assert_eq!(on_linux("c", CTRL_ALT_SHIFT), Some(Command::Panel(PanelKind::Changes)));
    assert_eq!(on_linux("w", CTRL_ALT_SHIFT), Some(Command::CloseTab));
    assert_eq!(on_linux("p", CTRL_ALT_SHIFT), Some(Command::FindFile));
    assert_eq!(shift_held("{", CTRL_ALT), Some(Command::PreviousTab));
    assert_eq!(shift_held("}", CTRL_ALT), Some(Command::NextTab));
}

#[test]
fn a_shifted_symbol_reads_as_its_key_on_a_us_keyboard() {
    assert_eq!(shift_held("{", CTRL), Some(Command::PreviousPane));
    assert_eq!(shift_held("}", CTRL), Some(Command::NextPane));
    assert_eq!(shift_held("?", CTRL), Some(Command::Shortcuts));
    assert_eq!(shift_held("+", CTRL), Some(Command::ZoomIn));
    assert_eq!(shift_held("_", CTRL), Some(Command::ZoomOut));
    assert_eq!(shift_held(")", CTRL), Some(Command::ZoomReset));
    assert_eq!(shift_held("<", CTRL), Some(Command::Panel(PanelKind::Settings)));
}

#[test]
fn a_symbol_typed_with_ctrl_and_no_shift_reaches_the_program() {
    assert_eq!(on_linux("<", CTRL), None);
    assert_eq!(on_linux("_", CTRL), None);
    assert_eq!(on_linux(")", CTRL), None);
    assert_eq!(on_linux("+", CTRL), None);
    assert_eq!(on_linux("?", CTRL), None);
    assert_eq!(on_linux("{", CTRL), None);
    assert_eq!(on_linux("{", CTRL_ALT), None);
}

#[test]
fn with_shift_held_a_symbol_of_another_layout_is_read_as_typed() {
    assert_eq!(shift_held("=", CTRL), Some(Command::ZoomIn));
    assert_eq!(shift_held("/", CTRL), Some(Command::Shortcuts));
    assert_eq!(shift_held("1", CTRL), None);
    assert_eq!(shift_held(";", CTRL), None);
}

#[test]
fn on_linux_alt_and_a_digit_goes_to_a_tab() {
    assert_eq!(chord(&pressed("2", ALT), false, Os::Linux, false), Some(("2", false)));
    assert_eq!(on_linux("9", ALT), Some(Command::Tab));
    assert_eq!(on_linux("0", ALT), None);
    assert_eq!(on_linux("@", CTRL), None);
    assert_eq!(on_linux("2", CTRL_SHIFT), None);
}

#[test]
fn on_linux_other_keys_are_left_to_the_program() {
    assert_eq!(on_linux("c", CTRL), None);
    assert_eq!(on_linux("[", CTRL), None);
    assert_eq!(on_linux("b", ALT), None);
    assert_eq!(on_linux("t", CTRL_ALT), None);
    assert_eq!(on_linux("y", CTRL_SHIFT), None);
    assert_eq!(on_linux("i", CTRL_ALT_SHIFT), None);
    assert_eq!(on_linux("v", SUPER), None);
    assert_eq!(on_linux("1", SUPER), None);
    assert_eq!(chord(&pressed("t", CTRL_SHIFT), true, Os::Mac, false), None);
}

#[test]
fn on_linux_usage_takes_shift_since_ctrl_shift_u_types_unicode() {
    assert_eq!(on_linux("u", CTRL_SHIFT), None);
    assert_eq!(on_linux("u", CTRL_ALT_SHIFT), Some(Command::Panel(PanelKind::Usage)));
    assert_eq!(keys_of(Command::Panel(PanelKind::Usage), Os::Linux).as_deref(), Some("Ctrl+Alt+Shift+U"));
}

#[test]
fn no_linux_shortcut_takes_a_key_the_desktop_keeps() {
    let kept = [
        "ctrl-shift-u",
        "ctrl-shift-q",
        "ctrl-shift-m",
        "ctrl-alt-shift-r",
        "ctrl-alt-shift-delete",
        "ctrl-alt-shift-pageup",
        "ctrl-alt-shift-pagedown",
        "ctrl-alt-shift-left",
        "ctrl-alt-shift-right",
        "ctrl-alt-shift-up",
        "ctrl-alt-shift-down",
        "ctrl-alt-shift-tab",
        "ctrl-alt-t",
        "ctrl-alt-l",
        "ctrl-alt-d",
        "ctrl-alt-delete",
        "ctrl-alt-left",
        "ctrl-alt-right",
        "alt-f4",
        "alt-tab",
        "alt-space",
        "alt-f2",
        "alt-f7",
        "alt-f8",
        "alt-f10",
        "alt-`",
        "super-1",
    ];
    for kept in kept {
        let keystroke = Keystroke::parse(kept).expect("a keystroke");
        for browser in [false, true] {
            let taken = shortcut(&keystroke, keystroke.modifiers.shift, Os::Linux, browser).map(|s| s.command);
            assert_eq!(taken, None, "{kept} is the desktop's");
        }
    }
}

#[test]
fn in_a_page_chrome_s_own_keys_reach_the_browser() {
    let answered = [
        ("c", CTRL, Command::Copy),
        ("x", CTRL, Command::Cut),
        ("v", CTRL, Command::Paste),
        ("l", CTRL, Command::FocusAddress),
        ("r", CTRL, Command::Reload),
        ("f5", NONE, Command::Reload),
        ("left", ALT, Command::Back),
        ("right", ALT, Command::Forward),
    ];
    for (key, modifiers, command) in answered {
        assert_eq!(in_a_page(key, modifiers), Some(command), "{key}");
    }
}

#[test]
fn chrome_s_editing_keys_stay_with_the_page_and_its_own_keys_with_it() {
    assert_eq!(in_a_page("a", CTRL), None);
    assert_eq!(in_a_page("z", CTRL), None);
    assert_eq!(on_linux("l", CTRL), None);
    assert_eq!(on_linux("left", ALT), None);
}

#[test]
fn linux_labels_read_as_linux_apps_write_them() {
    let labels = [
        (Command::NewTab, "Ctrl+Shift+T"),
        (Command::Panel(PanelKind::Changes), "Ctrl+Alt+Shift+C"),
        (Command::Tab, "Alt+1…9"),
        (Command::ZoomIn, "Ctrl+Shift+="),
        (Command::PreviousTab, "Ctrl+Alt+Shift+["),
        (Command::Shortcuts, "Ctrl+Shift+/"),
        (Command::Back, "Alt+←"),
        (Command::Reload, "Ctrl+R"),
    ];
    for (command, label) in labels {
        assert_eq!(keys_of(command, Os::Linux).as_deref(), Some(label));
    }
}

#[test]
fn linux_tooltips_and_descriptions_name_linux_keys() {
    assert_eq!(labelled("Changes", Command::Panel(PanelKind::Changes), Os::Linux), "Changes  Ctrl+Alt+Shift+C");
    assert_eq!(Command::Tab.describe(Os::Linux), "Go to a tab (Alt+9 the last)");
    assert_eq!(Command::Tab.describe(Os::Mac), "Go to a tab (⌘9 the last)");
}

#[test]
fn no_linux_label_shows_a_mac_key() {
    let labels = SHORTCUTS.iter().map(|s| s.label(Os::Linux));
    let listed = system_keys(Os::Linux).iter().chain(other_keys(Os::Linux)).map(|(keys, _)| (*keys).to_string());
    for label in labels.chain(listed) {
        assert!(!label.contains(['⌘', '⇧', '⌥', '⌃', '↩']), "{label} is a Mac key");
    }
}

#[test]
fn the_system_keys_follow_the_platform() {
    assert_eq!(system_keys(Os::Mac).last(), Some(&("⌘Q", "Quit")));
    assert_eq!(system_keys(Os::Linux).last(), Some(&("Ctrl+Shift+Q", "Quit")));
    assert!(system_keys(Os::Linux).iter().all(|(_, what)| !what.starts_with("Hide")));
}

#[test]
fn windows_quits_with_alt_f4_and_lists_no_mac_key() {
    assert_eq!(system_keys(Os::Windows).last(), Some(&("Alt+F4", "Quit")));
    assert_eq!(system_keys(Os::Windows).len(), system_keys(Os::Linux).len());
    let labels = SHORTCUTS.iter().map(|s| s.label(Os::Windows));
    let listed = system_keys(Os::Windows).iter().chain(other_keys(Os::Windows)).map(|(keys, _)| (*keys).to_string());
    for label in labels.chain(listed) {
        assert!(!label.contains(['⌘', '⇧', '⌥', '⌃', '↩']), "{label} is a Mac key");
    }
    assert!(other_keys(Os::Windows).iter().all(|(keys, _)| !keys.contains("Middle")));
}

#[test]
fn windows_takes_the_same_keys_as_linux() {
    for s in SHORTCUTS {
        assert_eq!(s.label(Os::Windows), s.label(Os::Linux));
    }
    for (key, modifiers) in [("t", CTRL_SHIFT), ("2", ALT), ("{", CTRL_SHIFT), ("c", CTRL_SHIFT)] {
        let ks = pressed(key, modifiers);
        assert_eq!(chord(&ks, true, Os::Windows, false), chord(&ks, true, Os::Linux, false));
    }
}

#[test]
fn alt_f4_and_alt_space_are_left_to_windows_alone() {
    let alt = Modifiers { alt: true, ..Modifiers::default() };
    assert!(left_to_system(&pressed("f4", alt), Os::Windows));
    assert!(left_to_system(&pressed("space", alt), Os::Windows));
    assert!(!left_to_system(&pressed("space", alt), Os::Linux));
    assert!(!left_to_system(&pressed("f4", alt), Os::Linux));
    assert!(!left_to_system(&pressed("f4", Modifiers { shift: true, ..alt }), Os::Windows));
    assert!(!left_to_system(&pressed("f4", Modifiers::default()), Os::Windows));
}

fn typed(key: &str, modifiers: Modifiers, text: &str) -> Keystroke {
    Keystroke { key_char: Some(text.into()), ..pressed(key, modifiers) }
}

#[test]
fn a_character_alt_gr_types_as_ctrl_alt_is_text_on_windows() {
    assert!(typed_with_alt_gr(&typed("e", CTRL_ALT, "é"), Os::Windows));
    assert!(typed_with_alt_gr(&typed("2", CTRL_ALT, "@"), Os::Windows));
    assert!(!typed_with_alt_gr(&typed("e", CTRL_ALT, "é"), Os::Linux));
    assert!(!typed_with_alt_gr(&typed("e", CTRL_ALT, "e"), Os::Windows));
    assert!(!typed_with_alt_gr(&typed("e", CTRL, "é"), Os::Windows));
    assert!(!typed_with_alt_gr(&pressed("e", CTRL_ALT), Os::Windows));
}

#[test]
fn a_character_alt_gr_types_never_reads_as_a_shortcut_on_windows() {
    let pound = typed("c", CTRL_ALT_SHIFT, "¢");
    assert_eq!(chord(&pound, true, Os::Windows, false), None);
    assert_eq!(chord(&pressed("c", CTRL_ALT_SHIFT), true, Os::Windows, false), Some(("c", true)));
    assert_eq!(chord(&pound, true, Os::Linux, false), Some(("c", true)));
}

#[test]
fn a_link_opens_with_the_system_on_command_or_ctrl_click() {
    assert!(opens_with_system(SUPER, Os::Mac));
    assert!(!opens_with_system(CTRL, Os::Mac));
    assert!(opens_with_system(CTRL, Os::Linux));
    assert!(!opens_with_system(SUPER, Os::Linux));
    assert!(opens_with_system(CTRL, Os::Windows));
    assert!(!opens_with_system(SUPER, Os::Windows));
}

#[test]
fn the_link_key_still_opens_with_shift_held() {
    assert!(opens_with_system(Modifiers { shift: true, ..SUPER }, Os::Mac));
    assert!(opens_with_system(CTRL_SHIFT, Os::Linux));
}
