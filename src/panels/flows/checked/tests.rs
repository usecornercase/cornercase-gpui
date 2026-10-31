use super::parse_checked;

const CHECK: &str = crate::test_util::fixture!("flows/import_check.json");

#[test]
fn a_check_lists_the_roles_with_their_agents() {
    let checked = parse_checked(CHECK).expect("check");
    let roles: Vec<(&str, String)> = checked.roles.iter().map(|r| (r.role.as_str(), r.facts())).collect();
    assert_eq!(
        roles,
        [("orchestrator", "grok".into()), ("coder", "grok · edits".into()), ("reviewer", "codex".into())]
    );
}

#[test]
fn a_check_keeps_the_risks_and_the_missing_agents() {
    let checked = parse_checked(CHECK).expect("check");
    assert!(checked.check.as_deref().is_some_and(|c| c.contains("passes --danger to `reviewer`")));
    assert!(checked.note.as_deref().is_some_and(|n| n.contains("not installed")));
}

#[test]
fn the_headline_says_where_the_flow_goes() {
    let checked = parse_checked(CHECK).expect("check");
    assert_eq!(checked.headline(), "zz-gpui-throwaway-7733 goes into yours, with 3 agents.");
}

#[test]
fn a_flow_without_risks_has_no_check_or_note() {
    let checked = parse_checked(r#"{"name":"Q","origin":"project","replaced":true,"roles":[]}"#).expect("check");
    assert_eq!((checked.check.clone(), checked.note.clone()), (None, None));
    assert_eq!(checked.headline(), "Q replaces the flow of that name in the project's flows, with 0 agents.");
}
