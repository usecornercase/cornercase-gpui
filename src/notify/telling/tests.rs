use super::*;
use crate::notify::{Event, parse_event};

fn message(name: &str) -> Message {
    let text = match name {
        "message" => crate::test_util::fixture!("events/message.json"),
        "message-workspace" => crate::test_util::fixture!("events/message-workspace.json"),
        "message-failed" => crate::test_util::fixture!("events/message-failed.json"),
        "message-freed" => crate::test_util::fixture!("events/message-freed.json"),
        _ => unreachable!(),
    };
    let Some(Event::Message(message)) = parse_event(text) else { panic!("not a message") };
    message
}

fn of_kind(kind: &str, panes: &[u64], workspace: Option<u64>) -> Message {
    Message {
        kind: kind.into(),
        text: "a sentence from cornercase".into(),
        panes: panes.to_vec(),
        workspace,
        failed: false,
    }
}

fn telling(kind: &str) -> Telling {
    Telling::of_message(&of_kind(kind, &[4], Some(2)), None)
}

fn session() -> Session {
    serde_json::from_value(serde_json::json!({
        "boot": "b", "revision": 1, "version": "0.13.65", "groups": [],
        "projects": [{"id": 1, "name": "shop", "path": "/p", "workspaces": [{
            "id": 2, "name": "fix-login", "path": "/p", "tabs": []}]}]
    }))
    .expect("a session")
}

const KINDS: [&str; 8] = [
    "notify",
    "queue-started",
    "queue-failed",
    "queue-missed",
    "queue-kept",
    "setup-failed",
    "flow-consent",
    "flow-merged",
];

fn run(parts: &[&str]) -> ToastGo {
    ToastGo::Run(args(parts))
}

#[test]
fn each_kind_is_read_from_cornercases_id() {
    let kinds = KINDS.map(Kind::of);
    assert_eq!(
        kinds,
        [
            Kind::Notify,
            Kind::QueueStarted,
            Kind::QueueFailed,
            Kind::QueueMissed,
            Kind::QueueKept,
            Kind::SetupFailed,
            Kind::FlowConsent,
            Kind::FlowMerged
        ]
    );
    assert_eq!(Kind::of(""), Kind::Other);
    assert_eq!(Kind::of("tunnel-something"), Kind::Other);
}

#[test]
fn each_kind_becomes_its_toast() {
    let toasts: Vec<ToastKind> = KINDS.iter().map(|kind| telling(kind).kind).collect();
    let (done, needs) = (ToastKind::Done, ToastKind::NeedsYou);
    assert_eq!(toasts, [done, done, needs, needs, needs, needs, needs, done]);
    assert_eq!(telling("tunnel-something").kind, done);
}

#[test]
fn each_kind_has_its_headline_and_a_merged_run_none() {
    let headlines: Vec<Option<String>> = KINDS.iter().map(|kind| telling(kind).headline).collect();
    let some = |text: &str| Some(text.to_string());
    let expected = [
        some("Message"),
        some("Queue"),
        some("Queue"),
        some("Queue"),
        some("Queue"),
        some("Setup"),
        some("Flow"),
        None,
    ];
    assert_eq!(headlines, expected);
    assert_eq!(telling("tunnel-something").headline, some("Message"));
}

#[test]
fn the_desktop_notification_is_the_headline_over_cornercases_sentence() {
    let failed = telling("queue-failed");
    assert_eq!((failed.headline.as_deref(), failed.text.as_str()), (Some("Queue"), "a sentence from cornercase"));
    let notify = telling("notify");
    assert_eq!((notify.headline.as_deref(), notify.text.as_str()), (Some("Message"), "a sentence from cornercase"));
}

#[test]
fn the_text_is_cornercases_sentence_as_it_comes() {
    let merged = Telling::of_message(&message("message-workspace"), None);
    assert_eq!(merged.text, "pull request merged in fix-login: 2 agents closed · ▶ on the workspace reopens them");
    assert_eq!(Telling::of_message(&message("message"), None).text, "the release build is ready");
}

#[test]
fn no_message_plays_a_sound() {
    assert!(KINDS.iter().chain(&["tunnel-something"]).all(|kind| telling(kind).sound.is_none()));
}

#[test]
fn show_goes_to_the_first_pane_it_names() {
    let telling = Telling::of_message(&of_kind("notify", &[4, 5], Some(2)), None);
    assert_eq!(telling.button, Some(("Show".to_string(), run(&["focus", "--pane", "4"]))));
}

#[test]
fn show_goes_to_the_workspace_when_no_pane_is_named() {
    let telling = Telling::of_message(&of_kind("queue-kept", &[], Some(7)), None);
    assert_eq!(telling.button, Some(("Show".to_string(), run(&["focus", "--workspace", "7"]))));
}

#[test]
fn a_message_that_names_nothing_has_no_button() {
    assert_eq!(Telling::of_message(&message("message"), None).button, None);
    assert_eq!(Telling::of_message(&of_kind("tunnel-something", &[], None), None).button, None);
}

#[test]
fn a_queue_message_that_names_nothing_opens_the_queue() {
    for kind in ["queue-failed", "queue-missed", "queue-kept"] {
        let telling = Telling::of_message(&of_kind(kind, &[], None), None);
        assert_eq!(telling.button, Some(("Open Queue".to_string(), ToastGo::Open(PanelKind::Queue))), "{kind}");
    }
}

#[test]
fn a_merged_run_offers_to_reopen_its_agents_as_a_job() {
    let telling = Telling::of_message(&message("message-workspace"), Some(&session()));
    let (busy, reopen) = crate::store::reopening(2, "fix-login");
    assert_eq!(telling.button, Some(("Reopen agents".to_string(), ToastGo::Job(busy, reopen))));
}

#[test]
fn the_reopen_job_names_the_workspace_even_when_the_session_does_not_know_it() {
    let telling = Telling::of_message(&message("message-workspace"), None);
    let (busy, reopen) = crate::store::reopening(2, "workspace 2");
    assert_eq!(telling.button, Some(("Reopen agents".to_string(), ToastGo::Job(busy, reopen))));
}

#[test]
fn a_merged_run_without_a_workspace_offers_nothing() {
    assert_eq!(Telling::of_message(&of_kind("flow-merged", &[4], None), None).button, None);
}

#[test]
fn a_notice_is_heard_as_before() {
    let Some(Event::Notice(notice)) = parse_event(crate::test_util::fixture!("events/notice.json")) else {
        panic!("not a notice");
    };
    let telling = Telling::of_notice(&notice, None);
    assert_eq!(telling.text, "claude needs you in shop › default");
    assert_eq!(telling.kind, ToastKind::NeedsYou);
    assert_eq!(telling.button, Some(("Go".to_string(), run(&["go", "cc-ab12cd-3"]))));
    assert_eq!(telling.headline.as_deref(), Some("claude needs you"));
    assert_eq!(telling.sound, None);
    let Some(Event::Notice(loud)) = parse_event(crate::test_util::fixture!("events/notice-sound.json")) else {
        panic!("not a notice");
    };
    assert!(Telling::of_notice(&loud, None).sound.is_some());
}

#[test]
fn a_toast_with_no_desktop_notification_waits_while_the_window_is_behind() {
    let merged = telling("flow-merged");
    assert!(merged.waits(false));
    assert!(!merged.waits(true));
    assert!(!telling("notify").waits(false));
}

const LOOKING_AT_THE_QUEUE: Looking = Looking { active: true, queue_open: true };

#[test]
fn a_queue_message_is_not_toasted_while_the_queue_sheet_shows_in_the_active_window() {
    for kind in ["queue-started", "queue-failed", "queue-missed", "queue-kept"] {
        let message = of_kind(kind, &[], None);
        assert!(dropped(&message, &LOOKING_AT_THE_QUEUE, |_| false), "{kind}");
        assert!(!dropped(&message, &Looking { active: true, queue_open: false }, |_| false), "{kind}");
        assert!(!dropped(&message, &Looking { active: false, queue_open: true }, |_| false), "{kind}");
    }
}

#[test]
fn a_failed_setup_is_not_told_while_this_windows_setup_job_for_it_runs() {
    let running = |key: &str| key == setup_key(2);
    let calm = Looking { active: true, queue_open: false };
    assert!(dropped(&of_kind("setup-failed", &[4], Some(2)), &calm, running));
    assert!(!dropped(&of_kind("setup-failed", &[4], Some(3)), &calm, running));
    assert!(!dropped(&of_kind("setup-failed", &[4], None), &calm, running));
}

#[test]
fn notify_and_the_flows_are_always_told() {
    for kind in ["notify", "flow-consent", "flow-merged", "tunnel-something"] {
        assert!(!dropped(&of_kind(kind, &[4], Some(2)), &LOOKING_AT_THE_QUEUE, |_| true), "{kind}");
    }
}

#[test]
fn a_merged_run_is_held_while_the_window_is_behind_and_told_in_order_once_it_is_active() {
    let mut waiting = Waiting::default();
    let first = Telling::of_message(&of_kind("flow-merged", &[4], Some(2)), None);
    let second = Telling::of_message(&of_kind("flow-merged", &[5], Some(3)), None);
    assert_eq!(waiting.hold_or_pass(first.clone(), false), None);
    assert_eq!(waiting.hold_or_pass(second.clone(), false), None);
    assert_eq!(waiting.release(false), Vec::<Telling>::new());
    assert_eq!(waiting.release(true), vec![first, second]);
    assert_eq!(waiting.release(true), Vec::<Telling>::new());
}

#[test]
fn a_message_with_a_notification_is_not_held_and_a_merged_run_is_not_held_while_active() {
    let mut waiting = Waiting::default();
    let notify = telling("notify");
    assert_eq!(waiting.hold_or_pass(notify.clone(), false), Some(notify));
    let merged = telling("flow-merged");
    assert_eq!(waiting.hold_or_pass(merged.clone(), true), Some(merged));
    assert_eq!(waiting.release(true), Vec::<Telling>::new());
}

#[test]
fn what_waits_is_forgotten_when_the_server_starts_a_new_run() {
    let mut waiting = Waiting::default();
    assert_eq!(waiting.hold_or_pass(telling("flow-merged"), false), None);
    waiting.forget();
    assert_eq!(waiting.release(true), Vec::<Telling>::new());
}

#[test]
fn the_queue_sheet_is_what_makes_the_window_look_at_the_queue() {
    assert!(Looking::of(true, Some(PanelKind::Queue)).queue_open);
    assert!(!Looking::of(true, Some(PanelKind::Settings)).queue_open);
    assert!(!Looking::of(true, None).queue_open);
    assert!(Looking::of(true, Some(PanelKind::Queue)).active);
    assert!(!Looking::of(false, None).active);
}

#[test]
fn a_message_reads_the_settings_only_when_the_window_is_behind() {
    assert!(telling("notify").needs_config(true));
    assert!(!telling("notify").needs_config(false));
    assert!(!telling("flow-merged").needs_config(true));
}

#[test]
fn a_notice_reads_the_settings_for_its_sound_and_when_the_window_is_behind() {
    let Some(Event::Notice(notice)) = parse_event(crate::test_util::fixture!("events/notice.json")) else {
        panic!("not a notice");
    };
    let quiet = Telling::of_notice(&notice, None);
    assert!(!quiet.needs_config(false));
    assert!(quiet.needs_config(true));
    let loud = Telling { sound: Some("needs-you".into()), ..quiet };
    assert!(loud.needs_config(false));
}

#[test]
fn a_desktop_notification_is_posted_only_behind_with_the_setting_on_and_a_headline() {
    let notify = telling("notify");
    assert!(notify.posts(true, true));
    assert!(!notify.posts(false, true));
    assert!(!notify.posts(true, false));
    assert!(!telling("flow-merged").posts(true, true));
}

#[test]
fn a_merged_run_whose_free_failed_is_told_as_needing_you() {
    let message = message("message-failed");
    let telling = Telling::of_message(&message, None);
    assert_eq!(telling.kind, ToastKind::NeedsYou);
    assert_eq!(telling.text, message.text);
    let (busy, reopen) = crate::store::reopening(2, "workspace 2");
    assert_eq!(telling.button, Some(("Reopen agents".to_string(), ToastGo::Job(busy, reopen))));
}

#[test]
fn an_older_servers_merged_run_is_still_a_green_toast_with_its_pointer() {
    let telling = Telling::of_message(&message("message-workspace"), None);
    assert_eq!(telling.kind, ToastKind::Done);
    assert!(telling.text.ends_with("▶ on the workspace reopens them"));
}

#[test]
fn a_failed_free_waits_behind_and_posts_no_notification() {
    let telling = Telling::of_message(&message("message-failed"), None);
    assert!(telling.waits(false) && !telling.waits(true));
    assert!(!telling.posts(true, true));
}

#[test]
fn a_failed_ending_of_a_kind_this_window_does_not_know_needs_you() {
    let mut message = of_kind("something-new", &[], None);
    message.failed = true;
    assert_eq!(Telling::of_message(&message, None).kind, ToastKind::NeedsYou);
}

#[test]
fn a_merged_run_that_freed_is_told_as_done_with_its_sentence_as_it_comes() {
    let message = message("message-freed");
    let telling = Telling::of_message(&message, None);
    assert_eq!(telling.kind, ToastKind::Done);
    assert_eq!(telling.text, "pull request merged in fa258: 5 agents closed, 100 KB of builds freed");
    assert!(!message.failed);
}
