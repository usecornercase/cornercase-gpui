use super::*;
use crate::app::machine::acts::boot_note;
use crate::context_menu::read_entries;
use crate::model::accounts::Switch;

fn run() -> MachineRun {
    MachineRun { machine: "vps".into(), boot: "b1".into() }
}

fn shell() -> TherePane {
    TherePane { pane: 5, path: Some("/home/ana/shop".into()), ..TherePane::default() }
}

fn others() -> Vec<(u64, String)> {
    vec![(6, "claude (pane 6)".into()), (7, "zsh (pane 7)".into())]
}

fn agent(pending: bool, switch: SwitchThere) -> TherePane {
    TherePane { agent: Some(AgentThere { pending, switch }), ..shell() }
}

fn switch(name: &str, chosen: bool) -> Switch {
    Switch {
        pane: 5,
        agent: "claude".into(),
        folder: name.into(),
        name: name.into(),
        runs: true,
        chosen,
        cancels: false,
    }
}

fn offers() -> SwitchThere {
    SwitchThere::Offers(vec![switch("home", false), switch("work", true)])
}

fn older() -> Option<String> {
    boot_note(Some(Version::new(0, 13, 21)))
}

fn rows(facts: &TherePane) -> Vec<(&'static str, String)> {
    read_entries(&there_pane_menu(&run(), facts))
}

fn row(kind: &'static str, text: &str) -> (&'static str, String) {
    (kind, text.to_string())
}

#[test]
fn a_shell_pane_offers_copy_paste_splits_its_path_and_close() {
    assert_eq!(
        rows(&shell()),
        [
            row("item", "Copy"),
            row("item", "Paste"),
            row("-", ""),
            row("item", "Split right"),
            row("item", "Split down"),
            row("-", ""),
            row("item", "Copy path"),
            row("-", ""),
            row("danger", "Close pane"),
        ]
    );
}

#[test]
fn a_pane_with_others_offers_move_pane_to_each_by_its_name() {
    let facts = TherePane { targets: others(), ..shell() };
    let menu = there_pane_menu(&run(), &facts);
    let Some(Entry::Submenu { entries, .. }) =
        menu.iter().find(|e| matches!(e, Entry::Submenu { label, .. } if label.as_ref() == "Move pane"))
    else {
        panic!("Move pane is offered")
    };
    assert_eq!(read_entries(entries), [row("submenu", "claude (pane 6)"), row("submenu", "zsh (pane 7)")]);
}

#[test]
fn move_pane_rests_with_what_this_computer_needs_when_its_cornercase_is_older() {
    let facts = TherePane { targets: others(), waits: older(), ..shell() };
    let rows = rows(&facts);
    let at = rows.iter().position(|r| *r == row("resting", "Move pane")).expect("Move pane at rest");
    assert_eq!(rows[at + 1], ("note", older().expect("a note")));
}

#[test]
fn a_pane_alone_has_no_move_pane_even_at_rest() {
    let facts = TherePane { waits: older(), ..shell() };
    assert!(rows(&facts).iter().all(|(_, label)| label != "Move pane"));
}

#[test]
fn an_agent_offers_its_mark() {
    assert!(rows(&agent(false, SwitchThere::Nothing)).contains(&row("item", "Mark as pending review")));
}

#[test]
fn an_agent_pending_review_offers_mark_as_reviewed() {
    assert!(rows(&agent(true, SwitchThere::Nothing)).contains(&row("item", "Mark as reviewed")));
}

#[test]
fn an_agents_mark_rests_with_what_this_computer_needs_when_older() {
    let facts = TherePane { waits: older(), ..agent(false, SwitchThere::Nothing) };
    let rows = rows(&facts);
    let at = rows.iter().position(|r| *r == row("resting", "Mark as pending review")).expect("a mark at rest");
    assert_eq!(rows[at + 1], ("note", older().expect("a note")));
}

#[test]
fn an_agent_there_offers_mark_switch_view_context_and_close_in_the_local_order() {
    let facts = TherePane { targets: others(), ..agent(false, offers()) };
    assert_eq!(
        rows(&facts),
        [
            row("item", "Copy"),
            row("item", "Paste"),
            row("-", ""),
            row("item", "Split right"),
            row("item", "Split down"),
            row("submenu", "Move pane"),
            row("-", ""),
            row("item", "Mark as pending review"),
            row("submenu", "Switch account"),
            row("item", "View context"),
            row("-", ""),
            row("danger", "Close agent…"),
            row("-", ""),
            row("item", "Copy path"),
            row("-", ""),
            row("danger", "Close pane"),
        ]
    );
}

#[test]
fn switch_account_there_lists_the_machines_accounts_and_no_manage_accounts() {
    let menu = there_pane_menu(&run(), &agent(false, offers()));
    let Some(Entry::Submenu { entries, .. }) =
        menu.iter().find(|e| matches!(e, Entry::Submenu { label, .. } if label.as_ref() == "Switch account"))
    else {
        panic!("Switch account is offered")
    };
    assert_eq!(read_entries(entries), [row("item", "home"), row("item", "work")]);
}

#[test]
fn switch_account_rests_with_its_note() {
    let rests = SwitchThere::Rests("Asking vps for its accounts…".into());
    let rows = rows(&agent(false, rests));
    let at = rows.iter().position(|r| *r == row("resting", "Switch account")).expect("a resting entry");
    assert_eq!(rows[at + 1], row("note", "Asking vps for its accounts…"));
}

#[test]
fn an_agent_with_no_accounts_there_has_no_switch_account() {
    assert!(rows(&agent(false, SwitchThere::Nothing)).iter().all(|(_, label)| label != "Switch account"));
}

#[test]
fn an_agents_menu_there_offers_no_fold() {
    let rows = rows(&agent(false, SwitchThere::Nothing));
    assert!(rows.iter().all(|(_, label)| label != "Collapse" && label != "Expand"));
}

#[test]
fn a_pane_without_a_path_has_no_copy_path() {
    let rows = rows(&TherePane { path: None, ..shell() });
    assert!(!rows.contains(&row("item", "Copy path")));
    assert!(rows.windows(2).all(|pair| !(pair[0].0 == "-" && pair[1].0 == "-")));
}

#[test]
fn a_shell_has_no_agent_entries() {
    let rows = rows(&shell());
    assert!(rows.iter().all(|(_, label)| label != "View context" && label != "Close agent…"));
}

#[test]
fn nothing_this_server_does_is_offered_there() {
    let facts = TherePane { targets: others(), ..agent(false, offers()) };
    for (_, label) in rows(&facts) {
        assert!(!label.starts_with("Open in"), "{label}");
        assert!(label != "Send the right click to the program", "{label}");
        assert!(label != "Manage accounts…", "{label}");
    }
}
