use serde_json::{Value, json};

use super::*;
use crate::cli::Version;
use crate::model::bots::in_app;
use crate::test_util::fixture;

fn listed_bots() -> Vec<Bot> {
    in_app(&serde_json::from_str(fixture!("bots/app-asks.json")).expect("fixture"))
}

fn ask(id: u64, names: &[&str], missing: &[&str]) -> Ask {
    let owned = |list: &[&str]| list.iter().map(|name| (*name).to_string()).collect();
    Ask { id, names: owned(names), missing: owned(missing), ..Ask::default() }
}

fn bot_with(id: &str, asks: Vec<Ask>) -> Bot {
    Bot { id: id.to_string(), asks, ..Bot::default() }
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| (*name).to_string()).collect()
}

fn typed(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs.iter().map(|(name, value)| ((*name).to_string(), (*value).to_string())).collect()
}

fn asks_of(value: &Value) -> Vec<Ask> {
    in_app(&json!({ "bots": [{ "id": "b", "asks": value }] })).remove(0).asks
}

#[test]
fn an_app_line_lists_a_bots_asks() {
    let campus = listed_bots().remove(0);
    assert_eq!(
        campus.asks,
        [
            Ask {
                id: 3,
                names: names(&["campus-user", "campus-password"]),
                about: "to read your grades on the campus site".into(),
                replace: false,
                routine: None,
                missing: names(&["campus-password"]),
            },
            Ask {
                id: 5,
                names: names(&["campus-token"]),
                about: "to replace the token that expired".into(),
                replace: true,
                routine: Some(12),
                missing: names(&["campus-token"]),
            },
        ]
    );
}

#[test]
fn a_missing_or_wrong_asks_is_none() {
    for value in [json!(null), json!("none"), json!(3), json!({ "id": 1 })] {
        assert_eq!(asks_of(&value), Vec::new(), "{value}");
    }
    let absent = in_app(&json!({ "bots": [{ "id": "b" }] }));
    assert_eq!((absent.len(), absent[0].asks.len()), (1, 0));
    let bots = listed_bots();
    assert_eq!(bots.iter().map(|bot| bot.id.as_str()).collect::<Vec<_>>(), ["campus", "ops", "mail"]);
    assert_eq!(bots[1].asks, Vec::new());
}

#[test]
fn a_bad_ask_leaves_the_others() {
    let mail = listed_bots().remove(2);
    assert_eq!(mail.asks.iter().map(|ask| ask.id).collect::<Vec<_>>(), [9]);
    assert_eq!(
        asks_of(&json!([{ "id": 0, "names": ["a"] }, { "id": 2, "names": [] }, { "id": 4, "names": ["a"] }])).len(),
        1
    );
}

#[test]
fn the_fields_are_the_missing_names() {
    assert_eq!(ask(3, &["user", "password"], &["password"]).fields(), names(&["password"]));
}

#[test]
fn without_missing_the_fields_are_the_names() {
    assert_eq!(ask(3, &["user", "password"], &[]).fields(), names(&["user", "password"]));
    assert_eq!(ask(3, &["user", "password"], &[]).joined(), "user, password");
}

#[test]
fn a_field_stays_when_its_name_is_saved_elsewhere() {
    let now = ask(3, &["user", "password"], &["password"]);
    assert_eq!(fields_after(&names(&["user", "password"]), &now), names(&["user", "password"]));
}

#[test]
fn a_removed_secret_gets_its_field_back_in_the_asks_order() {
    let now = ask(3, &["user", "password", "token"], &["user", "token"]);
    assert_eq!(fields_after(&names(&["token"]), &now), names(&["user", "token"]));
    assert_eq!(fields_after(&names(&["password"]), &now), names(&["user", "password", "token"]));
}

#[test]
fn the_give_json_holds_every_name_of_the_ask_and_no_other() {
    let wanted = ask(3, &["user", "password"], &["user", "password"]);
    let json = give_json(&wanted, &typed(&[("user", "ana"), ("password", "x1"), ("other", "y")])).expect("json");
    let read: Value = serde_json::from_str(&json).expect("object");
    assert_eq!(read, json!({ "user": "ana", "password": "x1" }));
    assert_eq!(give_json(&wanted, &typed(&[("user", "ana")])), None);
}

#[test]
fn the_give_json_keeps_quotes_backslashes_line_breaks_and_non_ascii() {
    let wanted = ask(3, &["a", "b", "c"], &["a", "b", "c"]);
    let values = [("a", "p\"a\\ss"), ("b", "two\nlines"), ("c", " ñ😀 ")];
    let json = give_json(&wanted, &typed(&values)).expect("json");
    let read: Map<String, Value> = serde_json::from_str(&json).expect("object");
    for (name, value) in values {
        assert_eq!(read[name], Value::String(value.to_string()), "{name}");
    }
}

#[test]
fn no_json_is_built_while_a_field_is_empty() {
    let wanted = ask(3, &["user", "password"], &["password"]);
    assert_eq!(give_json(&wanted, &typed(&[("user", ""), ("password", "x")])), None);
    assert!(give_json(&wanted, &typed(&[("password", "x")])).is_some());
}

#[test]
fn the_arguments_of_give_and_decline_never_hold_a_value() {
    assert_eq!(give_args("campus", 3), ["bot", "secret", "give", "campus", "3", "--json"]);
    assert_eq!(decline_args("campus", 3), ["bot", "secret", "decline", "campus", "3", "--json"]);
}

#[test]
fn return_saves_only_when_every_field_is_filled() {
    assert_eq!(on_enter(&[false, false]), Enter::Save);
}

#[test]
fn return_goes_to_the_first_empty_field() {
    assert_eq!(on_enter(&[false, true, true]), Enter::Focus(1));
    assert_eq!(on_enter(&[true]), Enter::Focus(0));
}

const SETTLED: Here = Here { shown: true, active: true, covered: false, keys: Keys::EmptyField };

#[test]
fn the_dialog_opens_by_itself_only_in_the_shown_chat_with_the_keys_in_its_empty_field() {
    assert!(opens(By::Itself, SETTLED, false));
    let off = [
        Here { shown: false, ..SETTLED },
        Here { active: false, ..SETTLED },
        Here { covered: true, ..SETTLED },
        Here { keys: keys_of(false, true), ..SETTLED },
        Here { keys: keys_of(true, false), ..SETTLED },
    ];
    for here in off {
        assert!(!opens(By::Itself, here, false), "{here:?}");
    }
    assert!(!opens(By::Itself, SETTLED, true));
}

#[test]
fn a_press_opens_it_unless_another_dialog_is_open() {
    let elsewhere = Here { shown: false, active: false, covered: true, keys: Keys::Elsewhere };
    assert!(opens(By::Person, elsewhere, false));
    assert!(!opens(By::Person, SETTLED, true));
}

#[test]
fn the_first_list_tells_no_ask() {
    let mut known = Known::default();
    assert_eq!(known.arrived(&[bot_with("a", vec![ask(1, &["x"], &[])])]), Vec::new());
}

#[test]
fn a_new_ask_arrives_once() {
    let mut known = Known::default();
    known.arrived(&[bot_with("a", vec![ask(1, &["x"], &[])])]);
    let list = [bot_with("a", vec![ask(1, &["x"], &[])]), bot_with("b", vec![ask(2, &["y"], &[])])];
    assert_eq!(known.arrived(&list), [("b".to_string(), 2)]);
    assert_eq!(known.arrived(&list), Vec::new());
}

#[test]
fn an_ask_that_left_is_forgotten() {
    let mut known = Known::default();
    known.arrived(&[bot_with("a", vec![ask(1, &["x"], &[])])]);
    assert_eq!(known.arrived(&[bot_with("a", Vec::new())]), Vec::new());
    assert_eq!(known.arrived(&[bot_with("a", vec![ask(3, &["x"], &[])])]), [("a".to_string(), 3)]);
}

#[test]
fn an_asks_rise_is_not_an_answers() {
    assert_eq!(counted_as(1, 2, 1), (2, 2));
    assert_eq!(counted_as(2, 2, 1), (2, 2));
    assert_eq!(counted_as(0, 1, 2), (1, 1));
    assert_eq!(counted_as(3, 0, 0), (3, 0));
}

#[test]
fn an_ask_in_the_shown_chat_of_the_active_window_is_not_told() {
    assert!(!tells_ask("a", (Some("a"), true), true));
    assert!(tells_ask("a", (Some("a"), false), true));
    assert!(tells_ask("a", (Some("b"), true), true));
    assert!(tells_ask("a", (None, true), true));
}

#[test]
fn nothing_is_told_below_the_gate() {
    assert!(!tells_ask("a", (None, true), false));
}

#[test]
fn the_dialog_leaves_when_its_ask_leaves_the_list() {
    let open = AskOpen { bot: "a".into(), ask: 1, dialog: 40 };
    assert!(!open.ended(&[bot_with("a", vec![ask(1, &["x"], &[])])], Some(40)));
    assert!(open.ended(&[bot_with("a", Vec::new())], Some(40)));
    assert!(open.ended(&[], Some(40)));
}

#[test]
fn another_dialog_is_left_alone() {
    let open = AskOpen { bot: "a".into(), ask: 1, dialog: 40 };
    assert!(!open.ended(&[], Some(41)));
    assert!(!open.ended(&[], None));
}

#[test]
fn giving_needs_this_computers_cornercase() {
    let has = Some(Version::new(0, 13, 100));
    assert_eq!(
        giving(Some(Lacks::ThisComputer(has)), Os::Mac),
        Give::Here("Giving it needs cornercase 0.13.101 or newer on this Mac (it has 0.13.100).".into())
    );
    assert_eq!(giving(None, Os::Mac), Give::Offered);
    assert_eq!(giving(Some(Lacks::Server(has)), Os::Mac), Give::No);
}

#[test]
fn full_access_is_said_in_the_error_colour() {
    assert_eq!(access_words("full"), Some(("full access", true)));
    assert_eq!(access_words("cornercase"), Some(("through cornercase", false)));
    assert_eq!(access_words(""), None);
}

#[test]
fn through_a_remote_the_sentence_names_no_computer() {
    assert!(what_happens(true).contains("on this computer"));
    assert!(!what_happens(false).contains("this computer"));
}

#[test]
fn the_title_is_cornercases_words() {
    assert_eq!(title("campus"), "campus asks for a secret");
}
