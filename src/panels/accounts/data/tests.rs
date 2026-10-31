use super::*;

const SAMPLE: &str = r#"{
  "profile": "Work",
  "profiles": [{"name":"Work","accounts":{"claude":"work"},"active":true},{"name":"Home","accounts":{}}],
  "providers": [
    {"accounts":[{"name":"work","path":"/s/accounts/claude/work","unshared":["settings.json"],
                  "login":{"state":"signed-in","email":"a@b.c","plan":"max","checked":"now"},"label":"Work"},
                 {"name":"home","path":"/s/accounts/claude/home","unshared":[],
                  "login":{"state":"signed-out","checked":"now"}}],
     "env":"CLAUDE_CONFIG_DIR","provider":"claude","system_path":"/home/me/.claude","title":"Claude Code",
     "active":"work","system_login":{"state":"weird"}},
    {"accounts":[],"env":"CODEX_HOME","provider":"codex","title":"Codex","system_label":"Mine"}
  ],
  "login": {"project": 1, "workspace": 2, "tab": 3, "pane": 4},
  "tokens": ["ANTHROPIC_API_KEY"]
}"#;

#[test]
fn parses_accounts_and_profiles() {
    let info = parse(SAMPLE).expect("parses");
    assert_eq!(info.profile, "Work");
    assert_eq!(info.providers.len(), 2);
    let claude = &info.providers[0];
    assert_eq!(claude.active, "work");
    assert_eq!(claude.accounts[0].unshared, ["settings.json"]);
    assert_eq!(claude.system_login.as_ref().map(|l| l.state), Some(LoginState::Unknown));
    assert_eq!(info.login.and_then(|i| i.tab), Some(3));
    assert_eq!(info.tokens, ["ANTHROPIC_API_KEY"]);
    assert!(parse(r#"{"providers":[],"profiles":[],"profile":""}"#).is_ok());
}

#[test]
fn logins_read_as_cornercase_says_them() {
    let info = parse(SAMPLE).expect("parses");
    let claude = &info.providers[0];
    assert_eq!(claude.accounts[0].login.as_ref().map(Login::headline).as_deref(), Some("Signed in as a@b.c"));
    assert_eq!(claude.accounts[1].login.as_ref().map(Login::headline).as_deref(), Some("Signed out"));
    assert_eq!(claude.system_login.as_ref().map(Login::headline).as_deref(), Some("Login unknown"));
    let failed = Login { state: LoginState::Failed, ..Login::default() };
    assert_eq!(failed.headline(), "Login check failed");
    let plan_only = Login { state: LoginState::SignedIn, plan: Some("plus".into()), ..Login::default() };
    assert_eq!(plan_only.headline(), "Signed in");
}

#[test]
fn a_known_login_shows_its_state_and_its_plan_apart() {
    let info = parse(SAMPLE).expect("parses");
    let work = info.providers[0].accounts[0].login.as_ref();
    let expected =
        Status::Login { state: LoginState::SignedIn, headline: "Signed in as a@b.c".into(), plan: Some("max".into()) };
    assert_eq!(status(work, true, true), expected);
}

#[test]
fn the_check_answers_for_every_account_and_for_the_own_login_once_there_are_accounts() {
    let info = parse(SAMPLE).expect("parses");
    let (claude, codex) = (&info.providers[0], &info.providers[1]);
    assert!(claude.checks("work"));
    assert!(claude.checks(""));
    assert!(!codex.checks(""));
}

#[test]
fn a_row_without_a_login_says_whether_the_check_is_still_coming() {
    assert_eq!(status(None, true, true), Status::Checking);
    assert_eq!(status(None, true, false), Status::NotChecked);
    assert_eq!(status(None, false, true), Status::Unasked);
}

#[test]
fn folders_under_home_start_with_a_tilde() {
    assert_eq!(home_relative("/Users/ana/.claude", "/Users/ana"), "~/.claude");
    assert_eq!(home_relative("/Users/ana", "/Users/ana"), "~");
    assert_eq!(home_relative("/Users/anabel/.claude", "/Users/ana"), "/Users/anabel/.claude");
    assert_eq!(home_relative("/srv/claude", ""), "/srv/claude");
}

#[test]
fn token_variables_are_named_in_one_sentence() {
    assert_eq!(tokens_title(&["ANTHROPIC_API_KEY".into()]), "ANTHROPIC_API_KEY is set where cornercase runs");
    assert_eq!(tokens_title(&["A".into(), "B".into(), "C".into()]), "A, B and C are set where cornercase runs");
}

#[test]
fn files_that_stopped_being_shared_are_listed() {
    assert_eq!(
        unshared_text(&["settings.json".into()]),
        "No longer shared with the own login: settings.json (now a file of its own)"
    );
    assert_eq!(
        unshared_text(&["settings.json".into(), "CLAUDE.md".into()]),
        "No longer shared with the own login: settings.json and CLAUDE.md (now files of its own)"
    );
}

#[test]
fn an_account_change_names_the_row_it_changes() {
    let account = |agent: &str, folder: &str| Some(Target::Account(agent.into(), folder.into()));
    assert_eq!(target(&add_args("claude", "work", Some(3))), Some(Target::Adding("claude".into())));
    assert_eq!(target(&remove_args("claude", "work")), account("claude", "work"));
    assert_eq!(target(&use_args("codex", "")), account("codex", ""));
    assert_eq!(target(&use_args("codex", "ci")), account("codex", "ci"));
    assert_eq!(target(&rename_args("claude", "", "Mine")), account("claude", ""));
    assert_eq!(target(&rename_args("claude", "work", "")), account("claude", "work"));
    assert_eq!(target(&args(&["config", "--json"])), None);
}

#[test]
fn a_profile_change_names_the_profile_it_changes() {
    assert_eq!(target(&profile_new_args("Night")), Some(Target::NewProfile));
    assert_eq!(target(&profile_use_args("")), Some(Target::Profile(String::new())));
    assert_eq!(target(&profile_set_args("Work", "claude", "home")), Some(Target::Profile("Work".into())));
    assert_eq!(target(&profile_rename_args("Work", "Office")), Some(Target::Profile("Work".into())));
    assert_eq!(target(&profile_delete_args("Night")), Some(Target::Profile("Night".into())));
}

#[test]
fn names_prefer_labels() {
    let info = parse(SAMPLE).expect("parses");
    let (claude, codex) = (&info.providers[0], &info.providers[1]);
    assert_eq!(claude.name_of("work"), "Work");
    assert_eq!(claude.name_of("home"), "home");
    assert_eq!(claude.name_of(""), "System");
    assert_eq!(codex.name_of(""), "Mine");
}

#[test]
fn profiles_list_every_agent() {
    let info = parse(SAMPLE).expect("parses");
    let pairs: Vec<(&str, &str)> =
        profile_accounts(&info.profiles[0], &info.providers).into_iter().map(|(p, a)| (p.id.as_str(), a)).collect();
    assert_eq!(pairs, [("claude", "work"), ("codex", "")]);
}

#[test]
fn names_are_checked_as_cornercase_does() {
    assert!(valid_name("work"));
    assert!(valid_name("my.work_2-x"));
    assert!(!valid_name(""));
    assert!(!valid_name(".."));
    assert!(!valid_name("a b"));
    assert!(!valid_name(&"x".repeat(33)));
}

#[test]
fn commands_keep_names_apart_from_options() {
    assert_eq!(add_args("claude", "work", None), ["account", "add", "--json", "--", "claude", "work"]);
    assert_eq!(
        add_args("claude", "work", Some(9)),
        ["account", "add", "--json", "--login", "--workspace", "9", "--", "claude", "work"]
    );
    assert_eq!(use_args("codex", ""), ["account", "use", "--system", "--json", "--", "codex"]);
    assert_eq!(rename_args("claude", "", "Mine"), ["account", "rename", "--json", "--system", "--", "claude", "Mine"]);
    assert_eq!(rename_args("claude", "work", ""), ["account", "rename", "--json", "--clear", "--", "claude", "work"]);
    assert_eq!(profile_set_args("Work", "claude", ""), ["profile", "set", "--provider=claude", "--", "Work"]);
    assert_eq!(
        profile_set_args("Work", "claude", "home"),
        ["profile", "set", "--provider=claude", "--account=home", "--", "Work"]
    );
    assert_eq!(profile_use_args(""), ["profile", "use", "--system"]);
}

#[test]
fn a_change_says_what_it_is_doing() {
    assert_eq!(doing(&add_args("claude", "work", Some(3))), "Adding the account and opening its login…");
    assert_eq!(doing(&add_args("claude", "work", None)), "Adding the account…");
    assert_eq!(doing(&remove_args("claude", "work")), "Removing the account and its folder…");
    assert_eq!(doing(&profile_delete_args("night")), "Deleting the profile…");
    assert_eq!(doing(&profile_use_args("night")), "Saving the profile…");
}

#[test]
fn an_agent_lists_its_own_login_first_then_its_accounts() {
    let info = parse(SAMPLE).expect("parses");
    let rows: Vec<(&str, bool)> = Row::of(&info.providers[0]).map(|r| (r.folder(), r.active())).collect();
    assert_eq!(rows, [("", false), ("work", true), ("home", false)]);
}

#[test]
fn a_row_shows_its_folder_under_its_name() {
    let info = parse(SAMPLE).expect("parses");
    let details: Vec<String> = Row::of(&info.providers[0]).map(|r| r.detail("/home/me")).collect();
    assert_eq!(details, ["~/.claude", "folder work · /s/accounts/claude/work", "/s/accounts/claude/home"]);
}

#[test]
fn removing_names_the_account_and_the_login_its_panes_go_back_to() {
    let info = parse(SAMPLE).expect("parses");
    let work = Row::of(&info.providers[0]).nth(1).expect("the work account");
    let ask = work.remove_ask();
    assert_eq!(ask.title, "Remove the account Work?");
    assert!(ask.message.contains("go back to Claude Code's own login"));
    assert_eq!(ask.command, remove_args("claude", "work"));
    assert_eq!(delete_ask("Night").command, profile_delete_args("Night"));
}

#[test]
fn a_signed_out_row_says_how_to_log_in_again() {
    let info = parse(SAMPLE).expect("parses");
    let home = Row::of(&info.providers[0]).nth(2).expect("the home account");
    let hint = home.login_hint(LoginState::SignedOut).expect("a hint");
    assert!(hint.ends_with("cornercase new-tab --account claude=home"));
    assert_eq!(home.login_hint(LoginState::SignedIn), None);
}
