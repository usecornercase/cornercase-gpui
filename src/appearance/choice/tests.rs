use super::*;

#[test]
fn without_a_file_the_window_uses_cornercase_dark() {
    let choice = Choice::parse("");
    assert_eq!(choice.in_use(true), "cornercase-dark");
    assert_eq!(choice.in_use(false), "cornercase-dark");
}

#[test]
fn following_the_system_uses_the_light_slot_in_light() {
    let choice = Choice { follow_system: true, ..Choice::default() };
    assert_eq!(choice.in_use(false), "cornercase-light");
    assert_eq!(choice.in_use(true), "cornercase-dark");
}

#[test]
fn a_light_theme_chosen_while_following_fills_the_light_slot() {
    let mut choice = Choice { follow_system: true, ..Choice::default() };
    choice.choose("nord", false);
    choice.choose("catppuccin-latte", true);
    assert_eq!((choice.dark.as_str(), choice.light.as_str()), ("nord", "catppuccin-latte"));
    assert_eq!(choice.theme, "cornercase-dark");
}

#[test]
fn turning_following_on_keeps_the_theme_in_use_in_its_slot() {
    let mut choice = Choice { theme: "dracula".to_string(), ..Choice::default() };
    choice.follow(true, true, false);
    assert_eq!(choice.in_use(true), "dracula");
    assert_eq!(choice.in_use(false), "cornercase-light");
}

#[test]
fn turning_following_off_keeps_what_was_showing() {
    let mut choice = Choice { follow_system: true, light: "solarized-light".to_string(), ..Choice::default() };
    choice.follow(false, false, true);
    assert_eq!(choice.theme, "solarized-light");
}

#[test]
fn an_unreadable_choice_falls_back_to_the_defaults() {
    assert_eq!(Choice::parse("{ nope"), Choice::default());
}
