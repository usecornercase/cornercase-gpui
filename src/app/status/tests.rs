use super::conn_words;
use crate::app::Conn;
use crate::cli::{Behind, Older};
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
