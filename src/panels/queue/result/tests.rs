use serde_json::json;

use super::{
    Answer, Fact, Kept, Page, Said, Talk, can_copy, can_retry_talk, copied, facts, merged, message_lines, notes,
    page_after, results_offered, role_word, said_heading, talk_lines, time_words, with_talk,
};
use crate::cli::Version;
use crate::panels::queue::data::Entry;
use crate::panels::queue::read::Keep;
use crate::panels::queue::time::Civil;
use crate::test_util::fixture;

const TODAY: Civil = Civil { year: 2026, month: 11, day: 12, hour: 12, minute: 0, weekday: 4 };

fn release(text: &str) -> Option<Version> {
    Version::parse(text)
}

fn said(role: &str, text: &str, written: Option<&str>, cut: bool) -> Said {
    Said { role: role.into(), text: text.into(), written: written.map(str::to_string), cut }
}

#[test]
fn the_detail_needs_cornercase_0_13_51() {
    assert!(!results_offered(None));
    assert!(!results_offered(release("0.13.50")));
    assert!(!results_offered(release("0.13.39")));
    assert!(results_offered(release("0.13.51")));
    assert!(results_offered(release("0.13.53")));
}

#[test]
fn reads_what_a_run_did() {
    let answer = Answer::read(fixture!("queue/show-result.json"), false).expect("the fixture");
    let result = answer.result.as_ref().expect("a result");
    assert_eq!((answer.entry.id, answer.entry.state.as_str()), (2, "started"));
    assert!(result.text.starts_with("## Done"));
    assert!(result.conversation && answer.can_talk());
    assert_eq!(answer.talk, None);
    assert_ne!(answer.blocks.len(), 0);
    assert_eq!(answer.lines, message_lines(&result.text));
    assert!(answer.entry.started.is_some() && answer.entry.result_at.is_some());
}

#[test]
fn reads_a_conversation_oldest_first() {
    let answer = Answer::read(fixture!("queue/show-conversation.json"), true).expect("the fixture");
    let Some(Talk::Said(talk)) = answer.talk else { panic!("a conversation") };
    let roles: Vec<&str> = talk.messages.iter().map(|m| m.role.as_str()).collect();
    assert_eq!(roles, ["prompt", "reply"]);
    assert!(!talk.earlier);
    assert_eq!(talk.blocks.len(), 2);
    assert!(talk.messages.iter().all(|m| m.written.is_some()));
}

#[test]
fn a_record_that_is_gone_keeps_the_last_message_and_says_why() {
    let answer = Answer::read(fixture!("queue/show-gone.json"), true).expect("the fixture");
    assert_eq!(answer.talk, Some(Talk::Gone));
    assert!(answer.result.as_ref().is_some_and(|r| r.text.starts_with("## Done") && !r.conversation));
    assert!(answer.note.as_deref().is_some_and(|note| note.contains("no longer has")));
    assert!(!answer.can_talk());
}

#[test]
fn a_custom_agent_has_no_message() {
    let answer = Answer::read(fixture!("queue/show-custom.json"), false).expect("the fixture");
    assert_eq!(answer.result, None);
    assert!(answer.lines.is_empty() && answer.blocks.is_empty());
    assert!(answer.note.as_deref().is_some_and(|note| note.contains("no result was kept")));
}

#[test]
fn an_answer_from_another_release_reads_leniently() {
    let text = json!({
        "id": 7, "state": "started", "new_key": [1], "result": { "text": "hi", "future": true },
        "messages": [
            { "role": "robot", "text": "beep" },
            { "role": 5, "text": "wrong type" },
            "not a message",
        ],
    })
    .to_string();
    let answer = Answer::read(&text, true).expect("it reads");
    assert_eq!((answer.note, answer.result.map(|r| r.text)), (None, Some("hi".to_string())));
    let Some(Talk::Said(talk)) = answer.talk else { panic!("a conversation") };
    assert_eq!(talk.messages.len(), 1);
    assert_eq!(role_word(&talk.messages[0].role), "Robot");
    assert!(!talk.earlier);
    let plain = Answer::read(&json!({ "id": 1 }).to_string(), true).expect("a bare entry");
    assert_eq!(plain.talk, Some(Talk::Gone));
}

#[test]
fn an_answer_that_is_not_json_is_an_error() {
    assert!(Answer::read("Error: unknown", false).is_err());
    assert!(Answer::read("[1, 2]", false).is_err());
}

fn entry() -> Entry {
    Entry { id: 3, at: 1_794_535_359, agent: "claude".into(), ..Entry::default() }
}

#[test]
fn facts_say_when_it_was_scheduled_started_and_kept() {
    let mut run = entry();
    run.started = Some(run.at + 60);
    run.place.branch = Some("fix".into());
    let kept =
        Kept { at: run.at + 3600, branch: Some("routine/x".into()), agent: Some("codex".into()), ..Kept::default() };
    let list = facts(&run, Some(&kept), TODAY, run.at + 7200);
    let labels: Vec<&str> = list.iter().map(|f| f.label).collect();
    assert_eq!(labels, ["Scheduled", "Started", "Result", "Agent", "Branch"]);
    assert_eq!(list[3], Fact { label: "Agent", value: "codex".into(), pull: None });
    assert_eq!(list[4].value, "routine/x");
    assert!(list[2].value.ends_with("· 1h ago"), "{}", list[2].value);
}

#[test]
fn an_entry_from_before_0_13_51_says_only_when_it_was_scheduled() {
    let list = facts(&entry(), None, TODAY, entry().at);
    let labels: Vec<&str> = list.iter().map(|f| f.label).collect();
    assert_eq!(labels, ["Scheduled", "Agent"]);
    assert_eq!(list[1].value, "claude");
}

#[test]
fn a_flows_result_names_its_orchestrators_agent() {
    let mut run = entry();
    run.flow = Some("review".into());
    let kept = Kept { agent: Some("claude".into()), ..Kept::default() };
    assert_eq!(facts(&run, Some(&kept), TODAY, run.at)[2].value, "claude");
    assert_eq!(facts(&run, None, TODAY, run.at)[1].value, "flow review");
}

#[test]
fn a_message_heading_reads_its_role_and_local_time() {
    let stamp = Some("2026-11-12T10:30:00.000Z");
    assert!(said_heading(&said("prompt", "x", stamp, false), TODAY).starts_with("Prompt · "));
    assert!(said_heading(&said("reply", "x", stamp, false), TODAY).starts_with("Reply · "));
    assert_eq!(said_heading(&said("reply", "x", None, false), TODAY), "Reply");
    assert_eq!(said_heading(&said("reply", "x", Some("soon"), false), TODAY), "Reply");
}

#[test]
fn the_text_view_reads_as_the_command_line_prints_it() {
    let messages =
        [said("prompt", "fix it\nnow", Some("2026-11-12T10:30:00.000Z"), false), said("reply", "done", None, true)];
    let lines = talk_lines(&messages, true);
    assert_eq!(lines[..2], ["(earlier messages are left out)", ""]);
    assert!(lines[2].starts_with("Prompt · 2026-11-12 "), "{}", lines[2]);
    assert_eq!(lines[3..5], ["fix it", "now"]);
    assert_eq!(lines[5], "");
    assert_eq!(lines[6..], ["Reply", "done", "… cut at 32 KiB", ""]);
}

#[test]
fn copy_takes_what_is_shown() {
    let answer = Answer::read(fixture!("queue/show-conversation.json"), true).expect("the fixture");
    let message = answer.result.as_ref().map(|r| r.text.clone()).expect("a result");
    assert_eq!(copied(&answer, Page::Message, false), Some(message));
    let Some(Talk::Said(talk)) = &answer.talk else { panic!("a conversation") };
    assert_eq!(copied(&answer, Page::Conversation, false), Some(talk.lines.join("\n")));
}

#[test]
fn times_read_as_a_day_and_how_long_ago() {
    assert!(time_words(0, TODAY, 7200).ends_with("· 2h ago"));
}

#[test]
fn nothing_to_copy_is_none_and_never_the_message_for_the_conversation() {
    let custom = Answer::read(fixture!("queue/show-custom.json"), false).expect("the fixture");
    assert_eq!(copied(&custom, Page::Message, false), None);
    let plain = Answer::read(fixture!("queue/show-result.json"), false).expect("the fixture");
    assert_eq!(copied(&plain, Page::Conversation, false), None);
}

fn plain() -> Answer {
    Answer::read(fixture!("queue/show-result.json"), false).expect("the fixture")
}

fn talking() -> Answer {
    Answer::read(fixture!("queue/show-conversation.json"), true).expect("the fixture")
}

#[test]
fn a_plain_answer_keeps_the_conversation_shown_and_a_newer_conversation_replaces_it() {
    let (kept, changed) = merged(Some(talking()), plain(), Keep { facts: true, talk: false });
    assert!(matches!(kept.talk, Some(Talk::Said(_))));
    assert!(!changed);
    let mut again = talking();
    again.talk = Some(Talk::Gone);
    let (replaced, changed) = merged(Some(talking()), again, Keep { facts: true, talk: true });
    assert_eq!(replaced.talk, Some(Talk::Gone));
    assert!(changed);
    assert!(merged(None, plain(), Keep { facts: true, talk: false }).1);
}

#[test]
fn a_conversation_alone_replaces_only_the_conversation() {
    let mut newer = talking();
    newer.talk = Some(Talk::Gone);
    let (shown, changed) = with_talk(plain(), newer);
    assert_eq!(shown.talk, Some(Talk::Gone));
    assert!(changed);
}

#[test]
fn the_page_goes_back_to_the_last_message_when_the_conversation_is_gone() {
    assert_eq!(page_after(&talking(), Page::Conversation), Page::Conversation);
    let gone = Answer::read(fixture!("queue/show-gone.json"), true).expect("the fixture");
    assert_eq!(page_after(&gone, Page::Conversation), Page::Message);
    let mut closed = talking();
    closed.result.as_mut().expect("a result").conversation = false;
    assert_eq!(page_after(&closed, Page::Conversation), Page::Message);
}

#[test]
fn the_notes_say_the_server_s_note_and_what_was_cut_on_the_page_it_belongs_to() {
    assert_eq!(notes(None, Page::Message), Vec::<String>::new());
    let gone = Answer::read(fixture!("queue/show-gone.json"), true).expect("the fixture");
    assert_eq!(notes(Some(&gone), Page::Message), ["Claude no longer has this conversation's record"]);
    let mut cut = plain();
    cut.result.as_mut().expect("a result").cut = true;
    assert_eq!(notes(Some(&cut), Page::Message), [super::CUT_NOTE]);
    assert_eq!(notes(Some(&cut), Page::Conversation), Vec::<String>::new());
    let mut earlier = talking();
    if let Some(Talk::Said(said)) = earlier.talk.as_mut() {
        said.earlier = true;
    }
    assert_eq!(notes(Some(&earlier), Page::Conversation), [super::EARLIER_NOTE]);
    assert_eq!(notes(Some(&earlier), Page::Message), Vec::<String>::new());
}

#[test]
fn copy_is_offered_exactly_when_there_is_text_to_copy() {
    let custom = Answer::read(fixture!("queue/show-custom.json"), false).expect("the fixture");
    for answer in [plain(), talking(), custom] {
        for page in [Page::Message, Page::Conversation] {
            assert_eq!(can_copy(&answer, page), copied(&answer, page, false).is_some());
        }
    }
}

#[test]
fn the_pull_request_comes_after_the_agent_and_before_the_branch() {
    let mut run = entry();
    run.place.branch = Some("fix".into());
    let pr = crate::model::PullRequest { number: 482, title: "Fix the login".into(), ..Default::default() };
    let kept = Kept { pull_request: Some(pr.clone()), ..Kept::default() };
    let list = facts(&run, Some(&kept), TODAY, run.at);
    let labels: Vec<&str> = list.iter().map(|f| f.label).collect();
    assert_eq!(labels, ["Scheduled", "Result", "Agent", "Pull request", "Branch"]);
    assert_eq!(list[3].pull, Some(pr));
    assert!(facts(&run, None, TODAY, run.at).iter().all(|f| f.pull.is_none()));
}

#[test]
fn a_tab_is_copied_as_the_spaces_the_text_view_shows() {
    let text = json!({ "id": 1, "result": { "text": "a\tb", "conversation": true } }).to_string();
    let answer = Answer::read(&text, false).expect("it reads");
    assert_eq!(copied(&answer, Page::Message, false), Some("a\tb".to_string()));
    assert_eq!(copied(&answer, Page::Message, true), Some("a    b".to_string()));
}

#[test]
fn a_gone_record_can_be_asked_again_only_while_the_result_says_it_has_one() {
    let text = json!({ "id": 1, "result": { "text": "x", "conversation": true }, "messages": null }).to_string();
    let answer = Answer::read(&text, true).expect("it reads");
    assert!(can_retry_talk(&answer));
    let gone = Answer::read(fixture!("queue/show-gone.json"), true).expect("the fixture");
    assert!(!can_retry_talk(&gone));
    assert!(!can_retry_talk(&plain()));
}

#[test]
fn a_retry_shows_the_conversation_as_loading_until_it_answers() {
    let text = json!({ "id": 1, "result": { "text": "x", "conversation": true }, "messages": null }).to_string();
    let mut answer = Answer::read(&text, true).expect("it reads");
    assert!(can_retry_talk(&answer));
    answer.forget_talk();
    assert_eq!(answer.talk, None);
}
