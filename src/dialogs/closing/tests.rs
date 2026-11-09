use super::*;

fn pane(agent: Option<&str>, program: Option<&str>) -> PaneInfo {
    PaneInfo { id: 5, agent: agent.map(Into::into), program: program.map(Into::into), ..PaneInfo::default() }
}

fn shop() -> Tab {
    Tab { id: 20, name: "shop".into(), ..Tab::default() }
}

#[test]
fn closing_a_pane_here_keeps_its_words() {
    let words = CloseWords::pane(Some(&pane(Some("claude"), None)), None);
    assert_eq!(words.title, "Close this pane?");
    assert_eq!(words.message, "It runs claude: closing the pane stops it.");
    assert_eq!((words.button, words.doing.as_str()), ("Close pane", "Closing claude…"));
}

#[test]
fn closing_a_pane_there_names_the_machine() {
    let words = CloseWords::pane(Some(&pane(Some("claude"), None)), Some("vps"));
    assert_eq!(words.title, "Close this pane on vps?");
    assert_eq!(words.message, "It runs claude: closing the pane stops it.");
    assert_eq!(words.doing, "Closing claude on vps…");
}

#[test]
fn a_pane_with_no_agent_is_named_by_its_program_else_its_shell() {
    assert_eq!(CloseWords::pane(Some(&pane(None, Some("vim"))), None).doing, "Closing vim…");
    assert_eq!(CloseWords::pane(Some(&pane(None, None)), None).doing, "Closing a shell…");
    assert_eq!(CloseWords::pane(None, Some("vps")).doing, "Closing a shell on vps…");
}

#[test]
fn closing_a_tab_here_keeps_its_words() {
    let agent = PaneInfo { status: Some("waiting".into()), ..pane(Some("claude"), None) };
    let words = CloseWords::tab(&shop(), Some(&agent), None);
    assert_eq!(words.title, "Close the tab shop?");
    assert_eq!(words.message, "It runs claude (waiting): closing the tab stops it. Its worktree and branch stay.");
    assert_eq!((words.button, words.doing.as_str()), ("Close tab", "Closing shop…"));
}

#[test]
fn closing_a_tab_there_names_the_machine_and_where_the_worktree_stays() {
    let agent = PaneInfo { status: Some("waiting".into()), ..pane(Some("claude"), None) };
    let words = CloseWords::tab(&shop(), Some(&agent), Some("vps"));
    assert_eq!(words.title, "Close the tab shop on vps?");
    assert_eq!(
        words.message,
        "It runs claude (waiting): closing the tab stops it. Its worktree and branch stay there."
    );
    assert_eq!(words.doing, "Closing shop on vps…");
}

#[test]
fn an_unnamed_agent_without_a_status_reads_as_an_agent() {
    let words = CloseWords::tab(&shop(), Some(&pane(None, None)), None);
    assert_eq!(words.message, "It runs an agent: closing the tab stops it. Its worktree and branch stay.");
}
