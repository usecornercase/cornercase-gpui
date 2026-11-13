use super::*;

#[test]
fn the_team_takes_two_columns_where_two_cards_fit() {
    assert_eq!(team_columns(640.), 2);
    assert_eq!(team_columns(480.), 2);
}

#[test]
fn a_narrow_page_stacks_the_team_in_one_column() {
    assert_eq!(team_columns(479.), 1);
    assert_eq!(team_columns(330.), 1);
}

#[test]
fn a_runs_pill_reads_its_standing() {
    let words = [Standing::Closed, Standing::Ended, Standing::Stopped(90), Standing::Running].map(hero_word);
    assert_eq!(words, ["agents closed", "reached its end", "stopped moving", "running"]);
}

#[test]
fn a_stopped_run_with_stages_owed_shows_the_stop_then_the_stages() {
    let run = Run { owed: vec![1, 2], ..Run::default() };
    let notes = run_notes(&run, Standing::Stopped(180));
    assert_eq!(notes.iter().map(|(_, kind)| *kind).collect::<Vec<_>>(), [Note::Stopped, Note::Owed]);
    assert!(notes[0].0.starts_with("stopped moving 3m ago: "));
    assert_eq!(notes[1].0, "stage 2 and 3 not run since the last change");
    assert_eq!(run_notes(&Run::default(), Standing::Running).len(), 0);
}
