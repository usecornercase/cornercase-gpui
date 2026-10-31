use super::{Health, Service, Status};

fn status() -> Status {
    serde_json::from_str(crate::test_util::fixture!("plugins/status.json")).expect("the status parses")
}

#[test]
fn says_what_each_service_is_doing() {
    let status = status();
    let said: Vec<String> = status.services("acme.notes").iter().map(Service::describe).collect();
    assert_eq!(said, ["sync running, pid 4242", "index stopped, restarting in 3 s", "watch starting"]);
}

#[test]
fn colours_follow_the_service_state() {
    let status = status();
    let health: Vec<Health> = status.services("acme.notes").iter().map(Service::health).collect();
    assert_eq!(health, [Health::Running, Health::Stopped, Health::Waiting]);
}

#[test]
fn a_plugin_without_services_or_unknown_has_none() {
    let status = status();
    assert!(status.services("gpui.test").is_empty());
    assert!(status.services("unknown").is_empty());
}

#[test]
fn keeps_why_the_plugins_cannot_be_read() {
    assert!(status().problem.is_some_and(|p| p.starts_with("plugins.json cannot be read")));
}

#[test]
fn a_service_restarting_now_or_in_an_unknown_state_still_reads() {
    let now = Service { id: "a".into(), state: "stopped".into(), pid: None, restart_in: Some(0) };
    assert_eq!(now.describe(), "a stopped, restarting now");
    let odd = Service { id: "b".into(), state: "paused".into(), pid: None, restart_in: None };
    assert_eq!(odd.describe(), "b paused");
    assert_eq!(odd.health(), Health::Waiting);
}
