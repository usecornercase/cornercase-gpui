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
    let notes = run_notes(&run, Standing::Stopped(180), 0);
    assert_eq!(notes.iter().map(|(_, kind)| *kind).collect::<Vec<_>>(), [Note::Stopped, Note::Owed]);
    assert!(notes[0].0.starts_with("stopped moving 3m ago: "));
    assert_eq!(notes[1].0, "stage 2 and 3 not run since the last change");
    assert_eq!(run_notes(&Run::default(), Standing::Running, 0).len(), 0);
}

#[test]
fn only_a_run_that_moves_has_a_glowing_pill() {
    let glowing = [Standing::Closed, Standing::Ended, Standing::Stopped(90), Standing::Running].map(glows);
    assert_eq!(glowing, [false, false, false, true]);
}

fn exited_run(names: &[&str]) -> Run {
    let roles = names.iter().zip(7..).map(|(n, p)| Member {
        role: (*n).into(),
        pane: Some(p),
        exited: true,
        ..Member::default()
    });
    let mut all = vec![Member { role: "orchestrator".into(), pane: Some(6), ..Member::default() }];
    all.extend(roles);
    Run { roles: all, ..Run::default() }
}

#[test]
fn a_run_with_an_exited_role_says_so_after_the_stop_and_the_stages() {
    let run = Run { owed: vec![1], ..exited_run(&["coder"]) };
    let notes = run_notes(&run, Standing::Stopped(180), 0);
    assert_eq!(notes.iter().map(|(_, kind)| *kind).collect::<Vec<_>>(), [Note::Stopped, Note::Owed, Note::Exited]);
    assert_eq!(notes[2].0, "the agent of coder exited");
}

#[test]
fn the_notes_are_muted_for_a_stop_and_in_the_waiting_colour_otherwise() {
    assert_eq!(note_colour(Note::Stopped), colours::muted());
    assert_eq!(note_colour(Note::Owed), colours::waiting());
    assert_eq!(note_colour(Note::Exited), colours::waiting());
}

#[test]
fn a_live_run_with_an_exited_role_offers_reopen_beside_stop() {
    let run = exited_run(&["coder"]);
    assert_eq!(
        actions(&run, Some(Reopen::Exited(1))),
        [Action::Show(6), Action::Reopen("Reopen 1 exited agent".into()), Action::Stop(6)]
    );
}

#[test]
fn a_closed_run_offers_its_closed_agents_back_and_forget() {
    let run = Run { state: Some("closed".into()), ..Run::default() };
    assert_eq!(
        actions(&run, Some(Reopen::Closed(3))),
        [Action::Reopen("Reopen 3 closed agents".into()), Action::Forget]
    );
}

#[test]
fn a_run_with_exited_roles_where_closed_agents_come_first_says_so() {
    let run = exited_run(&["coder"]);
    assert!(actions(&run, Some(Reopen::Closed(2))).contains(&Action::Reopen("Reopen 2 closed agents".into())));
    let notes = run_notes(&run, Standing::Running, 2);
    assert!(notes[0].0.ends_with("Reopen brings back this workspace's 2 closed agents first"));
}

#[test]
fn a_run_whose_server_sent_no_exited_offers_what_it_offered_before() {
    assert_eq!(actions(&exited_run(&[]), None), [Action::Show(6), Action::Stop(6)]);
}

#[test]
fn a_run_whose_every_agent_exited_offers_reopen_and_no_stop() {
    let mut run = exited_run(&["coder"]);
    run.roles[0].exited = true;
    assert_eq!(
        actions(&run, Some(Reopen::Exited(2))),
        [Action::Show(6), Action::Reopen("Reopen 2 exited agents".into())]
    );
}
