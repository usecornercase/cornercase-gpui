use super::*;
use crate::model::bots::Home;

fn bot(id: &str, home: &Home) -> Bot {
    Bot { id: id.into(), name: id.into(), home: home.clone(), ..Bot::default() }
}

fn seen(name: &str) -> MachineSeen {
    MachineSeen { name: name.into(), ..MachineSeen::default() }
}

fn names(groups: &[Group]) -> Vec<(Option<String>, Vec<String>)> {
    groups
        .iter()
        .map(|group| {
            let machine = group.machine.as_ref().map(|heading| heading.name.clone());
            (machine, group.bots.iter().map(|bot| bot.id.clone()).collect())
        })
        .collect()
}

#[test]
fn this_computers_bots_come_first_with_no_heading() {
    let (vps, here) = (Home::Machine("vps".into()), [bot("prs", &Home::Here)]);
    let there = [bot("ops", &vps)];
    let groups = bot_groups(&here, vec![(&seen("vps"), &there[..], None)]);
    assert_eq!(names(&groups), [(None, vec!["prs".to_string()]), (Some("vps".into()), vec!["ops".to_string()])]);
}

#[test]
fn a_machine_with_no_bot_has_no_heading() {
    let vps = seen("vps");
    assert_eq!(bot_groups(&[], vec![(&vps, &[], None)]), Vec::new());
}

#[test]
fn the_machines_keep_the_order_they_are_given() {
    let (a, b) = (Home::Machine("a".into()), Home::Machine("b".into()));
    let (on_b, on_a) = ([bot("x", &b)], [bot("y", &a)]);
    let groups = bot_groups(&[], vec![(&seen("b"), &on_b[..], None), (&seen("a"), &on_a[..], None)]);
    let order: Vec<_> = groups.iter().filter_map(|group| group.machine.as_ref().map(|h| h.name.clone())).collect();
    assert_eq!(order, ["b", "a"]);
}

#[test]
fn a_down_machine_carries_its_note() {
    let there = [bot("ops", &Home::Machine("vps".into()))];
    let groups = bot_groups(&[], vec![(&seen("vps"), &there[..], Some("vps unreachable".into()))]);
    assert_eq!(groups[0].note.as_deref(), Some("vps unreachable"));
}

#[test]
fn the_wait_for_linked_bots_shows_only_in_a_window_on_another_server_with_machines() {
    assert_eq!(linked_bots_wait(None, 2), None);
    assert_eq!(linked_bots_wait(Some("devbox"), 0), None);
    let said = linked_bots_wait(Some("devbox"), 1).expect("a line");
    assert!(said.contains("linked from devbox") && said.ends_with(MACHINE_BOTS_ISSUE), "{said}");
}
