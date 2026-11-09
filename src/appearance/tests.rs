use super::*;

fn with(user: &[(&str, &str)]) -> Appearance {
    let mut this = Appearance::read(PathBuf::from("/nonexistent/cornercase-gpui"), true);
    this.user = user.iter().map(|(a, b)| ((*a).to_string(), (*b).to_string())).collect();
    this.rebuild();
    this
}

#[test]
fn every_preset_is_listed_first_then_the_user_s_themes_by_name() {
    let this = with(&[("zed", r#"{"name": "Zed"}"#), ("abe", r#"{"name": "Abe"}"#)]);
    let names: Vec<String> = this.themes().into_iter().map(|t| t.name).collect();
    assert_eq!(names.len(), PRESETS.len() + 2);
    assert_eq!(&names[PRESETS.len()..], ["Abe", "Zed"]);
    assert_eq!(names[0], "cornercase dark");
}

#[test]
fn a_user_file_named_like_a_preset_is_listed_with_a_problem() {
    let this = with(&[("nord", "{}")]);
    let nord = this.themes().into_iter().find(|t| t.id == "nord").expect("listed");
    assert!(nord.problem.is_some_and(|why| why.contains("named like a preset")));
}

#[test]
fn a_broken_theme_is_listed_with_its_problem_and_its_file_name() {
    let this = with(&[("broken", "{ nope")]);
    let listed = this.themes().into_iter().find(|t| t.id == "broken").expect("listed");
    assert!(listed.problem.is_some());
    assert_eq!(listed.name, "broken");
}

#[test]
fn a_theme_in_use_that_is_gone_is_a_problem_not_a_crash() {
    let mut this = with(&[]);
    this.choice.theme = "vanished".to_string();
    this.rebuild();
    assert!(this.theme_of("vanished").is_none());
    assert!(this.base().is_none());
}

#[test]
fn a_new_problem_is_noticed_once() {
    let mut this = with(&[]);
    this.problem_changed(Some("themes/x.json: oops".to_string()));
    this.problem_changed(Some("themes/x.json: oops".to_string()));
    assert_eq!(this.take_notices().len(), 1);
    this.problem_changed(None);
    assert!(this.take_notices().is_empty());
}

#[test]
fn our_own_save_is_not_taken_as_an_edit() {
    let mut this = with(&[]);
    this.save_theme("mine", r#"{"name": "Mine"}"#.to_string());
    let path = files::theme_path(&this.folder, "mine");
    assert!(!this.reload(&path));
}

#[test]
fn a_file_changed_by_hand_is_taken_as_an_edit() {
    let folder = std::env::temp_dir().join(format!("cornercase-gpui-appearance-{}", std::process::id()));
    std::fs::create_dir_all(files::themes_dir(&folder)).expect("folder");
    let mut this = with(&[("mine", r#"{"name": "Mine"}"#)]);
    this.folder = folder.clone();
    let path = files::theme_path(&folder, "mine");
    std::fs::write(&path, r#"{"name": "Mine, edited"}"#).expect("write");
    assert!(this.reload(&path));
    assert!(!this.reload(&path));
    std::fs::remove_dir_all(&folder).ok();
}
