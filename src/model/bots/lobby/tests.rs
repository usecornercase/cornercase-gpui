use super::*;

const OLD: &str = "Making a bot needs cornercase 0.13.77 or newer.";

#[test]
fn a_server_with_no_bot_that_can_make_one_shows_the_welcome() {
    let lobby = lobby_of(&Offer::Hidden, Some(&Making::Offered), true);
    assert_eq!(lobby, Lobby::Empty { sets_itself_up: true });
    assert!(lobby.makes() && lobby.ideas());
}

#[test]
fn a_server_whose_bots_cannot_set_themselves_up_offers_no_ideas() {
    let lobby = lobby_of(&Offer::Hidden, Some(&Making::Offered), false);
    assert!(lobby.makes() && !lobby.ideas());
    assert!(!lobby.lines().concat().contains("schedules"));
}

#[test]
fn listed_bots_ask_to_pick_one() {
    let lobby = lobby_of(&Offer::Rows, Some(&Making::Offered), true);
    assert_eq!(lobby, Lobby::Pick);
    assert!(lobby.makes() && !lobby.ideas());
}

#[test]
fn an_older_server_says_what_it_needs_and_offers_nothing_to_make() {
    let lobby = lobby_of(&Offer::Hidden, Some(&Making::Server(OLD.into())), true);
    assert_eq!(lobby, Lobby::Lacks(OLD.into()));
    assert!(!lobby.makes() && lobby.lines().contains(&OLD.to_string()));
}

#[test]
fn bots_this_computer_cannot_show_say_so() {
    let words = "Showing your bots needs cornercase 0.13.77 or newer on this Mac.";
    assert_eq!(lobby_of(&Offer::Lacks(words.into()), Some(&Making::Offered), true), Lobby::Lacks(words.into()));
}

#[test]
fn each_idea_has_a_short_name_and_says_when_it_works() {
    for (name, about) in IDEAS {
        assert!(name.len() <= 40 && about.starts_with("Every"), "{name}: {about}");
    }
}
