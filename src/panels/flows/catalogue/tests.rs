use super::*;

const LOCAL: &str = crate::test_util::fixture!("flows/personas/catalogue.json");
const WEB: &str = crate::test_util::fixture!("flows/personas/catalogue-web.json");

fn local() -> Catalogue {
    parse_catalogue(LOCAL).expect("the catalogue")
}

fn ids(offered: &[&Offered]) -> Vec<String> {
    offered.iter().map(|offered| offered.persona.id.clone()).collect()
}

fn group<'a>(catalogue: &'a Catalogue, label: &str) -> Group<'a> {
    catalogue.groups("").into_iter().find(|group| group.label == label).expect("the division")
}

#[test]
fn personas_are_grouped_by_division_under_their_labels() {
    let catalogue = local();
    let groups = catalogue.groups("");
    let labels: Vec<&str> = groups.iter().map(|group| group.label.as_str()).collect();
    assert_eq!(labels, ["design", "Engineering", "Testing"]);
    assert_eq!(ids(&groups[1].offered), ["architect", "code-reviewer", "second-reviewer"]);
}

#[test]
fn a_persona_outside_every_division_comes_last() {
    let mut catalogue = local();
    catalogue.personas[0].division = None;
    let labels: Vec<String> = catalogue.groups("").into_iter().map(|group| group.label).collect();
    assert_eq!(labels.last().map(String::as_str), Some(UNSORTED));
}

#[test]
fn a_filter_keeps_the_divisions_that_hold_a_match() {
    let catalogue = local();
    let groups = catalogue.groups("MENTOR");
    assert_eq!(groups.len(), 1);
    assert_eq!(ids(&groups[0].offered), ["code-reviewer"]);
    assert_eq!(catalogue.groups("engineering/arch")[0].label, "Engineering");
}

#[test]
fn a_persona_with_a_problem_cannot_be_picked() {
    let catalogue = local();
    let mut chosen = Chosen::default();
    let reviewer = &catalogue.personas[2];
    assert!(reviewer.problem.as_deref().is_some_and(|problem| problem.contains("also named")));
    chosen.toggle(reviewer);
    assert_eq!(chosen.picked(&catalogue).len(), 0);
}

#[test]
fn nothing_is_picked_at_first() {
    let catalogue = local();
    let chosen = Chosen::default();
    assert!(catalogue.groups("").iter().all(|group| chosen.ticked(group) == Ticked::None));
}

#[test]
fn a_division_ticks_every_persona_it_can_import_and_unticks_them() {
    let catalogue = local();
    let testing = group(&catalogue, "Testing");
    let mut chosen = Chosen::default();
    chosen.toggle_group(&testing);
    assert_eq!(ids(&chosen.picked(&catalogue)), ["careful-tester", "tester"]);
    assert_eq!(chosen.ticked(&testing), Ticked::All);
    chosen.toggle_group(&testing);
    assert_eq!(chosen.picked(&catalogue).len(), 0);
}

#[test]
fn a_division_with_some_picked_is_ticked_in_part_and_its_tick_takes_the_rest() {
    let catalogue = local();
    let testing = group(&catalogue, "Testing");
    let mut chosen = Chosen::default();
    chosen.toggle(&catalogue.personas[5]);
    assert_eq!(chosen.ticked(&testing), Ticked::Some);
    chosen.toggle_group(&testing);
    assert_eq!(chosen.ticked(&testing), Ticked::All);
}

#[test]
fn a_division_ticks_only_what_the_filter_shows() {
    let catalogue = local();
    let mut chosen = Chosen::default();
    let found = catalogue.groups("two").remove(0);
    chosen.toggle_group(&found);
    assert_eq!(ids(&chosen.picked(&catalogue)), ["tester"]);
}

#[test]
fn the_catalogue_is_read_without_saving() {
    assert_eq!(catalogue_args(" owner/repo "), ["persona", "import", "--catalogue", "--json", "--", "owner/repo"]);
}

#[test]
fn an_import_names_each_pick_and_replaces_only_when_one_replaces_yours() {
    let catalogue = local();
    let designer = &catalogue.personas[0];
    assert_eq!(only_args("src", &[designer]), ["persona", "import", "--yes", "--json", "--only=designer", "--", "src"]);
    let tester = &catalogue.personas[4];
    assert!(tester.persona.replaced);
    assert!(only_args("src", &[designer, tester]).contains(&"--replace".to_string()));
}

#[test]
fn the_confirmation_says_the_risk_and_which_ones_replace_yours() {
    let catalogue = local();
    let picked = [&catalogue.personas[0], &catalogue.personas[4]];
    let text = confirmation(&catalogue, &picked);
    assert!(text.starts_with("Personas are prompt text"));
    assert!(text.ends_with("It replaces yours: 🧪 Careful Tester."));
}

#[test]
fn a_web_source_says_where_it_came_from_and_that_nobody_reviewed_it() {
    let catalogue = parse_catalogue(WEB).expect("the catalogue");
    assert!(catalogue.web);
    assert_eq!(catalogue.divisions[0].label.as_deref(), Some("Academic"));
    assert_eq!(catalogue.divisions[0].icon.as_deref(), Some("GraduationCap"));
    let check = sentence(&catalogue.check);
    assert!(check.starts_with("From msitarzewski/agency-agents (see https://github.com/msitarzewski/agency-agents)"));
    assert!(check.contains("nobody here reviewed"));
}

#[test]
fn the_summary_counts_what_can_be_picked_and_what_cannot() {
    assert_eq!(
        local().summary(),
        "3 personas to pick from, 3 that cannot be imported and 2 files skipped as not personas."
    );
}

#[test]
fn a_persona_without_a_name_is_shown_by_its_file() {
    let offered =
        Offered { file: "design/notes.md".into(), problem: Some("not a persona".into()), ..Offered::default() };
    assert_eq!(offered.title(), "design/notes.md");
    assert!(!offered.importable());
}
