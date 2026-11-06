use super::*;

fn agent(model: Option<&str>, context: Option<u32>) -> PaneInfo {
    PaneInfo { agent: Some("claude".into()), model: model.map(str::to_string), context, ..Default::default() }
}

#[test]
fn a_tab_shows_its_model_and_context() {
    assert_eq!(tab_detail(&agent(Some("Opus 5.5"), Some(12)), None).as_deref(), Some("Opus 5.5 · 12%"));
}

#[test]
fn a_tab_names_its_account_after_the_model() {
    let detail = tab_detail(&agent(Some("Opus 5.5"), Some(12)), Some("work"));
    assert_eq!(detail.as_deref(), Some("Opus 5.5 · work · 12%"));
}

#[test]
fn a_tab_without_a_model_still_names_its_account() {
    assert_eq!(tab_detail(&agent(None, None), Some("custom")).as_deref(), Some("custom"));
}

#[test]
fn a_tab_without_a_model_or_an_account_shows_nothing() {
    assert_eq!(tab_detail(&agent(None, Some(12)), None), None);
}
