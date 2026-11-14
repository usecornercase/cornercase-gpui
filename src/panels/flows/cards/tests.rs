use super::*;

fn member(exited: bool, closed: bool, status: Option<&str>) -> Member {
    Member {
        role: "coder".into(),
        pane: Some(8),
        exited,
        closed,
        status: status.map(str::to_string),
        ..Member::default()
    }
}

#[test]
fn an_exited_role_reads_exited_in_the_waiting_colour() {
    let word = status_word(Some(&member(true, false, None)));
    assert_eq!(word, Some(("exited".to_string(), colours::waiting())));
}

#[test]
fn a_closed_role_reads_closed_even_when_its_agent_exited() {
    let word = status_word(Some(&member(true, true, None))).expect("a word");
    assert_eq!(word.0, "closed");
}

#[test]
fn a_role_with_no_status_and_no_exited_has_no_word() {
    assert_eq!(status_word(Some(&member(false, false, None))), None);
}

#[test]
fn an_exited_roles_box_is_drawn_exited_even_when_it_holds_the_work() {
    let flow = model::FlowSpec::default();
    let mut run =
        model::Run { holder: Some("coder".into()), roles: vec![member(true, false, None)], ..Default::default() };
    let look = |run: &model::Run| {
        let plan = Plan { flow: &flow, run: Some(run), focus: None, key: "k", on_role: None };
        Look::of(plan, "coder")
    };
    assert!(look(&run) == Look::Exited);
    run.roles[0].closed = true;
    assert!(look(&run) == Look::Closed);
}
