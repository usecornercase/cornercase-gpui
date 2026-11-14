use super::*;

fn session() -> Session {
    serde_json::from_value(serde_json::json!({
        "boot": "b", "revision": 1, "version": "0.12.189", "groups": [],
        "projects": [{"id": 1, "name": "shop", "path": "/p", "workspaces": [{
            "id": 2, "name": "main", "path": "/p", "tabs": [
                {"id": 3, "name": "zsh", "panes": [
                    {"id": 4, "path": "/p", "active": true, "title": "vim notes.md"},
                    {"id": 5, "path": "/p", "active": false, "title": null}]},
                {"id": 6, "name": "claude", "panes": [{"id": 7, "path": "/p", "active": true}]}
            ]}]}],
        "view": {"id": 9, "projection": 1, "active": true, "shown": {"project": 1, "workspace": 2, "tab": 3, "pane": 4}}
    }))
    .expect("a session")
}

#[test]
fn the_title_is_the_panes_or_the_tabs() {
    let mut s = session();
    assert_eq!(window_title(&s), "vim notes.md — cornercase");
    s.view.as_mut().expect("view").shown.pane = Some(5);
    assert_eq!(window_title(&s), "zsh — cornercase");
    s.view.as_mut().expect("view").shown.tab = Some(6);
    s.view.as_mut().expect("view").shown.pane = None;
    assert_eq!(window_title(&s), "claude — cornercase");
    s.view = None;
    assert_eq!(window_title(&s), "cornercase");
}

#[test]
fn a_tab_is_placed() {
    assert_eq!(tab_place(&session(), 6).as_deref(), Some("shop › main › claude"));
    assert_eq!(tab_place(&session(), 60), None);
}

#[test]
fn a_failure_told_while_the_window_is_behind_is_held_once_for_when_it_is_seen() {
    let mut held = Vec::new();
    let told = HashSet::new();
    hold(&mut held, &told, alert::NOTIFICATION_FAILED, "first".into());
    hold(&mut held, &told, alert::NOTIFICATION_FAILED, "second".into());
    assert_eq!(held, vec![(alert::NOTIFICATION_FAILED, "first".to_string())]);
}

#[test]
fn a_failure_already_told_this_run_is_not_held_again() {
    let mut held = Vec::new();
    let told = HashSet::from([alert::NOTIFICATION_FAILED]);
    hold(&mut held, &told, alert::NOTIFICATION_FAILED, "again".into());
    assert_eq!(held, Vec::<(&str, String)>::new());
}

#[test]
fn an_update_toast_waits_while_the_window_is_behind() {
    let toast = UpdateToast::install_failed("cornercase", "0.13.50", "no", update::Updating::Cornercase);
    let mut later = Vec::new();
    assert_eq!(hold_or_tell(false, toast.clone(), &mut later), None);
    assert_eq!(later, vec![toast.clone()]);
    assert_eq!(hold_or_tell(true, toast.clone(), &mut later), Some(toast));
    assert_eq!(later.len(), 1);
}

#[test]
fn the_binary_is_read_again_only_when_update_said_nothing_the_window_reads() {
    use cli::{Said, Target, UpdateAnswer};
    let remote = Target::Remote(cli::Remote::new("vps", "").expect("a destination"));
    let nothing = UpdateAnswer::Printed(Said::Done(String::new()));
    assert!(checks_disk(&Target::Local, &nothing));
    assert!(!checks_disk(&remote, &nothing));
    let updated = UpdateAnswer::Updated { from: "0.13.53".into(), version: "0.13.74".into() };
    assert!(!checks_disk(&Target::Local, &updated));
    assert!(!checks_disk(&Target::Local, &UpdateAnswer::UpToDate { version: "0.13.74".into() }));
    assert!(!checks_disk(&Target::Local, &UpdateAnswer::Printed(Said::Failed("no".into()))));
    assert!(!checks_disk(&Target::Local, &UpdateAnswer::Refused));
}
