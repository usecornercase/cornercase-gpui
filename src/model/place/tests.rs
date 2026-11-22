use super::*;
use crate::model::ThereWorkspace;

fn here(id: &str) -> BotKey {
    BotKey::here(id)
}

fn vps() -> Home {
    Home::Machine("vps".into())
}

fn place() -> MachinePlace {
    let seen = crate::model::MachineSeen::default();
    MachinePlace::at(&seen, &ThereWorkspace::default(), None)
}

#[test]
fn showing_a_bot_leaves_a_machines_place() {
    let shown = Showing::Machine(place()).after(Moved::Bot(here("prs")));
    assert_eq!((shown.bot(), shown.machine()), (Some(&here("prs")), None));
}

#[test]
fn picking_a_machines_place_leaves_the_chat() {
    assert_eq!(Showing::Bot(here("prs")).after(Moved::Machine(place())).bot(), None);
}

#[test]
fn a_move_to_a_local_place_leaves_the_chat() {
    assert_eq!(Showing::Bot(here("prs")).after(Moved::Local), Showing::Local);
}

#[test]
fn the_removal_of_the_shown_bot_leaves_its_chat_for_the_bots_page() {
    assert_eq!(Showing::Bot(here("prs")).after(Moved::Removed(here("prs"))), Showing::Bots);
}

#[test]
fn the_removal_of_another_bot_keeps_the_chat() {
    assert_eq!(Showing::Bot(here("prs")).after(Moved::Removed(here("ops"))).bot(), Some(&here("prs")));
}

#[test]
fn a_list_without_the_shown_bot_leaves_its_chat_for_the_bots_page() {
    let shown = Showing::Bot(here("prs"));
    assert_eq!(shown.clone().after(Moved::Listed(Home::Here, vec!["ops".into()])), Showing::Bots);
    assert_eq!(shown.after(Moved::Listed(Home::Here, vec!["prs".into()])).bot(), Some(&here("prs")));
}

#[test]
fn a_list_of_one_home_that_lacks_the_id_keeps_a_chat_of_another_home() {
    let shown = Showing::Bot(here("prs"));
    assert_eq!(shown.clone().after(Moved::Listed(vps(), Vec::new())), shown);
    let there = Showing::Bot(BotKey::on("vps", "prs"));
    assert_eq!(there.clone().after(Moved::Listed(Home::Here, Vec::new())), there);
    assert_eq!(there.after(Moved::Listed(vps(), Vec::new())), Showing::Bots);
}

#[test]
fn a_removed_bot_on_a_machine_keeps_the_chat_of_the_same_id_here() {
    let shown = Showing::Bot(here("prs"));
    assert_eq!(shown.clone().after(Moved::Removed(BotKey::on("vps", "prs"))), shown);
}

#[test]
fn the_bots_page_shows_with_or_without_a_chat() {
    assert_eq!((Showing::Bots.bots(), Showing::Bot(here("prs")).bots()), (true, true));
    assert_eq!((Showing::Local.bots(), Showing::Machine(place()).bots()), (false, false));
}

#[test]
fn opening_the_bots_page_leaves_the_panes_and_a_chat() {
    assert_eq!(Showing::Bot(here("prs")).after(Moved::Bots), Showing::Bots);
    assert_eq!(Showing::Machine(place()).after(Moved::Bots), Showing::Bots);
}

#[test]
fn a_move_to_a_local_place_leaves_the_bots_page() {
    assert_eq!(Showing::Bots.after(Moved::Local), Showing::Local);
}

#[test]
fn a_list_or_a_removal_keeps_the_bots_page() {
    assert_eq!(Showing::Bots.after(Moved::Removed(here("prs"))), Showing::Bots);
    assert_eq!(Showing::Bots.after(Moved::Listed(Home::Here, Vec::new())), Showing::Bots);
}

#[test]
fn a_start_whose_dialog_is_still_shown_at_the_answer_is_awaited() {
    assert!(awaits(true, Some(7), Some(7)));
}

#[test]
fn a_start_whose_dialog_was_closed_before_the_answer_is_not_awaited() {
    assert_eq!((awaits(true, Some(7), None), awaits(true, Some(7), Some(8))), (false, false));
}

#[test]
fn a_start_that_did_not_ask_to_be_selected_is_not_awaited() {
    assert!(!awaits(false, Some(7), Some(7)));
}

#[test]
fn a_start_no_dialog_sent_is_not_awaited() {
    assert!(!awaits(true, None, None));
}
