use super::*;
use crate::notify::{Event, parse_event};
use crate::store::args;

fn run(parts: &[&str]) -> ToastGo {
    ToastGo::Run(args(parts))
}

#[test]
fn only_a_mac_app_with_a_bundle_identifier_posts_through_its_own_center() {
    assert!(uses_center(Os::Mac, Some("dev.usecornercase.gpui")));
    assert!(!uses_center(Os::Mac, None));
    assert!(!uses_center(Os::Mac, Some("")));
    assert!(!uses_center(Os::Linux, Some("dev.usecornercase.gpui")));
    assert!(!uses_center(Os::Windows, Some("dev.usecornercase.gpui")));
}

#[test]
fn a_click_on_a_notification_does_what_its_toasts_go_does() {
    let mut posted = Posted::default();
    let id = posted.remember(run(&["go", "cc-ab12cd-3"]));
    assert_eq!(posted.take(&id), Some(run(&["go", "cc-ab12cd-3"])));
    assert_eq!(posted.take(&id), None);
}

#[test]
fn a_click_on_a_machines_notice_goes_to_its_panes_there() {
    let Some(Event::Notice(notice)) = parse_event(crate::test_util::fixture!("events/notice-machine.json")) else {
        panic!("not a notice");
    };
    let seen = crate::model::MachineSeen { name: "vps".into(), boot: "b".into(), ..Default::default() };
    let action = notice.go(notice.there(&[seen]));
    assert!(matches!(action, ToastGo::Machine(_)));
    let mut posted = Posted::default();
    let id = posted.remember(action.clone());
    assert_eq!(posted.take(&id), Some(action));
}

#[test]
fn a_notification_this_store_did_not_post_only_brings_the_window_forward() {
    let mut posted = Posted::default();
    posted.remember(run(&["go", "a"]));
    assert_eq!(posted.take("1-1000000"), None);
    assert_eq!(Posted::default().take("1-1"), None);
}

#[test]
fn only_the_last_64_notices_are_kept_as_the_server_keeps_their_tokens() {
    let mut posted = Posted::default();
    let ids: Vec<String> = (0..=KEEP).map(|n| posted.remember(run(&["go", &n.to_string()]))).collect();
    assert_eq!(posted.take(&ids[0]), None);
    assert!(posted.take(&ids[1]).is_some());
    assert!(posted.take(&ids[KEEP]).is_some());
}

#[test]
fn each_notification_of_a_run_has_an_id_of_its_own() {
    let mut posted = Posted::default();
    let first = posted.remember(run(&["go", "a"]));
    let second = posted.remember(run(&["go", "a"]));
    assert_ne!(first, second);
    let prefix = format!("{}-", std::process::id());
    assert!(first.starts_with(&prefix) && second.starts_with(&prefix));
}

#[test]
fn a_refusal_says_where_to_allow_notifications() {
    let none = Refusal { allowed: false, error: None };
    assert_eq!(
        none.why(),
        "macOS does not let cornercase notify you; turn on Allow notifications in System Settings › Notifications › cornercase, or set desktop notifications to off in Settings › UI"
    );
    let said = Refusal { allowed: false, error: Some("Notifications are not allowed for this application".into()) };
    assert_eq!(
        said.why(),
        "Notifications are not allowed for this application; turn on Allow notifications in System Settings › Notifications › cornercase, or set desktop notifications to off in Settings › UI"
    );
    assert_eq!(Refusal { allowed: false, error: Some("  ".into()) }.why(), none.why());
}

#[test]
fn a_post_that_fails_says_what_macos_said() {
    assert_eq!(Refusal { allowed: true, error: Some("it broke".into()) }.why(), "it broke");
    assert_eq!(Refusal { allowed: true, error: None }.why(), "macOS did not post it");
}

#[test]
fn without_a_bundle_the_center_is_never_asked() {
    assert!(!available());
    assert_eq!(post(("1-1", "claude finished", "in shop")), Err(NO_BUNDLE.to_string()));
}
