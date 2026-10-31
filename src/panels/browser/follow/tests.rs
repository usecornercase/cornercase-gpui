use super::*;
use crate::panels::browser::tabs::Tab;

fn ready_long_ago() -> (Follow, Instant) {
    let mut follow = Follow::default();
    let start = Instant::now();
    follow.ready(start);
    (follow, start + Duration::from_secs(5))
}

#[test]
fn a_tab_nobody_here_opened_belongs_to_an_agent() {
    assert!(foreign(false, None));
}

#[test]
fn a_tab_the_panel_opened_or_a_page_of_a_workspace_opened_is_not_an_agents() {
    assert!(!foreign(true, Some(10)));
    assert!(!foreign(true, None));
    assert!(!foreign(false, Some(10)));
}

#[test]
fn the_tabs_found_on_connecting_are_not_followed() {
    let mut follow = Follow::default();
    let start = Instant::now();
    follow.ready(start);
    assert!(!follow.follows(start + Duration::from_millis(500), false));
}

#[test]
fn a_tab_an_agent_opens_later_is_followed() {
    let (follow, now) = ready_long_ago();
    assert!(follow.follows(now, false));
}

#[test]
fn the_user_at_the_panel_is_not_interrupted() {
    let (mut follow, now) = ready_long_ago();
    follow.engaged = true;
    assert!(!follow.follows(now, false));
    follow.engaged = false;
    assert!(!follow.follows(now, true));
}

#[test]
fn following_can_be_turned_off() {
    let (mut follow, now) = ready_long_ago();
    follow.off = true;
    assert!(!follow.follows(now, false));
}

#[test]
fn a_new_address_is_a_move_but_a_reload_or_an_anchor_is_not() {
    assert!(moved("about:blank", "http://localhost:5173/"));
    assert!(moved("http://localhost:5173/", "http://localhost:5173/cart"));
    assert!(!moved("http://localhost:5173/", "http://localhost:5173/"));
    assert!(!moved("http://localhost:5173/#top", "http://localhost:5173/#faq"));
}

#[test]
fn a_tab_claimed_after_chrome_announced_it_is_not_an_agents() {
    let mut tabs = Tabs::default();
    tabs.add(Tab::new("mine", "http://localhost:5173/", Some(10)));
    tabs.add(Tab::new("theirs", "http://localhost:5173/", None));
    let foreign = HashSet::from(["mine".to_string(), "theirs".to_string()]);
    assert!(!agents_tab(&foreign, &tabs, "mine"));
    assert!(agents_tab(&foreign, &tabs, "theirs"));
}
