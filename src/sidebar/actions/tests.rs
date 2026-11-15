use super::*;
use crate::model::ThereId;
use crate::sidebar::Target;

fn strs(words: &[&str]) -> Vec<String> {
    words.iter().map(ToString::to_string).collect()
}

fn bot(id: &str) -> Bot {
    Bot { id: id.into(), name: id.into(), ..Bot::default() }
}

#[test]
fn a_bots_rename_sends_its_new_name_trimmed() {
    let sent = renamed(Renaming::Bot("prs".into()), "PRs", " PR reviews ");
    assert_eq!(sent, Some(Renamed::Bot("prs".into(), "PR reviews".into())));
}

#[test]
fn an_empty_blank_or_unchanged_bot_name_sends_nothing() {
    for text in ["", "   ", "PRs"] {
        assert_eq!(renamed(Renaming::Bot("prs".into()), "PRs", text), None, "{text:?}");
    }
}

#[test]
fn the_other_rows_rename_as_before() {
    let here = |target, text: &str| renamed(Renaming::Here(target), "old", text);
    assert_eq!(here(Target::Group(3), ""), None);
    assert_eq!(here(Target::Group(3), "Ops"), Some(Renamed::Args(strs(&["rename", "--group", "3", "Ops"]))));
    assert_eq!(here(Target::Project(1), ""), Some(Renamed::Args(strs(&["rename", "--project", "1", ""]))));
    assert_eq!(here(Target::Workspace(2), "x"), Some(Renamed::Args(strs(&["rename", "--workspace", "2", "x"]))));
    assert_eq!(here(Target::Tab(4), "x"), Some(Renamed::Args(strs(&["rename", "--tab", "4", "x"]))));
    let aim = Aim { machine: "m".into(), boot: "b".into(), workspace: ThereId::default() };
    let there = renamed(Renaming::There(aim.clone()), "old", " x ");
    assert_eq!(there, Some(Renamed::There(aim, "x".into())));
}

#[test]
fn a_bots_rename_field_reads_bot_name_and_the_others_keep_theirs() {
    assert_eq!(rename_placeholder(&Renaming::Bot("prs".into())), "bot name");
    assert_eq!(rename_placeholder(&Renaming::Here(Target::Group(1))), "group name");
}

#[test]
fn a_bots_rename_is_gone_once_the_list_drops_the_bot() {
    let what = Renaming::Bot("prs".into());
    assert!(rename_gone(&what, &[bot("ops")]));
    assert!(!rename_gone(&what, &[bot("ops"), bot("prs")]));
    assert!(!rename_gone(&Renaming::Here(Target::Tab(1)), &[]));
}
