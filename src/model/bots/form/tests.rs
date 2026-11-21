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
        chat_apps: None,
        follow: None,
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

#[test]
fn an_agent_offering_neither_access_has_no_full_only_line() {
    let none = AgentInfo { agent: "x".into(), bot: true, ..AgentInfo::default() };
    assert_eq!(none.full_only_line(), None);
}

#[test]
fn an_empty_account_is_not_sent() {
    let list = agents();
    let t = Typed { account: Some(String::new()), ..typed("PRs") };
    let args = new_args(by(&list, "claude"), &t).expect("sent");
    assert!(!args.iter().any(|a| a.starts_with("--account")));
}

#[test]
fn without_a_default_the_first_agent_through_cornercase_is_chosen_before_a_full_only_one() {
    let text = r#"{"agents":[{"agent":"a","bot":true,"accesses":["full"]},{"agent":"b","bot":true,"accesses":["cornercase","full"]}]}"#;
    let list = agents_of(text).expect("inline");
    assert_eq!(chosen_at_open(&list, None), Some(1));
}

fn full_bot() -> Bot {
    Bot {
        id: "prs".into(),
        name: "PRs".into(),
        agent: "claude".into(),
        model: Some("opus".into()),
        effort: Some("high".into()),
        account: Some("made-up-1".into()),
        access: "full".into(),
        chat_apps: "always".into(),
        ..Bot::default()
    }
}

fn bare_bot() -> Bot {
    Bot { id: "prs".into(), name: "PRs".into(), agent: "claude".into(), access: "cornercase".into(), ..Bot::default() }
}

fn claude() -> AgentInfo {
    by(&agents(), "claude").clone()
}

fn plain_agent() -> AgentInfo {
    AgentInfo { agent: "x".into(), bot: true, ..AgentInfo::default() }
}

fn edit_of(bot: &Bot) -> (Editing, Typed) {
    let editing = Editing::of(bot);
    let now = editing.before.clone();
    (editing, now)
}

#[test]
fn an_edited_bot_opens_with_every_option_it_has() {
    let before = Editing::of(&full_bot()).before;
    assert_eq!(before.name, "PRs");
    assert_eq!(before.model, "opus");
    assert_eq!(before.effort.as_deref(), Some("high"));
    assert_eq!(before.account.as_deref(), Some("made-up-1"));
    assert_eq!(before.access.as_deref(), Some("full"));
    assert_eq!(before.chat_apps.as_deref(), Some("always"));
}

#[test]
fn an_edited_bot_with_no_option_opens_on_the_agents_own() {
    let before = Editing::of(&bare_bot()).before;
    assert_eq!((before.model.as_str(), before.effort, before.account), ("", None, None));
    assert_eq!(before.access.as_deref(), Some("cornercase"));
}

#[test]
fn an_access_the_form_does_not_know_opens_with_none_chosen() {
    for access in ["", "sandbox"] {
        let bot = Bot { access: access.into(), ..bare_bot() };
        assert_eq!(Editing::of(&bot).before.access, None);
    }
}

#[test]
fn nothing_changed_sends_nothing() {
    let (editing, now) = edit_of(&full_bot());
    assert_eq!(editing.args(&claude(), &now), None);
    assert_eq!(editing.save(false, &claude(), &now), Save::Closes);
}

#[test]
fn a_new_name_is_sent_trimmed_as_the_only_option() {
    let (editing, mut now) = edit_of(&full_bot());
    now.name = "  PR reviews ".into();
    assert_eq!(editing.args(&claude(), &now), Some(strs(&["bot", "edit", "prs", "--json", "--name=PR reviews"])));
}

#[test]
fn a_blank_name_is_no_change() {
    let (editing, mut now) = edit_of(&full_bot());
    now.name = "   ".into();
    assert_eq!(editing.args(&claude(), &now), None);
}

#[test]
fn a_name_that_starts_with_a_dash_arrives_whole() {
    let (editing, mut now) = edit_of(&full_bot());
    now.name = "-x".into();
    assert_eq!(editing.args(&claude(), &now).map(|args| args[4].clone()), Some("--name=-x".to_string()));
}

#[test]
fn a_model_set_or_put_back_to_the_agents_own_is_sent() {
    let (editing, mut now) = edit_of(&bare_bot());
    now.model = "sonnet".into();
    assert_eq!(editing.args(&claude(), &now).map(|a| a[4].clone()), Some("--model=sonnet".to_string()));
    let (editing, mut now) = edit_of(&full_bot());
    now.model = " ".into();
    assert_eq!(editing.args(&claude(), &now).map(|a| a[4].clone()), Some("--model=".to_string()));
}

#[test]
fn an_effort_set_or_put_back_to_the_agents_own_is_sent() {
    let (editing, mut now) = edit_of(&bare_bot());
    now.effort = Some("low".into());
    assert_eq!(editing.args(&claude(), &now).map(|a| a[4].clone()), Some("--effort=low".to_string()));
    let (editing, mut now) = edit_of(&full_bot());
    now.effort = None;
    assert_eq!(editing.args(&claude(), &now).map(|a| a[4].clone()), Some("--effort=".to_string()));
}

#[test]
fn an_effort_is_never_sent_for_an_agent_without_efforts() {
    let (editing, mut now) = edit_of(&full_bot());
    now.effort = None;
    assert_eq!(editing.args(&plain_agent(), &now), None);
}

#[test]
fn an_account_is_sent_only_when_another_was_picked() {
    let (editing, mut now) = edit_of(&full_bot());
    now.account = None;
    assert_eq!(editing.args(&claude(), &now), None);
    now.account = Some("made-up-2".into());
    assert_eq!(editing.args(&claude(), &now).map(|a| a[4].clone()), Some("--account=made-up-2".to_string()));
    now.account = Some(String::new());
    assert_eq!(editing.args(&claude(), &now).map(|a| a[4].clone()), Some("--account=".to_string()));
    now.account = Some("made-up-1".into());
    assert_eq!(editing.args(&claude(), &now), None);
}

#[test]
fn an_access_is_sent_only_when_it_changed() {
    let (editing, mut now) = edit_of(&full_bot());
    now.access = Some("cornercase".into());
    assert_eq!(editing.args(&claude(), &now).map(|a| a[4].clone()), Some("--access=cornercase".to_string()));
    now.access = Some("full".into());
    assert_eq!(editing.args(&claude(), &now), None);
    let unknown = Bot { access: String::new(), ..full_bot() };
    let (editing, mut now) = edit_of(&unknown);
    assert_eq!(editing.args(&claude(), &now), None);
    now.access = Some("full".into());
    assert_eq!(editing.args(&claude(), &now).map(|a| a[4].clone()), Some("--access=full".to_string()));
}

#[test]
fn two_changes_go_in_one_command_in_a_fixed_order() {
    let (editing, mut now) = edit_of(&bare_bot());
    now.model = "sonnet".into();
    now.name = "x".into();
    let want = strs(&["bot", "edit", "prs", "--json", "--name=x", "--model=sonnet"]);
    assert_eq!(editing.args(&claude(), &now), Some(want));
}

#[test]
fn an_edit_never_sends_the_agent_or_what_the_bot_is_for() {
    let (editing, _) = edit_of(&bare_bot());
    let now = Typed {
        name: "x".into(),
        model: "sonnet".into(),
        effort: Some("low".into()),
        account: Some("made-up-2".into()),
        access: Some("full".into()),
        about: "x".into(),
        chat_apps: None,
        follow: None,
    };
    let args = editing.args(&claude(), &now).expect("changes");
    assert!(args.iter().all(|word| !word.starts_with("--agent") && !word.starts_with("--about")), "{args:?}");
    assert_eq!(args.len(), 9);
}

#[test]
fn an_edit_made_elsewhere_is_not_undone_by_a_field_nobody_touched() {
    let (editing, mut now) = edit_of(&full_bot());
    now.name = "x".into();
    assert_eq!(editing.args(&claude(), &now), Some(strs(&["bot", "edit", "prs", "--json", "--name=x"])));
}

#[test]
fn a_second_save_while_the_first_runs_sends_nothing() {
    let (editing, mut now) = edit_of(&full_bot());
    now.name = "x".into();
    assert_eq!(editing.save(true, &claude(), &now), Save::Waits);
}

#[test]
fn a_save_with_a_change_sends_it() {
    let (editing, mut now) = edit_of(&full_bot());
    now.name = "x".into();
    let want = Save::Sends(strs(&["bot", "edit", "prs", "--json", "--name=x"]));
    assert_eq!(editing.save(false, &claude(), &now), want);
}

#[test]
fn a_rename_is_one_name_through_bot_edit() {
    assert_eq!(rename_args("prs", "PR reviews"), strs(&["bot", "edit", "prs", "--json", "--name=PR reviews"]));
    assert_eq!(rename_args("prs", "-x"), strs(&["bot", "edit", "prs", "--json", "--name=-x"]));
}

#[test]
fn a_new_bot_starts_every_agent_on_its_own_login() {
    let kept = kept_accounts(&agents(), None);
    assert_eq!(kept.len(), 8);
    assert!(kept.values().all(String::is_empty));
}

#[test]
fn an_edited_bots_picker_starts_on_its_account() {
    let editing = Editing::of(&full_bot());
    let kept = kept_accounts(&agents(), Some(&editing));
    assert_eq!(kept.get("claude").map(String::as_str), Some("made-up-1"));
    assert_eq!(kept.iter().filter(|(_, account)| !account.is_empty()).count(), 1);
}

fn newer() -> Vec<AgentInfo> {
    agents_of(fixture!("bots/agents-reads.json")).expect("fixture")
}

const READS_ANYTHING_NOTE: &str = "Through cornercase, it reads anything and writes only in its own folder.";

#[test]
fn an_agent_that_reads_only_its_folder_says_so_under_access() {
    let list = newer();
    for name in ["gemini", "copilot"] {
        assert_eq!(by(&list, name).through_note(), "Through cornercase, it reads and writes only in its own folder.");
    }
}

#[test]
fn an_agent_that_reads_anything_keeps_the_sentence_it_had() {
    let list = newer();
    for name in ["claude", "codex", "opencode", "cursor"] {
        assert_eq!(by(&list, name).through_note(), READS_ANYTHING_NOTE);
    }
}

#[test]
fn an_older_servers_agents_read_anything() {
    assert!(agents().iter().all(|agent| agent.through_note() == READS_ANYTHING_NOTE));
}

#[test]
fn a_reads_of_another_type_or_value_reads_anything_and_the_agent_stays() {
    let cases = [r"5", "null", "true", r#"["folder"]"#, r#"{"folder":true}"#, r#""anything""#, r#""Folder""#, r#""""#];
    let entries: Vec<String> = cases
        .iter()
        .enumerate()
        .map(|(at, reads)| format!(r#"{{"agent":"a{at}","bot":true,"reads":{reads}}}"#))
        .collect();
    let list = agents_of(&format!(r#"{{"agents":[{}]}}"#, entries.join(","))).expect("list");
    assert_eq!(list.len(), cases.len());
    assert!(list.iter().all(|agent| agent.through_note() == READS_ANYTHING_NOTE));
}

#[test]
fn the_agents_of_a_newer_server_are_read_with_their_reasons() {
    let list = newer();
    assert_eq!(
        by(&list, "opencode").full_only_line().as_deref(),
        Some("opencode can be a bot with full access only: a command can write outside its folder.")
    );
    assert_eq!(
        by(&list, "cursor").full_only_line().as_deref(),
        Some("Cursor CLI can be a bot with full access only: its edits can leave its folder.")
    );
    assert_eq!(by(&list, "gemini").full_only_line(), None);
}

#[test]
fn a_default_agent_that_reads_only_its_folder_is_chosen_at_open() {
    let list = newer();
    assert_eq!(chosen_at_open(&list, Some("gemini")), list.iter().position(|a| a.agent == "gemini"));
}

#[test]
fn the_form_says_a_bot_sets_itself_up_from_what_it_is_for() {
    let note = about_note(Has::Yes).expect("a note");
    assert!(note.contains("sets itself up") && note.contains("starts nothing"), "{note}");
}

#[test]
fn the_form_says_nothing_of_a_setup_to_a_server_without_it() {
    assert_eq!(about_note(Has::No(Some(crate::cli::Version::new(0, 13, 88)))), None);
    assert_eq!(about_note(Has::No(None)), None);
    assert_eq!(about_note(Has::Unread), None);
}

#[test]
fn a_new_bot_sends_the_chat_apps_choice_after_the_access() {
    let list = agents();
    let t = Typed { chat_apps: Some("always".into()), about: "x".into(), ..typed("PRs") };
    let args = new_args(by(&list, "claude"), &t).expect("sent");
    let want = [
        "bot",
        "new",
        "--json",
        "--agent=claude",
        "--access=cornercase",
        "--chat-apps=always",
        "--about=x",
        "--",
        "PRs",
    ];
    assert_eq!(args, strs(&want));
}

#[test]
fn a_new_bot_with_no_chat_apps_choice_sends_none() {
    let list = agents();
    let args = new_args(by(&list, "claude"), &typed("PRs")).expect("sent");
    assert!(args.iter().all(|word| !word.starts_with("--chat-apps")), "{args:?}");
}

#[test]
fn an_edited_bot_opens_with_its_chat_apps_option() {
    for option in ["away", "always", "off"] {
        let bot = Bot { chat_apps: option.into(), ..bare_bot() };
        assert_eq!(Editing::of(&bot).before.chat_apps.as_deref(), Some(option));
    }
}

#[test]
fn a_chat_apps_value_the_form_does_not_know_opens_with_none_chosen() {
    for option in ["", "sometimes"] {
        let bot = Bot { chat_apps: option.into(), ..bare_bot() };
        assert_eq!(Editing::of(&bot).before.chat_apps, None);
    }
}

#[test]
fn a_chat_apps_choice_is_sent_only_when_it_changed() {
    let (editing, mut now) = edit_of(&full_bot());
    assert_eq!(editing.args(&claude(), &now), None);
    now.chat_apps = Some("off".into());
    assert_eq!(editing.args(&claude(), &now), Some(strs(&["bot", "edit", "prs", "--json", "--chat-apps=off"])));
    now.chat_apps = None;
    assert_eq!(editing.args(&claude(), &now), None);
}

#[test]
fn a_chat_apps_change_goes_last_after_the_access() {
    let (editing, mut now) = edit_of(&bare_bot());
    now.chat_apps = Some("always".into());
    now.access = Some("full".into());
    now.name = "x".into();
    let want = strs(&["bot", "edit", "prs", "--json", "--name=x", "--access=full", "--chat-apps=always"]);
    assert_eq!(editing.args(&claude(), &now), Some(want));
}

#[test]
fn create_sends_follow_after_the_access_when_the_box_shows() {
    let list = agents();
    for (on, word) in [(true, "--follow=on"), (false, "--follow=off")] {
        let t = Typed { follow: Some(on), chat_apps: Some("away".into()), ..typed("PRs") };
        let args = new_args(by(&list, "claude"), &t).expect("sent");
        let want =
            ["bot", "new", "--json", "--agent=claude", "--access=cornercase", word, "--chat-apps=away", "--", "PRs"];
        assert_eq!(args, strs(&want));
    }
}

#[test]
fn create_sends_no_follow_when_the_row_is_hidden() {
    let list = agents();
    let args = new_args(by(&list, "claude"), &typed("PRs")).expect("sent");
    assert!(args.iter().all(|word| !word.starts_with("--follow")), "{args:?}");
}

#[test]
fn save_sends_follow_only_when_it_changed() {
    let (editing, now) = edit_of(&Bot { follow: Some(true), ..bare_bot() });
    assert_eq!(editing.args(&claude(), &now), None);
    let off = Typed { follow: Some(false), ..now.clone() };
    assert_eq!(editing.args(&claude(), &off), Some(strs(&["bot", "edit", "prs", "--json", "--follow=off"])));
    let hidden = Typed { follow: None, ..now };
    assert_eq!(editing.args(&claude(), &hidden), None);
}

#[test]
fn save_sends_no_follow_when_the_server_gave_none() {
    let (editing, now) = edit_of(&bare_bot());
    assert_eq!(editing.before.follow, None);
    let ticked = Typed { follow: Some(true), ..now };
    assert_eq!(editing.args(&claude(), &ticked), None);
}
