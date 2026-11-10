use super::*;
use crate::context_menu::read_entries;

fn run() -> MachineRun {
    MachineRun { machine: "vps".into(), boot: "b1".into() }
}

fn shell() -> TherePane {
    TherePane { pane: 5, path: Some("/home/ana/shop".into()), ..TherePane::default() }
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
fn a_pane_alone_has_no_move_and_one_with_others_rests_it_with_the_issue() {
    assert!(!rows(&shell()).contains(&row("resting", "Move pane")));
    let rows = rows(&TherePane { siblings: true, ..shell() });
    let at = rows.iter().position(|r| *r == row("resting", "Move pane")).expect("Move pane at rest");
    assert_eq!(rows[at + 1], row("note", BOOT_WAITS));
}

#[test]
fn an_agent_rests_its_mark_with_the_issue() {
    let agent = |pending| TherePane { agent: Some(AgentThere { pending, ..AgentThere::default() }), ..shell() };
    let rows_of = |pending| rows(&agent(pending));
    let at = rows_of(false).iter().position(|r| *r == row("resting", "Mark as pending review")).expect("a mark");
    assert_eq!(rows_of(false)[at + 1], row("note", BOOT_WAITS));
    assert!(rows_of(true).contains(&row("resting", "Mark as reviewed")));
}

#[test]
fn an_agents_menu_there_offers_no_fold() {
    let rows = rows(&TherePane { agent: Some(AgentThere::default()), ..shell() });
    assert!(rows.iter().all(|(_, label)| label != "Collapse" && label != "Expand"));
}

#[test]
fn a_pane_without_a_path_has_no_copy_path() {
    let rows = rows(&TherePane { path: None, ..shell() });
    assert!(!rows.contains(&row("item", "Copy path")));
    assert!(rows.windows(2).all(|pair| !(pair[0].0 == "-" && pair[1].0 == "-")));
}

fn on_account() -> TherePane {
    TherePane { agent: Some(AgentThere { pending: false, on_account: true }), ..shell() }
}

#[test]
fn an_agent_there_offers_view_context_and_close_agent() {
    let agent = TherePane { agent: Some(AgentThere::default()), ..shell() };
    assert_eq!(
        rows(&agent),
        [
            row("item", "Copy"),
            row("item", "Paste"),
            row("-", ""),
            row("item", "Split right"),
            row("item", "Split down"),
            row("-", ""),
            row("resting", "Mark as pending review"),
            row("note", BOOT_WAITS),
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
fn an_agent_on_an_account_rests_switch_account_with_its_issue() {
    let rows = rows(&on_account());
    let at = rows.iter().position(|r| *r == row("note", BOOT_WAITS)).expect("the mark's note");
    assert_eq!(rows[at + 1], row("resting", "Switch account"));
    assert_eq!(rows[at + 2], row("note", RESTART_WAITS));
}

#[test]
fn an_agent_on_its_own_login_has_no_switch_account() {
    let agent = TherePane { agent: Some(AgentThere::default()), ..shell() };
    assert!(rows(&agent).iter().all(|(_, label)| label != "Switch account"));
}

#[test]
fn a_shell_has_no_agent_entries() {
    let rows = rows(&shell());
    assert!(rows.iter().all(|(_, label)| label != "View context" && label != "Close agent…"));
}

#[test]
fn nothing_this_server_does_is_offered_there() {
    let agent = TherePane { siblings: true, ..on_account() };
    for (kind, label) in rows(&agent) {
        assert!(!label.starts_with("Open in"), "{label}");
        assert!(label != "Send the right click to the program", "{label}");
        assert!(!label.starts_with("Switch account") || kind == "resting", "{label}");
    }
}
