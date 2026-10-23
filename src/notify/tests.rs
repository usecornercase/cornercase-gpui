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
