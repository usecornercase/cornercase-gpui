use super::*;
use crate::test_util::fixture;

fn agents() -> Vec<AgentInfo> {
    agents_of(fixture!("bots/agents.json")).expect("fixture")
}

fn by<'a>(list: &'a [AgentInfo], name: &str) -> &'a AgentInfo {
    list.iter().find(|a| a.agent == name).expect("agent")
}

fn typed(name: &str) -> Typed {
    Typed { name: name.to_string(), access: Some(CORNERCASE.to_string()), ..Typed::default() }
}

fn strs(words: &[&str]) -> Vec<String> {
    words.iter().map(ToString::to_string).collect()
}

#[test]
fn the_agents_are_read_with_every_field() {
    let list = agents();
    let claude = by(&list, "claude");
    assert!(claude.bot);
    assert_eq!(claude.why, None);
    assert_eq!(claude.accesses, strs(&["cornercase", "full"]));
    assert_eq!(claude.efforts.len(), 5);
    assert_eq!(claude.models, strs(&["opus", "sonnet", "haiku"]));
    assert_eq!(list.len(), 8);
}

#[test]
fn an_agent_that_cannot_be_a_bot_keeps_its_why() {
    let list = agents();
    assert_eq!(by(&list, "codex").why.as_deref(), Some("not installed"));
    assert_eq!(by(&list, "gemini").why.as_deref(), Some("not yet"));
    assert!(!by(&list, "gemini").bot);
}

#[test]
fn an_agent_with_one_access_keeps_it_alone() {
    let list = agents_of(fixture!("bots/agents-codex.json")).expect("fixture");
    let codex = by(&list, "codex");
    assert_eq!(codex.accesses, strs(&["full"]));
    assert!(codex.bot);
}

#[test]
fn an_agent_without_models_or_efforts_offers_none() {
    let list = agents_of(r#"{"agents":[{"agent":"x","bot":true}]}"#).expect("inline");
    assert!(!list[0].takes_effort());
    assert_eq!(list[0].models.len(), 0);
    let codex = agents_of(fixture!("bots/agents-codex.json")).expect("fixture");
    assert!(by(&codex, "codex").takes_effort());
}

#[test]
fn an_unknown_agent_and_an_unknown_field_are_read() {
    let text =
        r#"{"agents":[{"agent":"hermes","title":"Hermes","bot":true,"accesses":["cornercase"],"colour":"red"}]}"#;
    let list = agents_of(text).expect("inline");
    assert_eq!(list[0].agent, "hermes");
    assert!(list[0].bot);
}

#[test]
fn a_missing_list_or_one_that_is_not_a_list_is_an_error() {
    assert!(agents_of("{}").is_err());
    assert!(agents_of(r#"{"agents": 3}"#).is_err());
}

#[test]
fn an_entry_that_does_not_read_is_left_out() {
    let list = agents_of(r#"{"agents":[{"agent":5},{"agent":"claude","bot":true},{"title":"x"}]}"#).expect("inline");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].agent, "claude");
}

#[test]
fn the_servers_default_agent_is_chosen_when_it_can_be_a_bot_through_cornercase() {
    let list = agents_of(r#"{"agents":[{"agent":"a","bot":true,"accesses":["cornercase"]},{"agent":"b","bot":true,"accesses":["cornercase","full"]}]}"#).expect("inline");
    assert_eq!(chosen_at_open(&list, Some("b")), Some(1));
}

#[test]
fn a_default_agent_with_full_access_only_gives_way_to_one_that_offers_cornercase() {
    let list = agents_of(fixture!("bots/agents-codex.json")).expect("fixture");
    assert_eq!(chosen_at_open(&list, Some("codex")), Some(0));
}

#[test]
fn full_access_only_agents_are_chosen_when_nothing_else_can_be_a_bot() {
    let list = agents_of(r#"{"agents":[{"agent":"codex","bot":true,"accesses":["full"]}]}"#).expect("inline");
    assert_eq!(chosen_at_open(&list, Some("codex")), Some(0));
}

#[test]
fn the_id_of_a_new_bot_is_read_from_its_answer() {
    assert_eq!(id_of(r#"{"id":"prs","name":"PRs"}"#).as_deref(), Some("prs"));
    assert_eq!(id_of(r#"{"name":"PRs"}"#), None);
    assert_eq!(id_of(r#"{"id":""}"#), None);
    assert_eq!(id_of("not json"), None);
}

#[test]
fn else_the_first_agent_that_can_be_a_bot_is_chosen() {
    let list = agents();
    assert_eq!(chosen_at_open(&list, Some("codex")), Some(0));
    assert_eq!(chosen_at_open(&list, None), Some(0));
}

#[test]
fn no_agent_is_chosen_when_none_can_be_a_bot() {
    let list = agents_of(r#"{"agents":[{"agent":"codex"}]}"#).expect("inline");
    assert_eq!(chosen_at_open(&list, Some("codex")), None);
}

#[test]
fn a_name_alone_sends_every_other_field_at_its_default() {
    let list = agents();
    let args = new_args(by(&list, "claude"), &typed("PRs")).expect("sent");
    assert_eq!(args, strs(&["bot", "new", "--json", "--agent=claude", "--access=cornercase", "--", "PRs"]));
}

#[test]
fn each_field_set_is_sent_as_an_option() {
    let list = agents();
    let all = Typed {
        name: "PRs".into(),
        model: " opus ".into(),
        effort: Some("high".into()),
        account: Some("/tmp/made-up".into()),
        access: Some(FULL.into()),
        about: "line one\nline two".into(),
    };
    let args = new_args(by(&list, "claude"), &all).expect("sent");
    let want = [
        "bot",
        "new",
        "--json",
        "--agent=claude",
        "--model=opus",
        "--effort=high",
        "--account=/tmp/made-up",
        "--access=full",
        "--about=line one\nline two",
        "--",
        "PRs",
    ];
    assert_eq!(args, strs(&want));
}

#[test]
fn a_name_that_starts_with_a_dash_goes_after_the_double_dash() {
    let list = agents();
    let args = new_args(by(&list, "claude"), &typed("-x")).expect("sent");
    assert_eq!(&args[args.len() - 2..], strs(&["--", "-x"]));
}

#[test]
fn a_name_is_sent_trimmed_and_one_of_blanks_sends_nothing() {
    let list = agents();
    let claude = by(&list, "claude");
    assert_eq!(new_args(claude, &typed("  PRs ")).expect("sent").last().map(String::as_str), Some("PRs"));
    assert_eq!(new_args(claude, &typed("   ")), Err(NotSent::Blank));
}

#[test]
fn access_is_never_chosen_when_cornercase_is_not_offered() {
    let codex = agents_of(fixture!("bots/agents-codex.json")).expect("fixture");
    assert_eq!(by(&codex, "codex").first_access(), None);
    assert_eq!(by(&agents(), "claude").first_access().as_deref(), Some("cornercase"));
}

#[test]
fn create_sends_nothing_without_an_access() {
    let list = agents();
    let none = Typed { name: "PRs".into(), ..Typed::default() };
    assert_eq!(new_args(by(&list, "claude"), &none), Err(NotSent::Access));
}

#[test]
fn effort_is_sent_only_for_an_agent_that_takes_one() {
    let bare = AgentInfo { agent: "x".into(), bot: true, ..AgentInfo::default() };
    let t = Typed { effort: Some("high".into()), ..typed("PRs") };
    let args = new_args(&bare, &t).expect("sent");
    assert!(!args.iter().any(|a| a.starts_with("--effort")));
}

#[test]
fn an_about_with_line_breaks_goes_whole_and_one_of_blanks_is_left_out() {
    let list = agents();
    let claude = by(&list, "claude");
    let with = Typed { about: "a\n\nb\n".into(), ..typed("PRs") };
    assert!(new_args(claude, &with).expect("sent").contains(&"--about=a\n\nb\n".to_string()));
    let blank = Typed { about: " \n ".into(), ..typed("PRs") };
    assert!(!new_args(claude, &blank).expect("sent").iter().any(|a| a.starts_with("--about")));
}

#[test]
fn an_agent_that_cannot_be_a_bot_is_not_sent_and_is_labelled_with_its_why() {
    let list = agents();
    let codex = by(&list, "codex");
    assert_eq!(codex.label(), "Codex · not installed");
    assert_eq!(new_args(codex, &typed("PRs")), Err(NotSent::NotABot("Codex cannot be a bot: not installed".into())));
}

#[test]
fn a_model_hint_names_the_first_suggestion() {
    let list = agents();
    assert_eq!(by(&list, "claude").model_hint(), "the agent's own, such as opus");
    assert_eq!(by(&list, "codex").model_hint(), "the agent's own");
}

#[test]
fn an_agent_with_full_access_only_says_so() {
    let codex = agents_of(fixture!("bots/agents-codex.json")).expect("fixture");
    let line = by(&codex, "codex").full_only_line();
    assert_eq!(line.as_deref(), Some("Codex can be a bot with full access only: its sandbox cannot reach cornercase."));
    assert_eq!(by(&agents(), "claude").full_only_line(), None);
}
