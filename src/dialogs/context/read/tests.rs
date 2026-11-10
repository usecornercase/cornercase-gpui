use std::time::Duration;

use anyhow::anyhow;

use super::*;
use crate::cli::Target;
use crate::store::AgentReach;
use crate::test_util::fixture;

fn outcome(text: &str) -> Outcome {
    outcome_of(Ok(text.to_string()))
}

fn settled(text: &str) -> Reads {
    let mut reads = Reads::default();
    reads.start();
    reads.settle(outcome(text), Instant::now());
    reads
}

fn total(reads: &Reads) -> Option<u64> {
    Some(reads.answer()?.breakdown.as_ref()?.total_tokens)
}

#[test]
fn only_the_first_command_asks_for_the_cached_answer() {
    assert_eq!(cached_args(7, None), ["context", "--pane", "7", "--cached", "--json"]);
    assert_eq!(read_args(7, None), ["context", "--pane", "7", "--json"]);
}

#[test]
fn a_read_there_says_which_run_its_pane_is_from() {
    assert_eq!(cached_args(9, Some("b1")), ["context", "--pane", "9", "--cached", "--json", "--boot", "b1"]);
    assert_eq!(read_args(9, Some("b1")), ["context", "--pane", "9", "--json", "--boot", "b1"]);
}

#[test]
fn a_read_here_sends_no_boot() {
    assert!(!read_args(9, None).contains(&"--boot".to_string()));
}

#[test]
fn a_read_refused_for_another_run_says_the_machine_restarted() {
    let stale = cli::failure("context", Some(4), b"this request is for another run");
    let err = stale_read("vps", Err(stale)).expect_err("a refusal");
    assert_eq!(err.to_string(), "vps restarted: nothing was sent");
    let other = stale_read("vps", Err(anyhow!("cornercase context: no such pane"))).expect_err("a failure");
    assert_eq!(other.to_string(), "cornercase context: no such pane");
}

#[test]
fn a_read_follows_the_cached_answer() {
    assert!(settled(fixture!("context/cached-none.json")).read_follows(true));
    assert!(settled(fixture!("context/read.json")).read_follows(true));
    assert!(settled(fixture!("context/failed.json")).read_follows(true));
}

#[test]
fn no_read_follows_once_the_dialog_has_closed() {
    assert!(!settled(fixture!("context/cached-none.json")).read_follows(false));
}

#[test]
fn no_read_follows_an_agent_that_says_nothing_more() {
    assert!(!settled(fixture!("context/other-agent.json")).read_follows(true));
}

#[test]
fn no_read_follows_a_command_that_failed() {
    let mut reads = Reads::default();
    reads.settle(outcome_of(Err(anyhow!("cornercase context: pane 1 runs no agent cornercase reads"))), Instant::now());
    assert!(!reads.read_follows(true));
    assert_eq!(reads.failure(), Some("pane 1 runs no agent cornercase reads"));
}

#[test]
fn a_failed_read_says_why_beside_the_last_numbers() {
    let reads = settled(fixture!("context/failed.json"));
    assert_eq!(reads.failure(), Some("claude exited without answering"));
    assert_eq!(total(&reads), Some(251_853));
}

#[test]
fn an_agent_that_has_not_said_its_conversation_says_why_without_numbers() {
    let reads = settled(fixture!("context/not-yet.json"));
    assert_eq!(reads.failure(), Some("Claude Code has not said yet which conversation it is in"));
    assert_eq!(total(&reads), None);
    assert_eq!(reads.age_note(Instant::now()), None);
}

#[test]
fn a_command_that_fails_keeps_the_numbers_already_shown() {
    let mut reads = settled(fixture!("context/read.json"));
    reads.start();
    reads.settle(outcome_of(Err(anyhow!("cornercase context: the server went away"))), Instant::now());
    assert_eq!(reads.failure(), Some("the server went away"));
    assert_eq!(total(&reads), Some(251_853));
}

#[test]
fn an_answer_that_cannot_be_read_says_so() {
    let Outcome::Failed(why) = outcome("not json") else { panic!("a failure") };
    assert!(why.starts_with("cannot read the answer of cornercase context: "), "{why}");
}

#[test]
fn a_new_read_hides_the_old_failure_until_it_answers() {
    let mut reads = settled(fixture!("context/failed.json"));
    reads.start();
    assert_eq!(reads.failure(), None);
    reads.settle(outcome(fixture!("context/read.json")), Instant::now());
    assert_eq!(reads.failure(), None);
}

#[test]
fn an_answer_ends_an_older_command_failure() {
    let mut reads = Reads::default();
    reads.settle(Outcome::Failed("gone".into()), Instant::now());
    reads.settle(outcome(fixture!("context/read.json")), Instant::now());
    assert_eq!(reads.failure(), None);
}

#[test]
fn only_one_read_runs_at_a_time() {
    let mut reads = Reads::default();
    assert!(reads.can_read());
    reads.start();
    assert!(reads.running() && !reads.can_read());
    reads.settle(outcome(fixture!("context/read.json")), Instant::now());
    assert!(!reads.running() && reads.can_read());
}

#[test]
fn a_first_read_waits_for_numbers_and_the_others_keep_theirs() {
    let reading = |text: &str| {
        let mut reads = settled(text);
        reads.start();
        reads.waits_for_numbers()
    };
    assert!(reading(fixture!("context/cached-none.json")));
    assert!(!reading(fixture!("context/read.json")));
    assert!(!reading(fixture!("context/other-agent.json")));
    assert!(!settled(fixture!("context/cached-none.json")).waits_for_numbers());
}

#[test]
fn an_older_server_or_command_line_reads_as_too_old() {
    let server = "cornercase context: error: the running cornercase server (0.12.236) has no `context` command; \
                  update cornercase and restart the server";
    let client = "cornercase context: error: unrecognized subcommand 'context'\n\nUsage: cornercase <COMMAND>";
    for refusal in [server, client] {
        assert_eq!(outcome_of(Err(anyhow!(refusal))), Outcome::TooOld);
    }
}

#[test]
fn a_server_too_old_is_not_asked_again() {
    let mut reads = Reads::default();
    reads.settle(Outcome::TooOld, Instant::now());
    assert!(reads.too_old() && !reads.can_read() && !reads.read_follows(true));
    assert_eq!(reads.failure(), None);
}

#[test]
fn any_other_refusal_is_a_failure_that_says_why() {
    let refusal = "cornercase context: there is no pane 9; `cornercase status` lists them";
    let why = "there is no pane 9; `cornercase status` lists them";
    assert_eq!(outcome_of(Err(anyhow!(refusal))), Outcome::Failed(why.into()));
}

#[test]
fn the_age_goes_on_after_the_answer() {
    let mut reads = Reads::default();
    let at = Instant::now();
    let text = fixture!("context/read.json").replace("\"age\": 0", "\"age\": 100");
    reads.settle(outcome(&text), at);
    assert_eq!(reads.age_note(at + Duration::from_secs(20)).as_deref(), Some("Read 2m ago"));
}

#[test]
fn an_answer_without_numbers_has_no_age() {
    assert_eq!(settled(fixture!("context/cached-none.json")).age_note(Instant::now()), None);
}

#[test]
fn an_age_reads_in_the_largest_unit_that_fits() {
    let said: Vec<String> = [0, 59, 60, 3_599, 3_600, 86_400].map(read_ago).to_vec();
    assert_eq!(said, ["Read just now", "Read just now", "Read 1m ago", "Read 59m ago", "Read 1h ago", "Read 1d ago"]);
}

#[test]
fn an_answer_that_lands_once_the_tab_fell_is_dropped_for_its_reason() {
    let answer = outcome("{\"title\":\"Claude Code\"}");
    let ready = AgentReach::Ready(Target::Local);
    assert_eq!(landed(answer.clone(), &ready), answer);
    let down = AgentReach::Down("vps unreachable".into());
    assert_eq!(landed(answer.clone(), &down), Outcome::Failed("vps unreachable".into()));
    let gone = AgentReach::Gone("vps restarted".into());
    assert_eq!(landed(answer, &gone), Outcome::Failed("vps restarted".into()));
}

#[test]
fn a_read_there_is_a_job_of_its_own() {
    assert_eq!(job_key(5, None), "context:5");
    assert_eq!(job_key(5, Some(("vps", "b1"))), "context:vps:b1:5");
}

#[test]
fn the_job_and_title_name_the_machine() {
    assert_eq!(job_words(Some("Claude Code"), None), "Reading Claude Code's context…");
    assert_eq!(job_words(None, Some("vps")), "Reading the agent's context on vps…");
    assert_eq!(title(None), "Context");
    assert_eq!(title(Some("vps")), "Context on vps");
}

fn there(sends_boot: bool) -> ContextThere {
    ContextThere { machine: "vps".into(), boot: "b1".into(), home: String::new(), sends_boot }
}

#[test]
fn a_read_there_carries_the_tabs_run_only_where_this_cornercase_takes_it() {
    assert_eq!(boot_for(Some(&there(true))), Some("b1"));
    assert_eq!(boot_for(Some(&there(false))), None);
    assert_eq!(boot_for(None), None);
}
