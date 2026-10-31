use super::{Checked, Preview, install_question, update_question};

fn preview(text: &str) -> Preview {
    Preview::parse(text).expect("the preview parses")
}

fn notch() -> Preview {
    preview(crate::test_util::fixture!("plugins/install-preview.json"))
}

fn notes() -> Preview {
    preview(crate::test_util::fixture!("plugins/update-preview.json"))
}

fn current() -> Preview {
    preview(crate::test_util::fixture!("plugins/update-current.json"))
}

#[test]
fn an_install_preview_confirms_its_own_commit() {
    let p = notch();
    let command = p.install_command("jacogaz/cornercase-notch");
    assert_eq!(command[..3], ["plugin", "install", "jacogaz/cornercase-notch"]);
    assert_eq!(command[3..], ["--commit", "4da3bb3f5be198050dc3cba0e96d15084a5e068a", "--yes"]);
    assert_eq!(p.installed(), "installed Notch 0.1.1, turned off");
}

#[test]
fn the_install_question_shows_every_preview_line() {
    let p = notch();
    let q = install_question(&p);
    assert_eq!(q.title, "Install Notch?");
    assert_eq!(q.details, p.lines);
    assert!(q.details.iter().any(|l| l == "  window process panel: bin/cornercase-notch panel"));
}

#[test]
fn an_update_preview_says_what_changes() {
    let p = notes();
    assert!(!p.up_to_date);
    assert_eq!(p.change(), "Notes: 0123456789ab → 89abcdef0123");
    assert_eq!(p.updated(), "updated Notes to 1.3.0, still on");
    let command = p.update_command();
    assert_eq!(command[..3], ["plugin", "update", "acme.notes"]);
    assert_eq!(command[3..], ["--commit", "89abcdef0123456789abcdef0123456789abcdef", "--yes"]);
}

#[test]
fn an_up_to_date_plugin_has_no_lines() {
    let p = current();
    assert!(p.up_to_date);
    assert_eq!(p.id, "jacogaz.notch");
    assert_eq!(p.lines.len(), 0);
}

#[test]
fn one_update_is_asked_by_name_and_several_by_count() {
    let p = notes();
    assert_eq!(update_question(std::slice::from_ref(&p)).title, "Update Notes?");
    let both = update_question(&[p.clone(), p.clone()]);
    assert_eq!(both.title, "Update 2 plugins?");
    assert_eq!(both.details.len(), 2 * (p.lines.len() + 1));
    assert_eq!(both.details[0], "Notes: 0123456789ab → 89abcdef0123");
    assert_eq!(both.details[1], "  Notes 1.3.0 (acme.notes)");
}

#[test]
fn checking_waits_for_every_plugin() {
    let mut checked = Checked::new(2);
    assert!(!checked.add(Ok(current())));
    assert_eq!(checked.up_to_date().as_deref(), Some("jacogaz.notch is up to date"));
    assert!(checked.add(Ok(current())));
    assert_eq!(checked.up_to_date().as_deref(), Some("every installed plugin is up to date"));
    assert!(checked.changed.is_empty());
}

#[test]
fn a_change_or_a_failure_is_not_up_to_date() {
    let mut checked = Checked::new(3);
    checked.add(Ok(current()));
    checked.add(Err(anyhow::anyhow!("acme.x is linked")));
    assert_eq!(checked.up_to_date(), None);
    assert!(checked.add(Ok(notes())));
    assert_eq!(checked.failed, ["acme.x is linked"]);
    assert_eq!(checked.changed.len(), 1);
}
