use serde_json::json;

use super::*;
use crate::panels::browser::agent::Menu;
use crate::panels::browser::element::Element;
use crate::panels::browser::picker::{Crop, Pick, Picked};

#[test]
fn copying_asks_the_page_for_its_selection() {
    let script = selection_script(false);
    assert_eq!(script["returnByValue"], true);
    assert!(script["expression"].as_str().is_some_and(|e| e.contains("__cornercase.selected()")));
}

#[test]
fn cutting_asks_the_page_to_delete_what_it_returns() {
    assert!(selection_script(true)["expression"].as_str().is_some_and(|e| e.contains("__cornercase.cut()")));
}

#[test]
fn the_selection_comes_back_as_the_value() {
    let reply = json!({ "result": { "type": "string", "value": "hello" } });
    assert_eq!(evaluated_text(&reply).as_deref(), Some("hello"));
}

#[test]
fn an_empty_selection_leaves_the_clipboard_alone() {
    assert_eq!(evaluated_text(&json!({ "result": { "type": "string", "value": "" } })), None);
    assert_eq!(evaluated_text(&json!({ "result": { "type": "undefined" } })), None);
}

#[test]
fn a_typed_address_keeps_the_keys_in_the_address_until_its_page_shows() {
    assert_eq!(keys_go(KeysAt::Address, false, Waiting::Same), (KeysGo::Stay, true));
}

#[test]
fn the_page_takes_the_keys_once_the_typed_page_shows() {
    assert_eq!(keys_go(KeysAt::Address, true, Waiting::Same), (KeysGo::Page, false));
}

#[test]
fn a_page_that_went_away_hands_its_keys_to_the_address() {
    assert_eq!(keys_go(KeysAt::Viewport, false, Waiting::No), (KeysGo::Address, false));
}

#[test]
fn a_page_with_the_keys_keeps_them() {
    assert_eq!(keys_go(KeysAt::Viewport, true, Waiting::No), (KeysGo::Stay, false));
}

#[test]
fn an_address_nobody_waits_on_keeps_the_keys_when_its_page_shows() {
    assert_eq!(keys_go(KeysAt::Address, true, Waiting::No), (KeysGo::Stay, false));
}

#[test]
fn an_address_edited_while_its_page_waits_keeps_the_keys_and_drops_the_wait() {
    assert_eq!(keys_go(KeysAt::Address, true, Waiting::Edited), (KeysGo::Stay, false));
    assert_eq!(keys_go(KeysAt::Address, false, Waiting::Edited), (KeysGo::Stay, false));
}

#[test]
fn a_click_elsewhere_drops_the_wait_for_the_page() {
    assert_eq!(keys_go(KeysAt::Elsewhere, false, Waiting::Same), (KeysGo::Stay, false));
    assert_eq!(keys_go(KeysAt::Elsewhere, true, Waiting::Same), (KeysGo::Stay, false));
}

fn agents(menu: Option<Menu>, pick: Option<Pick>) -> Agents {
    Agents { pick, menu, ..Agents::default() }
}

#[test]
fn every_browser_menu_is_one_thing_on_top() {
    for menu in [Menu::Agents, Menu::Shots] {
        assert_eq!(escape_tops(false, &agents(Some(menu), None), false), [Top::Menu]);
    }
    assert_eq!(escape_tops(false, &agents(None, None), true), [Top::Menu]);
}

fn picked() -> Pick {
    Pick::Picked(Box::new(Picked { element: Element::default(), crop: Crop::Missing }))
}

#[test]
fn the_chooser_sits_over_the_picked_card() {
    let tops = escape_tops(false, &agents(Some(Menu::Chooser), Some(picked())), false);
    assert_eq!(tops, [Top::Menu, Top::Pick]);
}

#[test]
fn a_chooser_without_its_card_is_nothing_on_top() {
    assert_eq!(escape_tops(false, &agents(Some(Menu::Chooser), None), false), Vec::<Top>::new());
    let picking = agents(Some(Menu::Chooser), Some(Pick::Picking("s".into())));
    assert_eq!(escape_tops(false, &picking, false), [Top::Pick]);
}

#[test]
fn the_other_menus_need_no_card() {
    assert_eq!(escape_tops(false, &agents(Some(Menu::Shots), None), false), [Top::Menu]);
}

#[test]
fn a_pick_in_progress_is_on_top() {
    assert_eq!(escape_tops(false, &agents(None, Some(Pick::Picking("s".into()))), false), [Top::Pick]);
}

#[test]
fn a_select_menu_is_listed_first() {
    assert_eq!(escape_tops(true, &agents(Some(Menu::Agents), None), false), [Top::Choice, Top::Menu]);
}

#[test]
fn nothing_open_is_nothing_on_top() {
    assert_eq!(escape_tops(false, &Agents::default(), false), Vec::<Top>::new());
}
