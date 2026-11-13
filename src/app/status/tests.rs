use super::conn_words;
use crate::app::Conn;
use crate::cli::{Behind, Older};
use crate::model::machines::listed;
use crate::platform::Os;
use crate::terminal::{behind_lead, older_lead};

fn older(there: Option<&str>) -> Older {
    Older { destination: "vps".into(), there: there.map(str::to_string), here: "0.13.28".into(), update: Vec::new() }
}

#[test]
fn a_lost_connection_says_disconnected_only_after_it_came_up() {
    let conn = Conn::Lost("cornercase start-server: Error: boom".into());
    assert_eq!(conn_words(&conn, false, ""), ("not connected".to_string(), Some("boom".to_string())));
    assert_eq!(conn_words(&conn, true, "").0, "disconnected");
}

#[test]
fn an_older_cornercase_there_reads_as_the_machines_sheet_says_it() {
    let known = older(Some("0.13.21"));
    assert_eq!(
        conn_words(&Conn::Older(known.clone()), false, ""),
        ("older · 0.13.21".into(), Some(older_lead(&known, Os::CURRENT)))
    );
    let unknown = older(None);
    assert_eq!(
        conn_words(&Conn::Older(unknown.clone()), false, ""),
        ("older".into(), Some(older_lead(&unknown, Os::CURRENT)))
    );
    let behind = Behind { destination: "vps".into(), server: "0.13.21".into(), here: "0.13.28".into() };
    assert_eq!(
        conn_words(&Conn::Behind(behind.clone()), false, ""),
        ("older server · 0.13.21".into(), Some(behind_lead(&behind, Os::CURRENT)))
    );
}

#[test]
fn the_connection_tip_names_this_windows_version() {
    let window = format!("This window is {}", crate::self_update::version_line());
    let with = super::connection_tip(Some("boom".into()), None);
    assert_eq!(with, format!("boom\n{window}"));
    assert!(super::connection_tip(None, None).ends_with(&window));
}

#[test]
fn the_label_counts_this_windows_tunnels_on_another_server() {
    let parts = super::label_parts(Some("devbox"), 2, "cornercase 0.13.51");
    assert_eq!(parts.count.as_deref(), Some("⇄2"));
    assert_eq!(parts.host.as_deref(), Some("devbox"));
    assert!(super::label_parts(Some("devbox"), 0, "x").count.is_none());
}

#[test]
fn the_label_counts_a_machines_tunnels_on_this_computer() {
    let parts = super::label_parts(None, 2, "cornercase 0.13.54");
    assert_eq!((parts.host, parts.count.as_deref()), (None, Some("⇄2")));
    assert!(super::label_parts(None, 0, "x").count.is_none());
}

#[test]
fn the_tip_names_the_machines_with_tunnels() {
    let names = |names: &[&str]| listed(&names.iter().map(ToString::to_string).collect::<Vec<_>>());
    assert_eq!(names(&["vps"]), "vps");
    assert_eq!(names(&["vps", "cap"]), "vps and cap");
    assert_eq!(names(&["vps", "cap", "nas"]), "vps, cap and nas");
    assert!(super::connection_tip(None, Some("vps")).starts_with("Tunnels to vps, and connecting to another server"));
}

#[test]
fn the_tip_leads_with_the_tunnels_only_on_another_server() {
    assert!(
        super::connection_tip(None, Some("devbox")).starts_with("Tunnels to devbox, and connecting to another server")
    );
    assert!(super::connection_tip(Some("boom".into()), Some("devbox")).starts_with("boom"));
}

#[test]
fn a_machine_tunnels_open_entry_says_where_it_opens() {
    use super::tunnels::open_label;
    use crate::model::Owner;
    use crate::model::tunnels::Toward;
    let machine = Toward::Machine("vps".into());
    assert_eq!(open_label(&machine, None), "Open in the system browser");
    assert_eq!(open_label(&machine, Some(&Owner::Nobody)), "Open in the Browser");
    assert_eq!(open_label(&Toward::Server, None), "Open in the Browser");
}
