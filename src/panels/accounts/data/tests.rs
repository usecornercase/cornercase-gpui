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
    assert_eq!(claude.accounts[0].login.as_ref().map(Login::text).as_deref(), Some("signed in as a@b.c (max)"));
    assert_eq!(claude.accounts[1].login.as_ref().map(Login::text).as_deref(), Some("signed out"));
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
