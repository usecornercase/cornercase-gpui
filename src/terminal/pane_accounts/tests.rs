use super::*;
use crate::render::layout::{PADDING, rect};

fn switcher(agent: &str, name: &str) -> Switcher {
    Switcher {
        pane: 1,
        agent: agent.into(),
        title: agent.into(),
        folder: name.into(),
        name: name.into(),
        runs: false,
        waiting: false,
    }
}

fn both() -> Vec<Switcher> {
    vec![switcher("claude", "work"), switcher("codex", "System")]
}

#[test]
fn chips_name_their_agent_while_they_all_fit() {
    assert_eq!(chip_texts(&both(), 40), ["claude: work", "codex: System"]);
}

#[test]
fn chips_leave_their_agent_out_when_they_do_not_all_fit() {
    assert_eq!(chip_texts(&both(), 20), ["work", "System"]);
}

#[test]
fn the_last_chips_go_when_even_the_names_do_not_fit() {
    assert_eq!(chip_texts(&both(), 10), ["work"]);
}

#[test]
fn a_name_too_long_for_the_card_is_cut() {
    let long = [switcher("claude", "a-very-long-account")];
    assert_eq!(chip_texts(&long, 10), ["a-ver…"]);
}

#[test]
fn a_card_with_no_room_for_a_name_shows_no_chip() {
    assert_eq!(chip_texts(&both(), 6), Vec::<String>::new());
}

#[test]
fn a_waiting_restart_shows_on_its_chip() {
    let waiting = [Switcher { waiting: true, ..switcher("claude", "work") }];
    assert_eq!(chip_texts(&waiting, 60), ["claude: work · restarts after this turn"]);
}

#[test]
fn chips_sit_in_the_cards_bottom_padding_at_its_right() {
    let at = chips_origin(rect(100., 50., 400., 300.), gpui::size(px(80.), px(HEIGHT)));
    assert_eq!(at, point(px(100. + 400. - SIDE - 80.), px(50. + 300. - LIFT - HEIGHT)));
    const { assert!(HEIGHT + LIFT - PADDING <= 1., "a chip must not cover the text of the pane's last row") };
}

#[test]
fn a_cards_room_counts_its_characters_inside_the_sides() {
    let metrics = Metrics { family: "Mono".into(), font_size: px(16.), cell_width: px(8.), line_height: px(20.) };
    assert_eq!(room_in(rect(0., 0., 324., 100.), &metrics), 60);
}
