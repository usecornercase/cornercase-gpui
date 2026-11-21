use super::*;
use crate::cli::{Feature, Lacks, Version};
use crate::test_util::fixture;

#[test]
fn a_bot_here_and_one_on_a_machine_with_the_same_id_are_two_keys() {
    assert_ne!(BotKey::here("prs"), BotKey::on("vps", "prs"));
    assert_eq!(BotKey::on("vps", "prs"), BotKey::on("vps", "prs"));
}

#[test]
fn a_machines_tag_is_not_its_id() {
    assert_eq!(BotKey::here("prs").tag(), "prs");
    assert_eq!(BotKey::on("vps", "prs").tag(), "vps/prs");
}

#[test]
fn the_label_names_the_machine_only_for_a_machines_bot() {
    assert_eq!(label("PRs", &Home::Here), "PRs");
    assert_eq!(label("PRs", &Home::Machine("vps".into())), "PRs on vps");
}

#[test]
fn a_bot_line_of_the_events_is_read() {
    let first = fixture!("bots/events.jsonl").lines().next().expect("a line");
    let event = event_of(first).expect("a bot event");
    assert_eq!((event.bot.as_str(), event.line.is_some()), ("ops", true));
}

#[test]
fn another_event_or_a_broken_line_is_no_bot_event() {
    assert_eq!(event_of(r#"{"event":"notice","bot":"ops"}"#), None);
    assert_eq!(event_of("{not json"), None);
}

#[test]
fn a_machines_bot_offers_no_give_and_says_where_to_give_it() {
    let vps = Home::Machine("vps".into());
    assert_eq!(giving_at(&Home::Here, Give::Offered), Give::Offered);
    let Give::Here(words) = giving_at(&vps, Give::Offered) else { panic!("words, not a button") };
    assert!(words.starts_with("Give it on vps"), "{words}");
}

fn releases_of(home: &Home, there: Option<&str>) -> Releases<'static> {
    let session = Session { version: "0.13.109".into(), ..Session::default() };
    let machines = [Machine { name: "vps".into(), server_version: there.map(str::to_string), ..Machine::default() }];
    releases_at(home, (Some(&session), &machines), Release::Read(Some(Version::new(0, 13, 109))))
}

#[test]
fn a_machines_bot_is_gated_on_that_machines_release() {
    let vps = Home::Machine("vps".into());
    assert_eq!(releases_of(&vps, Some("0.13.95")).server, Some(Version::new(0, 13, 95)));
    assert_eq!(
        releases_of(&vps, Some("0.13.95")).lacks(Feature::BotAboutRead),
        Some(Lacks::Server(Some(Version::new(0, 13, 95))))
    );
    assert_eq!(releases_of(&vps, None).lacks(Feature::BotAboutRead), Some(Lacks::Server(None)));
    assert_eq!(releases_of(&Home::Machine("gone".into()), Some("0.13.109")).server, None);
    assert_eq!(releases_of(&vps, Some("0.13.109")).lacks(Feature::BotAboutRead), None);
}

#[test]
fn this_computers_bot_is_gated_on_the_sessions_release() {
    let here = releases_of(&Home::Here, Some("0.13.21"));
    assert_eq!((here.server, here.machine), (Some(Version::new(0, 13, 109)), None));
    assert_eq!(here.lacks(Feature::BotAboutRead), None);
}
