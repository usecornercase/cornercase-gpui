use super::*;
use crate::cli::Version;
use crate::test_util::fixture;

fn seen(saved: u64, count: usize, unreadable: bool) -> SecretsSeen {
    SecretsSeen { saved, count, unreadable }
}

#[test]
fn the_list_is_read_with_its_names_and_dates() {
    let listed = listed_of(fixture!("bots/secret-list.json")).expect("list");
    let found: Vec<(&str, i64)> = listed.secrets.iter().map(|secret| (secret.name.as_str(), secret.saved)).collect();
    assert_eq!(found, [("campus-password", 1_795_000_100), ("shell-token", 1_795_003_700)]);
    assert_eq!(listed.key, "file");
    assert_eq!(listed.names(), ["campus-password", "shell-token"]);
}

#[test]
fn an_entry_without_a_name_or_that_does_not_read_is_left_out() {
    let text = r#"{"bot":"campus","secrets":[{"name":"a","saved":1},{"saved":2},{"name":"","saved":3},{"name":4},"b",{"name":"c","saved":"x"},{"name":"d"}]}"#;
    let listed = listed_of(text).expect("list");
    assert_eq!(listed.names(), ["a", "d"]);
    assert_eq!(listed.key, "");
}

#[test]
fn an_answer_without_a_list_is_an_error_and_never_an_empty_list() {
    assert!(listed_of(r#"{"bot":"campus","key":"file"}"#).is_err());
    assert!(listed_of(r#"{"bot":"campus","secrets":{"a":1}}"#).is_err());
    assert!(listed_of("not json").is_err());
    assert_eq!(listed_of(r#"{"secrets":[]}"#).expect("empty").secrets, []);
}

#[test]
fn the_arguments_of_set_list_and_remove_are_exact_and_hold_no_value() {
    assert_eq!(secret_list_args("campus"), ["bot", "secret", "list", "--json", "--", "campus"]);
    assert_eq!(secret_set_args("campus", "-x"), ["bot", "secret", "set", "--json", "--", "campus", "-x"]);
    assert_eq!(secret_remove_args("-b", "pw"), ["bot", "secret", "remove", "--json", "--", "-b", "pw"]);
}

#[test]
fn a_name_cornercase_would_refuse_is_said_before_anything_is_sent() {
    let only = Some("A secret's name takes only a-z, 0-9, - and _.".to_string());
    assert_eq!(name_refusal("Campus"), only);
    assert_eq!(name_refusal("bad name"), only);
    assert_eq!(name_refusal("ñ"), only);
    assert_eq!(name_refusal(&"a".repeat(41)), Some("A secret's name takes 1 to 40 characters, not 41.".to_string()));
    assert_eq!(name_refusal(""), Some("A secret's name takes 1 to 40 characters, not 0.".to_string()));
    for good in ["-x", "a_b-1", &"a".repeat(40)] {
        assert_eq!(name_refusal(good), None, "{good}");
    }
}

#[test]
fn return_goes_to_what_is_missing_and_saves_only_with_a_name_and_a_value() {
    assert_eq!(on_secret_enter("", false), SecretEnter::FocusName);
    assert_eq!(on_secret_enter("", true), SecretEnter::FocusName);
    assert!(matches!(on_secret_enter("Bad", false), SecretEnter::Refused(_)));
    assert_eq!(on_secret_enter("pw", true), SecretEnter::FocusValue);
    assert_eq!(on_secret_enter("pw", false), SecretEnter::Save);
}

#[test]
fn a_name_already_saved_says_it_replaces() {
    let saved = ["campus-password".to_string(), "token".to_string()];
    assert!(replaces("token", &saved));
    assert!(!replaces("tok", &saved));
    assert!(!replaces("token", &[]));
}

#[test]
fn the_section_is_shown_named_as_missing_here_or_hidden() {
    assert_eq!(secrets_section(None, Os::Mac), SecretsSection::Shown);
    let older = Some(Version::new(0, 13, 99));
    assert_eq!(secrets_section(Some(Lacks::Server(older)), Os::Mac), SecretsSection::Hidden);
    assert_eq!(secrets_section(Some(Lacks::Machine("devbox", older)), Os::Mac), SecretsSection::Hidden);
    let words = "Needs cornercase 0.13.100 or newer on this Mac (it has 0.13.99).";
    assert_eq!(secrets_section(Some(Lacks::ThisComputer(older)), Os::Mac), SecretsSection::Lacks(words.to_string()));
}

#[test]
fn the_list_is_read_again_only_when_it_can_have_changed() {
    let was = seen(1, 2, false);
    assert!(rereads(Some(2), was, seen(2, 2, false)), "a secret saved here");
    assert!(rereads(Some(2), was, seen(1, 3, false)), "a count the list does not have");
    assert!(!rereads(Some(3), was, seen(1, 3, false)), "a count the list already has");
    assert!(!rereads(Some(2), was, was), "nothing moved");
    assert!(rereads(Some(2), was, seen(1, 2, true)), "the file stopped opening");
    assert!(rereads(None, was, seen(1, 1, false)), "a failed list and a new count");
    assert!(!rereads(None, was, was), "a failed list and nothing moved");
}

#[test]
fn the_words_name_the_bot_and_the_secret() {
    assert_eq!(
        empty_words("campus"),
        "campus has no secrets yet. Add… saves a password or a token only campus can read."
    );
    assert_eq!(REMOVE_SECRET, "Remove secret");
    assert_eq!(remove_secret_message("pw", "campus"), "Remove pw from campus? The bot can no longer read it.");
    assert_eq!(secrets_read_failed("no file"), "Could not read the secrets: no file");
    assert_eq!(secret_save_failed("too long"), "Could not save the secret: too long");
    assert_eq!(secret_remove_failed("pw", "campus", "gone"), "Could not remove pw from campus: gone");
}

#[test]
fn what_a_secret_is_names_the_computer_that_keeps_it() {
    assert!(what_a_secret_is(true).ends_with("on this computer."));
    assert!(what_a_secret_is(false).ends_with("on the computer that runs cornercase."));
}
