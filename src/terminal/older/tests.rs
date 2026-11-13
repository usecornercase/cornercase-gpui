use super::{behind_heading, behind_lead, older_heading, older_lead, restart_args, restart_job, restart_question};
use crate::cli::{Behind, Older};
use crate::platform::Os;

fn older(there: Option<&str>) -> Older {
    Older {
        destination: "vps".into(),
        there: there.map(str::to_string),
        here: "0.13.28".into(),
        update: ["cornercase", "--remote", "vps", "update"].map(String::from).to_vec(),
    }
}

fn behind() -> Behind {
    Behind { destination: "vps".into(), server: "0.13.21".into(), here: "0.13.28".into() }
}

#[test]
fn the_older_page_names_both_releases_and_what_the_window_needs() {
    let older = older(Some("0.13.21"));
    assert_eq!(older_heading(&older), "vps runs an older cornercase");
    let lead = older_lead(&older, Os::Mac);
    for part in ["0.13.21", "0.13.28", "this Mac", "the same release there, or a newer one"] {
        assert!(lead.contains(part), "{lead}");
    }
}

#[test]
fn a_cornercase_too_old_to_say_its_release_is_still_older() {
    let lead = older_lead(&older(None), Os::Mac);
    assert!(lead.contains("too old to say"), "{lead}");
    assert!(!lead.contains("None"), "{lead}");
}

#[test]
fn the_page_says_this_computer_on_linux() {
    assert!(older_lead(&older(Some("0.13.21")), Os::Linux).contains("this computer"));
}

#[test]
fn the_behind_page_names_the_servers_release() {
    assert_eq!(behind_heading(&behind()), "The server on vps still runs 0.13.21");
    let lead = behind_lead(&behind(), Os::Mac);
    assert!(lead.contains("0.13.28") && lead.contains("this Mac"), "{lead}");
}

#[test]
fn the_restart_asks_and_says_what_stops() {
    let (title, message, button) = restart_question("vps");
    assert_eq!(title, "Restart cornercase on vps?");
    assert!(message.contains("stops every program") && message.contains("come back"), "{message}");
    assert_eq!(button, "Restart");
}

#[test]
fn the_restart_runs_with_yes_on_the_windows_target() {
    assert_eq!(restart_args(), ["restart", "--yes"]);
}

#[test]
fn the_jobs_name_the_machine() {
    assert_eq!(restart_job("vps").key(), "restart-there:vps");
}
