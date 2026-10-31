use serde_json::json;

use super::*;

fn event() -> Event {
    Event {
        session: Some("S".into()),
        method: "Runtime.bindingCalled".into(),
        params: json!({ "executionContextId": 7 }),
    }
}

fn fruit() -> Choice {
    let message = json!({
        "kind": "select",
        "options": [
            { "label": "Apple", "disabled": false, "group": "" },
            { "label": "Banana", "disabled": true, "group": "Yellow" },
            { "label": "Lemon", "disabled": false, "group": "Yellow" },
            { "label": "lime", "disabled": false, "group": "Green" },
        ],
        "selected": 0,
        "x": 10, "y": 20, "w": 120, "h": 24,
    });
    Choice::read(&message, &event()).expect("a choice")
}

#[test]
fn a_select_message_becomes_a_choice_from_its_page() {
    let choice = fruit();
    assert_eq!(choice.items.len(), 4);
    assert_eq!((choice.selected, choice.highlight), (Some(0), Some(0)));
    assert_eq!(choice.rect, Rect { x: 10., y: 20., w: 120., h: 24. });
    assert_eq!((choice.session.as_deref(), choice.context), (Some("S"), Some(7)));
}

#[test]
fn a_selected_index_past_the_options_is_none() {
    let message = json!({ "options": [{ "label": "One" }], "selected": 4, "x": 0, "y": 0 });
    assert_eq!(Choice::read(&message, &event()).expect("a choice").selected, None);
}

#[test]
fn stepping_skips_disabled_options() {
    let mut choice = fruit();
    choice.step(true);
    assert_eq!(choice.highlight, Some(2));
    choice.step(false);
    assert_eq!(choice.highlight, Some(0));
}

#[test]
fn stepping_stops_at_the_ends() {
    let mut choice = fruit();
    choice.step(false);
    assert_eq!(choice.highlight, Some(0));
    choice.highlight = Some(3);
    choice.step(true);
    assert_eq!(choice.highlight, Some(3));
}

#[test]
fn typing_a_letter_jumps_to_the_next_option_starting_with_it() {
    let mut choice = fruit();
    choice.seek('L');
    assert_eq!(choice.highlight, Some(2));
    choice.seek('l');
    assert_eq!(choice.highlight, Some(3));
    choice.seek('l');
    assert_eq!(choice.highlight, Some(2));
}

#[test]
fn groups_head_their_options() {
    let choice = fruit();
    assert_eq!(
        choice.rows(),
        [Row::Item(0), Row::Group("Yellow"), Row::Item(1), Row::Item(2), Row::Group("Green"), Row::Item(3)]
    );
}

#[test]
fn choosing_runs_in_the_frame_that_asked() {
    let script = choose_script(Some(2), Some(7));
    assert_eq!(script, json!({ "expression": "window.__cornercase && window.__cornercase.choose(2)", "contextId": 7 }));
}

#[test]
fn closing_without_a_choice_tells_the_page_minus_one() {
    assert_eq!(choose_script(None, None)["expression"], "window.__cornercase && window.__cornercase.choose(-1)");
}
