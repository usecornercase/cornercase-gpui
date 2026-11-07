use super::{Linked, PairKey, Reading};
use crate::test_util::fixture;

fn lines(text: &str) -> Vec<&str> {
    text.lines().collect()
}

fn read(key: PairKey, line: &str) -> Reading {
    let mut reading = Reading::Asking;
    assert_eq!(reading.line(key, line), None, "not the end of it");
    reading
}

fn linked(reading: Reading) -> Linked {
    let Reading::Showing(linked) = reading else { panic!("no address in {reading:?}") };
    linked
}

fn telegram() -> Vec<&'static str> {
    lines(fixture!("settings/pair-telegram.jsonl"))
}

#[test]
fn only_browsers_and_chats_can_be_paired() {
    assert_eq!(PairKey::of("browsers"), Some(PairKey::Browsers));
    assert_eq!(PairKey::of("telegram.chat"), Some(PairKey::Telegram));
    assert_eq!(PairKey::of("discord.chat"), Some(PairKey::Discord));
    assert_eq!(PairKey::of("telegram"), None);
    assert_eq!(PairKey::of("linear"), None);
}

#[test]
fn each_key_names_itself_to_the_command() {
    assert_eq!(PairKey::Discord.args(), ["config", "pair", "--json", "--", "discord.chat"]);
    assert_eq!(PairKey::Browsers.args(), ["config", "pair", "--json", "--", "browsers"]);
}

#[test]
fn the_dialogs_take_the_titles_of_the_tuis() {
    assert_eq!(PairKey::Browsers.title(), "Remote");
    assert_eq!(PairKey::Telegram.title(), "Telegram");
    assert_eq!(PairKey::Discord.title(), "Discord");
}

#[test]
fn a_line_with_no_address_yet_says_what_it_waits_for() {
    let first = lines(fixture!("settings/pair-browsers.jsonl"))[0];
    assert_eq!(read(PairKey::Browsers, first), Reading::Starting(vec!["Starting Tailscale…".to_string()]));
}

#[test]
fn a_line_with_an_address_gives_its_code_and_the_lines_under_it() {
    let line = lines(fixture!("settings/pair-browsers.jsonl"))[1];
    let linked = linked(read(PairKey::Browsers, line));
    assert_eq!(linked.url, "https://ana.tail0.ts.net:8443/pair?t=00112233445566778899aabbccddeeff");
    assert!(linked.code.is_some());
    assert_eq!(
        linked.notes,
        ["Scan the code with your phone's camera.", "The phone needs the Tailscale app, logged in as ana@example.com."]
    );
}

#[test]
fn a_line_keeps_cornercases_name_in_lower_case() {
    let line =
        r#"{"lines":["cornercase then writes to you, and the bot answers only you."],"url":"https://example.com"}"#;
    assert_eq!(
        linked(read(PairKey::Discord, line)).notes,
        ["cornercase then writes to you, and the bot answers only you."]
    );
}

#[test]
fn a_later_line_replaces_what_was_shown() {
    let mut reading = read(PairKey::Telegram, telegram()[0]);
    assert_eq!(
        linked(reading.clone()).notes,
        ["Scan the code with your phone's camera, then tap Start in Telegram.", "The bot then answers only that chat."]
    );
    let advice = r#"{"lines":["make the app private"],"url":"https://t.me/ana_bot?start=00112233"}"#;
    assert_eq!(reading.line(PairKey::Telegram, advice), None);
    assert_eq!(linked(reading).notes, ["Make the app private"]);
}

#[test]
fn an_address_too_long_for_a_code_is_still_shown() {
    let long = format!(r#"{{"lines":[],"url":"https://t.me/{}"}}"#, "x".repeat(4000));
    let linked = linked(read(PairKey::Telegram, &long));
    assert_eq!(linked.code, None);
    assert!(linked.url.starts_with("https://t.me/x"));
}

#[test]
fn a_line_that_says_nothing_keeps_what_shows() {
    let mut reading = read(PairKey::Telegram, telegram()[0]);
    let before = reading.clone();
    assert_eq!(reading.line(PairKey::Telegram, "{}"), None);
    assert_eq!(reading.line(PairKey::Telegram, "not json"), None);
    assert_eq!(reading, before);
}

#[test]
fn a_paired_browser_says_a_browser_connected() {
    let end = lines(fixture!("settings/pair-browsers.jsonl"))[2];
    assert_eq!(Reading::Asking.line(PairKey::Browsers, end).as_deref(), Some("A browser connected"));
}

#[test]
fn a_paired_chat_says_its_name() {
    let end = telegram()[1];
    assert_eq!(Reading::Asking.line(PairKey::Telegram, end).as_deref(), Some("Telegram connected with Ana"));
}

#[test]
fn a_paired_chat_without_a_name_says_its_platform() {
    assert_eq!(Reading::Asking.line(PairKey::Discord, r#"{"paired":true}"#).as_deref(), Some("Discord connected"));
    let empty = r#"{"chat":"","paired":true}"#;
    assert_eq!(Reading::Asking.line(PairKey::Discord, empty).as_deref(), Some("Discord connected"));
}

#[test]
fn a_stream_that_stops_says_why_in_the_servers_words() {
    let end = lines(fixture!("settings/pair-tailscale-stopped.jsonl"))[1];
    let reading = read(PairKey::Browsers, end);
    assert_eq!(reading, Reading::Refused("Tailscale is not connected (Stopped): open it and log in".to_string()));
}

#[test]
fn the_error_after_a_stop_does_not_replace_the_words_of_the_stop() {
    let end = r#"{"error":"no phone paired within ten minutes: run this again","paired":false}"#;
    let mut reading = read(PairKey::Browsers, end);
    reading.ended("Error: something else\n");
    assert_eq!(reading, Reading::Refused("No phone paired within ten minutes: run this again".to_string()));
}

#[test]
fn a_refusal_to_start_comes_in_words() {
    for (stderr, words) in [
        (
            "Error: remote access is off: turn it on in Settings › Remote or with `cornercase config set remote true`\n",
            "Remote access is off: turn it on in Settings › Remote or with `cornercase config set remote true`",
        ),
        ("Error: tailscale is not installed on this computer\n", "Tailscale is not installed on this computer"),
        ("Error: paste the bot's token first\n", "Paste the bot's token first"),
    ] {
        let mut reading = Reading::Asking;
        reading.ended(stderr);
        assert_eq!(reading, Reading::Refused(words.to_string()));
    }
}

#[test]
fn a_stream_lost_with_a_code_showing_says_why() {
    let mut reading = read(PairKey::Telegram, telegram()[0]);
    reading.ended("Error: the server stopped before a phone paired\n");
    assert_eq!(reading, Reading::Refused("The server stopped before a phone paired".to_string()));
}

#[test]
fn a_stream_that_ends_with_nothing_to_say_says_no_phone_was_paired() {
    let mut reading = read(PairKey::Telegram, telegram()[0]);
    reading.ended("");
    assert_eq!(reading, Reading::Refused("No phone was paired".to_string()));
}
