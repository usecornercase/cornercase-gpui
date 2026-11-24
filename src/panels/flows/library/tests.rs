use super::*;
use crate::panels::flows::model::parse_listing;

const LIST: &str = crate::test_util::fixture!("flows/personas/list.json");
const SHOWN: &str = crate::test_util::fixture!("flows/personas/shown.json");
const FLOWS: &str = crate::test_util::fixture!("flows/personas/flows.json");

fn library() -> Library {
    parse_library(LIST).expect("the list")
}

#[test]
fn the_library_lists_personas_and_the_files_it_cannot_read() {
    let library = library();
    let ids: Vec<&str> = library.personas.iter().map(|persona| persona.id.as_str()).collect();
    assert_eq!(ids, ["careful-tester", "engineering-code-reviewer"]);
    assert_eq!(library.broken.len(), 1);
    assert!(library.broken[0].error.contains("no front matter"));
}

#[test]
fn a_persona_is_labelled_by_its_emoji_and_name() {
    assert_eq!(library().get("engineering-code-reviewer").map(Persona::label), Some("👁️ Code Reviewer".into()));
    let bare = Persona { name: "Plain".into(), ..Persona::default() };
    assert_eq!(bare.label(), "Plain");
}

#[test]
fn a_persona_comes_from_its_repository_or_is_yours() {
    let library = library();
    assert_eq!(library.get("careful-tester").map(Persona::origin), Some("yours".into()));
    assert_eq!(
        library.get("engineering-code-reviewer").map(Persona::origin),
        Some("msitarzewski/agency-agents".into())
    );
}

#[test]
fn a_source_on_the_disk_or_the_web_shows_its_file_name() {
    assert_eq!(short_source("/Users/me/personas/tester.md"), "tester.md");
    assert_eq!(short_source("https://example.com/agents/tester.md"), "tester.md");
    assert_eq!(short_source("./reviewer.md"), "reviewer.md");
    assert_eq!(short_source("owner/repo"), "owner/repo");
}

#[test]
fn the_instructions_are_the_text_after_the_front_matter() {
    let shown = parse_shown(SHOWN).expect("the persona");
    assert_eq!(body(&shown.text), "Test first.\n");
    assert_eq!(body("no front matter\n"), "no front matter\n");
    assert_eq!(body("---\nname: x\nno end\n"), "---\nname: x\nno end\n");
}

#[test]
fn an_edit_starts_from_what_the_persona_says() {
    let draft = Draft::of(&parse_shown(SHOWN).expect("the persona"));
    assert_eq!(draft.name, "Careful Tester");
    assert_eq!(draft.emoji, "🧪");
    assert_eq!(draft.description, "");
    assert_eq!(draft.body, "Test first.\n");
}

#[test]
fn a_new_persona_sends_only_what_was_filled_in() {
    let draft =
        Draft { name: " Careful Tester ".into(), emoji: "🧪".into(), body: "Test.".into(), ..Draft::default() };
    assert_eq!(draft.args(None), ["persona", "new", "-", "--name=Careful Tester", "--emoji=🧪", "--json"]);
}

#[test]
fn an_edit_sends_every_field_so_an_emptied_one_is_cleared() {
    let draft = Draft { name: "Tester".into(), body: "Test.".into(), ..Draft::default() };
    assert_eq!(
        draft.args(Some("careful-tester")),
        [
            "persona",
            "edit",
            "careful-tester",
            "--instructions=-",
            "--name=Tester",
            "--description=",
            "--emoji=",
            "--json"
        ]
    );
}

#[test]
fn a_value_starting_with_a_dash_stays_a_value() {
    let draft = Draft { name: "-x".into(), body: "Test.".into(), ..Draft::default() };
    assert!(draft.args(None).contains(&"--name=-x".to_string()));
}

#[test]
fn a_draft_needs_a_name_and_instructions() {
    let named = Draft { name: "Tester".into(), body: "Test.".into(), ..Draft::default() };
    assert_eq!(named.problem(), None);
    assert!(Draft { name: " ".into(), ..named.clone() }.problem().is_some());
    assert!(Draft { body: "\n".into(), ..named }.problem().is_some());
}

#[test]
fn the_flows_that_follow_a_persona_are_named_once() {
    let listing = parse_listing(FLOWS).expect("the flows");
    assert_eq!(followers(&listing.flows, "engineering-code-reviewer"), ["Reviewed"]);
    assert_eq!(followers(&listing.flows, "careful-tester"), ["Reviewed"]);
    assert_eq!(followers(&listing.flows, "other").len(), 0);
}

#[test]
fn names_are_listed_in_words() {
    let names = |list: &[&str]| and_list(&list.iter().map(ToString::to_string).collect::<Vec<_>>());
    assert_eq!(names(&["Quick"]), "Quick");
    assert_eq!(names(&["Quick", "Arena"]), "Quick and Arena");
    assert_eq!(names(&["Quick", "Arena", "Full"]), "Quick, Arena and Full");
}

#[test]
fn a_size_is_rounded_up_to_kibibytes() {
    assert_eq!(kib(3154), "4 KiB");
    assert_eq!(kib(1024), "1 KiB");
}

#[test]
fn a_run_is_labelled_from_the_library() {
    let library = library();
    let label = labeller(Some(&library));
    assert_eq!(label("careful-tester"), Some("🧪 Careful Tester".into()));
    assert_eq!(label("gone"), None);
    assert_eq!(labeller(None)("careful-tester"), None);
}

#[test]
fn an_import_answers_what_it_saved_and_where() {
    let imported = parse_preview(crate::test_util::fixture!("flows/personas/imported.json")).expect("the import");
    assert!(imported.saved);
    assert_eq!(imported.personas[0].path.as_deref(), Some("/tmp/cc884/personas/engineering-code-reviewer.md"));
}
