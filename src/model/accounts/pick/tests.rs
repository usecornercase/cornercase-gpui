use super::*;
use crate::test_util::fixture;

fn listed() -> AccountsInfo {
    serde_json::from_str(fixture!("accounts/pickable.json")).expect("a list")
}

fn claude() -> AgentChoices {
    listed().choices().into_iter().find(|c| c.agent == "claude").expect("claude's accounts")
}

fn folders(choices: &AgentChoices) -> Vec<&str> {
    choices.accounts.iter().map(|a| a.folder.as_str()).collect()
}

fn pick(folder: &str) -> AccountPick {
    AccountPick { agent: "claude".into(), folder: folder.into() }
}

#[test]
fn only_an_agent_with_accounts_offers_a_choice() {
    let agents: Vec<String> = listed().choices().into_iter().map(|c| c.agent).collect();
    assert_eq!(agents, ["claude", "codex"]);
}

#[test]
fn a_server_without_accounts_offers_nothing() {
    let mut info = listed();
    info.providers.iter_mut().for_each(|p| p.accounts.clear());
    assert_eq!(info.choices(), Vec::<AgentChoices>::new());
}

#[test]
fn the_own_login_comes_first_and_labels_name_the_accounts() {
    let choices = claude();
    assert_eq!(folders(&choices), ["", "personal", "work"]);
    let names: Vec<&str> = choices.accounts.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["System", "Home", "work"]);
    assert_eq!(choices.title, "Claude Code");
}

#[test]
fn the_one_in_use_is_the_active_account() {
    assert_eq!(claude().in_use, "work");
}

#[test]
fn the_own_login_is_in_use_when_nothing_is_active() {
    let codex = listed().choices().into_iter().find(|c| c.agent == "codex").expect("codex's accounts");
    assert_eq!(codex.in_use, "");
}

#[test]
fn an_active_account_that_is_gone_leaves_the_own_login_in_use() {
    let mut info = listed();
    info.providers[0].active = "removed".into();
    assert_eq!(info.choices()[0].in_use, "");
}

#[test]
fn the_setting_turned_off_offers_nothing() {
    let settings = AccountSettings { picker: false, ..Default::default() };
    assert_eq!(settings.offered(&listed()), Vec::<AgentChoices>::new());
}

#[test]
fn the_setting_on_offers_the_list_it_is_given() {
    let settings = AccountSettings { picker: true, ..Default::default() };
    assert_eq!(settings.offered(&listed()), listed().choices());
}

#[test]
fn nobody_who_does_not_pick_sends_anything() {
    assert_eq!(Picking::new(claude(), None).pick(), None);
}

#[test]
fn picking_another_account_sends_it() {
    let mut picking = Picking::new(claude(), None);
    picking.choose("personal");
    assert_eq!(picking.pick(), Some(pick("personal")));
}

#[test]
fn picking_the_own_login_sends_an_empty_account() {
    let mut picking = Picking::new(claude(), None);
    picking.choose("");
    assert_eq!(picking.pick(), Some(pick("")));
}

#[test]
fn choosing_the_one_in_use_again_sends_nothing() {
    let mut picking = Picking::new(claude(), None);
    picking.choose("personal");
    picking.choose("work");
    assert_eq!(picking.pick(), None);
}

#[test]
fn a_saved_account_stays_without_being_sent_again() {
    assert_eq!(Picking::new(claude(), Some("personal")).pick(), None);
}

#[test]
fn the_one_in_use_replaces_a_saved_account_when_picked() {
    let mut picking = Picking::new(claude(), Some("personal"));
    picking.choose("work");
    assert_eq!(picking.pick(), Some(pick("work")));
}

#[test]
fn a_new_default_does_not_become_a_pick() {
    let mut picking = Picking::new(claude(), None);
    picking.follow(AgentChoices { in_use: "personal".into(), ..claude() });
    assert_eq!(picking.pick(), None);
    assert_eq!(picking.shown().name, "Home");
}

#[test]
fn a_pick_stays_when_the_default_moves_onto_it_and_away() {
    let mut picking = Picking::new(claude(), None);
    picking.choose("personal");
    picking.follow(AgentChoices { in_use: "personal".into(), ..claude() });
    assert_eq!(picking.pick(), None);
    picking.follow(claude());
    assert_eq!(picking.pick(), Some(pick("personal")));
}

#[test]
fn the_list_says_whether_it_changed() {
    let mut picking = Picking::new(claude(), None);
    assert!(!picking.follow(claude()));
    assert!(picking.follow(AgentChoices { in_use: String::new(), ..claude() }));
}

#[test]
fn the_rows_name_the_one_in_use_and_the_own_login() {
    let rows = Picking::new(claude(), None).rows();
    let notes: Vec<&str> = rows.iter().map(|r| r.note.as_str()).collect();
    assert_eq!(notes, ["own login", "", "in use"]);
    let chosen: Vec<&str> = rows.iter().filter(|r| r.chosen).map(|r| r.name.as_str()).collect();
    assert_eq!(chosen, ["work"]);
}

#[test]
fn the_own_login_in_use_says_both() {
    let picking = Picking::new(AgentChoices { in_use: String::new(), ..claude() }, None);
    assert_eq!(picking.shown().note, "own login · in use");
}

#[test]
fn the_dropdown_starts_on_the_one_in_use() {
    let picking = Picking::new(claude(), None);
    assert_eq!(picking.shown().name, "work");
    assert_eq!(picking.shown().note, "in use");
    assert_eq!(picking.chosen_at(), Some(2));
}

#[test]
fn a_saved_account_is_what_the_dropdown_starts_on() {
    let picking = Picking::new(claude(), Some("personal"));
    assert_eq!(picking.shown().name, "Home");
    assert_eq!(picking.shown().note, "");
}

#[test]
fn a_saved_account_that_is_gone_says_so_and_is_not_replaced() {
    let picking = Picking::new(claude(), Some("home"));
    assert_eq!(picking.shown().name, "home (gone)");
    assert_eq!(picking.gone().as_deref(), Some("The claude account home is gone: pick another account"));
    assert_eq!(picking.pick(), None);
    assert_eq!(picking.chosen_at(), None);
}

#[test]
fn another_pick_settles_an_account_that_is_gone() {
    let mut picking = Picking::new(claude(), Some("home"));
    picking.choose("work");
    assert_eq!(picking.gone(), None);
    assert_eq!(picking.pick(), Some(pick("work")));
}

#[test]
fn a_pick_removed_meanwhile_is_still_sent_for_the_server_to_refuse() {
    let mut picking = Picking::new(claude(), None);
    picking.choose("personal");
    let without = claude().accounts.into_iter().filter(|a| a.folder != "personal").collect();
    picking.follow(AgentChoices { accounts: without, ..claude() });
    assert_eq!(picking.shown().name, "personal (gone)");
    assert_eq!(picking.pick(), Some(pick("personal")));
}

#[test]
fn only_a_form_left_on_the_one_in_use_follows_it() {
    let mut picking = Picking::new(claude(), None);
    assert!(picking.follows_in_use());
    picking.choose("personal");
    assert!(!picking.follows_in_use());
    assert!(!Picking::new(claude(), Some("work")).follows_in_use());
}

#[test]
fn the_arguments_name_the_agent_and_its_account() {
    let picks = [
        AccountPick { agent: "claude".into(), folder: "work".into() },
        AccountPick { agent: "codex".into(), folder: String::new() },
    ];
    assert_eq!(account_args(&picks), ["--account", "claude=work", "--account", "codex="]);
}

#[test]
fn no_pick_adds_no_argument() {
    assert_eq!(account_args(&[]), Vec::<String>::new());
}
