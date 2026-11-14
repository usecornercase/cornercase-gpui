use super::{Spot, behind_gear, gear_entries, gear_lit, spot};
use crate::context_menu::{Entry, read_entries};
use crate::panels::PanelKind;
use crate::platform::Os;
use crate::sidebar::rail::{SHEETS, Slot, column};

const BEHIND: [PanelKind; 8] = [
    PanelKind::Settings,
    PanelKind::Accounts,
    PanelKind::Usage,
    PanelKind::Machines,
    PanelKind::Worktrees,
    PanelKind::Processes,
    PanelKind::Ports,
    PanelKind::Plugins,
];

fn keys(os: Os) -> Vec<String> {
    gear_entries(os)
        .iter()
        .filter_map(|entry| match entry {
            Entry::Item { keys, .. } => keys.as_ref().map(ToString::to_string),
            _ => None,
        })
        .collect()
}

#[test]
fn every_panel_has_one_place_on_the_rail_or_behind_the_gear() {
    let rail = column(true);
    for kind in PanelKind::ALL {
        let places = [rail.contains(&Slot::Panel(kind)), behind_gear(kind)];
        assert_eq!(places.iter().filter(|here| **here).count(), 1, "{kind:?}");
    }
    assert!(SHEETS.iter().all(|kind| !behind_gear(*kind)));
}

#[test]
fn the_gears_menu_lists_settings_then_two_groups() {
    let labels: Vec<String> = read_entries(&gear_entries(Os::Mac)).into_iter().map(|(_, label)| label).collect();
    let expected = ["Settings", "", "Accounts", "Usage", "Machines", "", "Worktrees", "Processes", "Ports", "Plugins"];
    assert_eq!(labels, expected);
    let kinds: Vec<&str> = read_entries(&gear_entries(Os::Mac)).into_iter().map(|(kind, _)| kind).collect();
    assert_eq!(kinds.iter().filter(|kind| **kind == "-").count(), 2);
}

#[test]
fn each_entry_shows_its_panels_icon() {
    let icons: Vec<_> = gear_entries(Os::Mac)
        .iter()
        .filter_map(|entry| match entry {
            Entry::Item { icon, .. } => *icon,
            _ => None,
        })
        .collect();
    let expected: Vec<_> = BEHIND.iter().map(|kind| kind.icon()).collect();
    assert_eq!(icons, expected);
}

#[test]
fn each_entry_shows_its_keys_as_a_mac_spells_them() {
    assert_eq!(keys(Os::Mac), ["⌘,", "⇧⌘A", "⌘U", "⇧⌘N", "⇧⌘T", "⇧⌘M", "⇧⌘O", "⇧⌘X"]);
}

#[test]
fn on_linux_each_entry_shows_the_ctrl_keys() {
    assert_eq!(
        keys(Os::Linux),
        [
            "Ctrl+Shift+,",
            "Ctrl+Alt+Shift+A",
            "Ctrl+Alt+Shift+U",
            "Ctrl+Alt+Shift+N",
            "Ctrl+Alt+Shift+T",
            "Ctrl+Alt+Shift+M",
            "Ctrl+Alt+Shift+O",
            "Ctrl+Alt+Shift+X",
        ]
    );
}

#[test]
fn windows_shows_the_keys_linux_does() {
    assert_eq!(keys(Os::Windows), keys(Os::Linux));
}

#[test]
fn the_gear_is_lit_while_settings_or_a_sheet_behind_it_is_open() {
    for kind in BEHIND {
        assert!(gear_lit(Some(kind)), "{kind:?}");
    }
}

#[test]
fn a_sheet_on_the_rail_or_none_leaves_the_gear_unlit() {
    assert!(!gear_lit(None));
    for kind in SHEETS {
        assert!(!gear_lit(Some(kind)), "{kind:?}");
    }
}

#[test]
fn a_panel_on_the_rail_is_ringed_on_its_own_icon() {
    for kind in [PanelKind::Changes, PanelKind::Search, PanelKind::Flows] {
        assert_eq!(spot(kind), Spot::Icon(kind));
    }
}

#[test]
fn a_panel_behind_the_gear_rings_the_gear() {
    for kind in BEHIND {
        assert_eq!(spot(kind), Spot::Gear, "{kind:?}");
    }
}
