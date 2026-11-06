use serde_json::json;

use super::{Offers, Run, Spot, listed_offers as offers, target, told};
use crate::model::Shown;
use crate::store::ToastKind;
use crate::test_util::fixture;

fn read_actions(offers: &mut Offers, answer: anyhow::Result<serde_json::Value>) -> anyhow::Result<()> {
    let asked = offers.ask();
    offers.read_actions(asked, answer)
}

fn read_status(offers: &mut Offers, answer: anyhow::Result<serde_json::Value>) -> anyhow::Result<()> {
    let asked = offers.ask();
    offers.read_status(asked, answer)
}

fn titles(runs: &[Run]) -> Vec<&str> {
    runs.iter().map(Run::title).collect()
}

fn listed(contexts: &[&str]) -> Vec<String> {
    contexts.iter().map(ToString::to_string).collect()
}

#[test]
fn a_menu_shows_what_is_offered_for_its_kind_under_each_plugins_name() {
    let groups = offers().offered(Spot::Workspace(3));
    let shown: Vec<(&str, Vec<&str>)> = groups.iter().map(|g| (g.name.as_str(), titles(&g.runs))).collect();
    assert_eq!(shown, [("Notes", vec!["Say where", "Fail on purpose"]), ("Deploy", vec!["Deploy now"])]);
}

#[test]
fn an_action_for_another_kind_stays_out_of_the_menu() {
    let groups = offers().offered(Spot::Project(4));
    let shown: Vec<(&str, Vec<&str>)> = groups.iter().map(|g| (g.name.as_str(), titles(&g.runs))).collect();
    assert_eq!(shown, [("Notes", vec!["Say where"])]);
}

#[test]
fn with_no_plugin_nothing_is_offered() {
    let offers = Offers::default();
    assert!(offers.offered(Spot::Pane(1)).is_empty());
    assert!(offers.buttons().is_empty());
}

#[test]
fn a_failed_read_offers_nothing() {
    let mut offers = offers();
    read_actions(&mut offers, Err(anyhow::anyhow!("no server"))).expect_err("the failure");
    assert!(offers.offered(Spot::Pane(1)).is_empty());
    assert!(offers.buttons().is_empty());
}

#[test]
fn a_failed_read_says_why() {
    let mut offers = Offers::default();
    let actions = read_actions(&mut offers, Err(anyhow::anyhow!("no server"))).expect_err("the failure");
    assert_eq!(actions.to_string(), "no server");
    let status = read_status(&mut offers, Err(anyhow::anyhow!("no answer"))).expect_err("the failure");
    assert_eq!(status.to_string(), "no answer");
}

#[test]
fn an_answer_that_cannot_be_read_is_a_failure() {
    let mut offers = offers();
    read_actions(&mut offers, Ok(json!({"actions": 3}))).expect_err("not a list of actions");
    read_status(&mut offers, Ok(json!({"plugins": 3}))).expect_err("not a list of plugins");
    assert!(offers.offered(Spot::Pane(1)).is_empty());
    assert!(offers.status.is_none());
}

#[test]
fn an_older_read_cannot_replace_the_actions_of_a_newer_one() {
    let mut offers = Offers::default();
    let older = offers.ask();
    let newer = offers.ask();
    offers.read_actions(newer, Ok(json!({"revision": 3, "actions": []}))).expect("the newer answer");
    offers.read_actions(older, Ok(json!({"revision": 2, "actions": []}))).expect("dropped");
    assert_eq!(offers.list.revision, 3);
}

#[test]
fn an_older_read_cannot_replace_the_status_of_a_newer_one() {
    let mut offers = Offers::default();
    let older = offers.ask();
    let newer = offers.ask();
    offers.read_status(newer, Ok(json!({"problem": "the newer"}))).expect("the newer answer");
    offers.read_status(older, Ok(json!({"problem": "the older"}))).expect("dropped");
    assert_eq!(offers.status.and_then(|status| status.problem).as_deref(), Some("the newer"));
}

#[test]
fn a_failure_of_an_older_read_is_not_told_and_changes_nothing() {
    let mut offers = Offers::default();
    let older = offers.ask();
    let newer = offers.ask();
    offers.read_actions(newer, Ok(json!({"revision": 3, "actions": []}))).expect("the newer answer");
    offers.read_actions(older, Err(anyhow::anyhow!("no server"))).expect("dropped");
    assert_eq!(offers.list.revision, 3);
}

#[test]
fn a_run_names_its_plugin_action_revision_and_target() {
    let groups = offers().offered(Spot::Pane(5));
    let command = groups[0].runs[0].command();
    assert_eq!(command, ["plugin", "run", "acme.notes", "say", "--json", "--revision", "2", "--pane", "5"]);
}

#[test]
fn each_kind_of_row_is_named_by_its_own_flag() {
    assert_eq!(Spot::Project(1).args(), ["--project", "1"]);
    assert_eq!(Spot::Workspace(2).args(), ["--workspace", "2"]);
    assert_eq!(Spot::Tab(3).args(), ["--tab", "3"]);
    assert_eq!(Spot::Pane(4).args(), ["--pane", "4"]);
}

#[test]
fn a_buttons_action_runs_with_no_target() {
    let buttons = offers().buttons();
    let command = buttons[1].runs[0].command();
    assert_eq!(command, ["plugin", "run", "acme.deploy", "go", "--json", "--revision", "2"]);
}

#[test]
fn a_plugin_has_one_button_with_its_actions_in_the_manifests_order() {
    let buttons = offers().buttons();
    let shown: Vec<(&str, Vec<&str>)> = buttons.iter().map(|b| (b.title.as_str(), titles(&b.runs))).collect();
    assert_eq!(shown, [("Notes board", vec!["Open board", "Tally"]), ("Deploy", vec!["Deploy now"])]);
}

#[test]
fn a_button_takes_its_icon_from_the_manifest() {
    let buttons = offers().buttons();
    assert_eq!(buttons[0].glyph(), "✎");
    assert_eq!(buttons[1].glyph(), "D");
}

#[test]
fn a_plugin_that_is_off_has_no_button_though_its_status_lists_one() {
    let mut offers = offers();
    let answer = json!({"revision": 3, "actions": [{"plugin": "acme.deploy", "action": "go", "name": "Deploy",
        "title": "Deploy now", "contexts": ["button"], "placement": "job"}]});
    read_actions(&mut offers, Ok(answer)).expect("the actions parse");
    let buttons = offers.buttons();
    assert_eq!(buttons.len(), 1);
    assert_eq!(buttons[0].plugin, "acme.deploy");
}

#[test]
fn without_the_status_a_button_goes_by_the_plugins_name() {
    let mut offers = offers();
    read_status(&mut offers, Err(anyhow::anyhow!("no server"))).expect_err("the failure");
    let buttons = offers.buttons();
    assert_eq!((buttons[0].title.as_str(), buttons[0].glyph().as_str()), ("Notes", "N"));
}

#[test]
fn an_icon_from_a_private_font_gives_way_to_the_titles_initial() {
    let mut offers = offers();
    let status = json!({"plugins": [{"id": "acme.deploy", "button": {"title": "ship it", "icon": "\u{ea60}"}}]});
    read_status(&mut offers, Ok(status)).expect("the status parses");
    let buttons = offers.buttons();
    assert_eq!(buttons[1].glyph(), "S");
}

#[test]
fn a_finished_run_says_so_and_reads_nothing_again() {
    let told = told("Say where", false, Ok(r#"{"plugin": "p", "error": null, "ms": 51}"#.to_string()));
    let (text, kind) = told.said.expect("it says so");
    assert_eq!(text, "Say where: done in 51 ms");
    assert!(kind == ToastKind::Info);
    assert!(!told.refused);
}

#[test]
fn a_failed_run_is_told_from_its_outcome() {
    let told = told("Fail on purpose", false, Ok(fixture!("plugins/run-failed.json").to_string()));
    let (text, kind) = told.said.expect("it says so");
    assert_eq!(text, "Fail on purpose: exit status: 3; `cornercase plugin logs acme.notes` says more");
    assert!(kind == ToastKind::Error);
    assert!(!told.refused);
}

#[test]
fn a_refused_run_is_told_and_reads_the_list_again() {
    let told =
        told("Say where", false, Err(anyhow::anyhow!("cornercase plugin: the plugins changed since revision 1")));
    let (text, kind) = told.said.expect("it says so");
    assert_eq!(text, "Say where: cornercase plugin: the plugins changed since revision 1");
    assert!(kind == ToastKind::Error);
    assert!(told.refused);
}

#[test]
fn a_refused_popup_says_what_it_waits_for() {
    let told = told("Open board", true, Err(anyhow::anyhow!("cornercase plugin: no window is attached")));
    let (text, _) = told.said.expect("it says so");
    assert!(text.starts_with("Open board: cornercase plugin: no window is attached ("), "{text}");
    assert!(text.ends_with("usecornercase/cornercase-terminal#666)"), "{text}");
}

#[test]
fn a_popup_that_opened_adds_nothing_to_its_outcome() {
    let told = told("Open board", true, Ok(r#"{"plugin": "p", "error": null, "ms": 51}"#.to_string()));
    assert_eq!(told.said.expect("it says so").0, "Open board: done in 51 ms");
}

#[test]
fn the_sheet_runs_on_the_most_precise_place_shown() {
    let shown = Shown { project: Some(1), workspace: Some(2), tab: Some(3), pane: Some(4) };
    assert_eq!(target(&listed(&["workspace", "pane"]), shown), Some(Spot::Pane(4)));
    assert_eq!(target(&listed(&["project", "tab"]), shown), Some(Spot::Tab(3)));
    assert_eq!(target(&listed(&["pane", "button"]), shown), Some(Spot::Button));
}

#[test]
fn the_sheet_has_nothing_to_run_on_when_the_window_shows_none_of_its_kinds() {
    let shown = Shown { project: Some(1), workspace: Some(2), tab: Some(3), pane: Some(4) };
    assert_eq!(target(&listed(&["pane"]), Shown::default()), None);
    assert_eq!(target(&[], shown), None);
}
