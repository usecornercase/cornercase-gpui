use super::*;
use crate::panels::flows::model::{BackSpec, StageSpec};

type Stage<'a> = (&'a [&'a str], Option<(usize, &'a str)>);

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

fn flow(stages: &[Stage]) -> FlowSpec {
    let stages = stages
        .iter()
        .map(|(roles, back)| StageSpec {
            roles: roles.iter().map(|r| (*r).to_string()).collect(),
            back: back.map(|(to, when)| BackSpec { to, when: when.to_string(), at_most: 2 }),
        })
        .collect();
    FlowSpec { stages, ..FlowSpec::default() }
}

fn arena() -> FlowSpec {
    flow(&[
        (&["coder A", "coder B", "coder C"], None),
        (&["judge"], None),
        (&["integrator"], None),
        (&["reviewer"], Some((3, "findings"))),
    ])
}

fn full_issue() -> FlowSpec {
    flow(&[
        (&["coder"], None),
        (&["reviewer", "security"], Some((1, "findings"))),
        (&["tester"], Some((1, "failures"))),
    ])
}

fn card<'a>(graph: &'a Graph, role: &str) -> &'a Card {
    graph.cards.iter().find(|c| c.role == role).expect("card")
}

fn wire(graph: &Graph, edge: Edge) -> &Wire {
    graph.wires.iter().find(|w| w.edge == Some(edge)).expect("wire")
}

fn step(edge: Edge, from: Option<&str>, to: &str) -> Step {
    Step { edge: Some(edge), from: from.map(str::to_string), to: to.to_string(), time: String::new() }
}

#[test]
fn parallel_roles_sit_side_by_side_centred_under_the_orchestrator() {
    let graph = layout(&arena(), None);
    let (a, b, c) = (card(&graph, "coder A"), card(&graph, "coder B"), card(&graph, "coder C"));
    assert!(close(a.rect.y, b.rect.y) && close(b.rect.y, c.rect.y));
    assert!(a.rect.right() < b.rect.x && b.rect.right() < c.rect.x);
    assert!((b.rect.centre_x() - graph.lead.centre_x()).abs() < 0.01);
    assert!((card(&graph, "judge").rect.centre_x() - graph.lead.centre_x()).abs() < 0.01);
}

#[test]
fn stages_go_down_below_the_orchestrator_and_its_divider() {
    let graph = layout(&arena(), None);
    let tops: Vec<f32> =
        ["coder A", "judge", "integrator", "reviewer"].iter().map(|r| card(&graph, r).rect.y).collect();
    assert!(tops.windows(2).all(|pair| pair[0] + CARD_H < pair[1]));
    assert!(graph.lead.bottom() < graph.divider && graph.divider < tops[0]);
    assert!(graph.end.y > tops[3] + CARD_H);
    assert_eq!(graph.markers.len(), 4);
    assert!(graph.markers.iter().zip(&tops).all(|(m, top)| m.x == RAIL && (m.y - (top + CARD_H / 2.)).abs() < 0.01));
}

#[test]
fn every_role_of_a_stage_is_wired_to_every_role_of_the_next() {
    let graph = layout(&full_issue(), None);
    let into_review: Vec<&Wire> = graph.wires.iter().filter(|w| w.edge == Some(Edge::Forward { stage: 0 })).collect();
    assert_eq!(into_review.len(), 2);
    assert!(into_review.iter().all(|w| w.from.as_deref() == Some("coder") && w.way == Way::Down));
    let security = card(&graph, "security");
    let branch = into_review.iter().find(|w| w.to.as_deref() == Some("security")).expect("branch");
    assert!((branch.tip.x - security.rect.centre_x()).abs() < 0.01 && branch.tip.y < security.rect.y);
    assert_eq!(graph.wires.iter().filter(|w| w.edge == Some(Edge::Forward { stage: 1 })).count(), 2);
}

#[test]
fn the_last_stage_is_wired_to_the_end() {
    let graph = layout(&full_issue(), None);
    let ends: Vec<&Wire> = graph.wires.iter().filter(|w| w.edge.is_none()).collect();
    assert_eq!(ends.len(), 1);
    assert_eq!(ends[0].from.as_deref(), Some("tester"));
    assert!((ends[0].tip.x - graph.end.centre_x()).abs() < 0.01 && ends[0].tip.y < graph.end.y);
}

#[test]
fn a_loop_runs_right_of_the_cards_it_spans_and_points_back_into_its_target() {
    let graph = layout(&arena(), None);
    let back = wire(&graph, Edge::Loop { stage: 3 });
    let (integrator, reviewer) = (card(&graph, "integrator"), card(&graph, "reviewer"));
    let lane = back.curves.iter().map(|c| c.to.x).fold(0., f32::max);
    assert!(lane > integrator.rect.right() && lane > reviewer.rect.right());
    assert_eq!(back.way, Way::Left);
    assert!((back.tip.y - integrator.rect.mid_y()).abs() < 0.01 && back.tip.x > integrator.rect.right());
    assert!((back.curves[0].from.y - reviewer.rect.mid_y()).abs() < 0.01);
    assert_eq!((back.from.as_deref(), back.to.as_deref()), (Some("reviewer"), Some("integrator")));
}

#[test]
fn overlapping_loops_take_lanes_further_out() {
    let graph = layout(&full_issue(), None);
    let lane = |stage| wire(&graph, Edge::Loop { stage }).curves[2].from.x;
    assert!(lane(2) > lane(1));
    assert!(lane(1) > card(&graph, "security").rect.right());
}

fn ship_it() -> FlowSpec {
    flow(&[
        (&["planner"], None),
        (&["frontend", "backend", "docs"], None),
        (&["reviewer"], Some((2, "findings"))),
        (&["qa"], Some((2, "bugs"))),
    ])
}

#[test]
fn a_loop_label_sits_on_its_loop_when_the_loop_leaves_room_for_it() {
    let graph = layout(&ship_it(), None);
    let back = BackSpec { to: 2, when: "findings".into(), at_most: 2 };
    let label = graph.labels.iter().find(|l| l.stage == 2).expect("label");
    let lane = wire(&graph, Edge::Loop { stage: 2 }).curves[2].from.x;
    assert!(label.at.x > card(&graph, "reviewer").rect.right());
    assert!(label.at.x + label_width(&back, false) < lane);
    assert!(close(label.at.y, card(&graph, "reviewer").rect.mid_y()));
}

#[test]
fn loop_labels_line_up_right_of_every_lane_beside_their_stage() {
    let graph = layout(&full_issue(), None);
    assert_eq!(graph.labels.len(), 2);
    let outer = wire(&graph, Edge::Loop { stage: 2 }).curves[2].from.x;
    assert!(graph.labels.iter().all(|l| l.at.x > outer && close(l.at.x, graph.labels[0].at.x)));
    assert!(graph.labels.iter().all(|l| (l.at.y - graph.markers[l.stage].y).abs() < 0.01));
    let failures = BackSpec { to: 1, when: "failures".into(), at_most: 2 };
    assert!(graph.labels.iter().all(|l| graph.width >= l.at.x + label_width(&failures, false)));
}

#[test]
fn a_loop_to_a_later_stage_is_not_drawn() {
    let graph = layout(&flow(&[(&["coder"], Some((2, "never"))), (&["reviewer"], None)]), None);
    assert!(graph.wires.iter().all(|w| !matches!(w.edge, Some(Edge::Loop { .. }))));
    assert_eq!(graph.labels.len(), 0);
}

#[test]
fn a_step_outside_the_plan_runs_left_of_the_cards_it_passes() {
    let graph = layout(&arena(), Some(Edge::Outside { from: Some(0), to: 2 }));
    let outside = wire(&graph, Edge::Outside { from: Some(0), to: 2 });
    let lane = outside.curves[2].from.x;
    assert!(lane < card(&graph, "coder A").rect.x && lane > RAIL);
    assert_eq!((outside.way, outside.to.as_deref()), (Way::Right, Some("integrator")));
    assert!(outside.tip.x < card(&graph, "integrator").rect.x);
}

#[test]
fn a_step_outside_the_plan_from_the_start_leaves_the_orchestrator() {
    let graph = layout(&arena(), Some(Edge::Outside { from: None, to: 1 }));
    let outside = wire(&graph, Edge::Outside { from: None, to: 1 });
    assert!((outside.curves[0].from.y - graph.lead.mid_y()).abs() < 0.01);
    assert_eq!(outside.from, None);
}

#[test]
fn a_forward_step_lights_only_the_branch_it_took() {
    let graph = layout(&full_issue(), None);
    let taken = step(Edge::Forward { stage: 0 }, Some("coder"), "security");
    let sources = graph.sources(0);
    let lit: Vec<&str> =
        graph.wires.iter().filter(|w| w.carries(&taken, &sources)).filter_map(|w| w.to.as_deref()).collect();
    assert_eq!(lit, vec!["security"]);
}

#[test]
fn a_forward_step_from_a_role_outside_the_stage_lights_every_branch_into_its_target() {
    let graph = layout(&arena(), None);
    let taken = step(Edge::Forward { stage: 0 }, Some("orchestrator"), "judge");
    let sources = graph.sources(0);
    assert_eq!(graph.wires.iter().filter(|w| w.carries(&taken, &sources)).count(), 3);
}

#[test]
fn a_loop_step_lights_its_loop_and_nothing_else() {
    let graph = layout(&arena(), None);
    let taken = step(Edge::Loop { stage: 3 }, Some("reviewer"), "integrator");
    let lit: Vec<Option<Edge>> = graph.wires.iter().filter(|w| w.carries(&taken, &[])).map(|w| w.edge).collect();
    assert_eq!(lit, vec![Some(Edge::Loop { stage: 3 })]);
    let nowhere = Step { edge: None, ..taken };
    assert!(graph.wires.iter().all(|w| !w.carries(&nowhere, &[])));
}

#[test]
fn a_point_along_a_wire_runs_from_its_start_to_its_end() {
    let graph = layout(&arena(), None);
    let back = wire(&graph, Edge::Loop { stage: 3 });
    let (first, last) = (back.curves[0].from, back.curves[back.curves.len() - 1].to);
    assert_eq!(back.at(0.), first);
    assert!(distance(back.at(1.), last) < 0.01);
    let middle = back.at(0.5);
    assert!(middle.x > card(&graph, "reviewer").rect.right() && middle.y < first.y && middle.y > last.y);
}

#[test]
fn a_flow_without_stages_keeps_its_orchestrator_and_end() {
    let graph = layout(&FlowSpec::default(), None);
    assert!(graph.cards.is_empty() && graph.wires.is_empty() && graph.markers.is_empty());
    assert!(graph.end.y > graph.divider && graph.height > graph.end.bottom());
    assert!(graph.width >= LEAD_W);
}

#[test]
fn a_board_with_room_keeps_the_natural_size() {
    assert_eq!(fit(&arena(), None, 2000.), layout(&arena(), None));
}

#[test]
fn a_narrow_board_shrinks_the_cards_until_the_plan_fits() {
    let natural = layout(&arena(), None);
    let narrow = fit(&arena(), None, natural.width - 60.);
    assert!(narrow.width <= natural.width - 60.);
    assert!(card(&narrow, "coder A").rect.w < CARD_W);
    assert!(narrow.cards.iter().all(|c| close(c.rect.w, card(&narrow, "judge").rect.w)));
}

#[test]
fn loops_into_one_stage_point_at_it_side_by_side_the_outer_one_higher() {
    let graph = layout(&full_issue(), None);
    let (inner, outer) = (wire(&graph, Edge::Loop { stage: 1 }), wire(&graph, Edge::Loop { stage: 2 }));
    let coder = card(&graph, "coder").rect;
    assert!(outer.tip.y < inner.tip.y);
    assert!(close(f32::midpoint(outer.tip.y, inner.tip.y), coder.mid_y()));
    assert!(inner.tip.y < coder.y + CARD_H && outer.tip.y > coder.y);
}

#[test]
fn a_lone_loop_points_at_the_middle_of_its_target() {
    let graph = layout(&arena(), None);
    assert!((wire(&graph, Edge::Loop { stage: 3 }).tip.y - card(&graph, "integrator").rect.mid_y()).abs() < 0.01);
}

#[test]
fn a_tight_board_shortens_the_loop_labels_before_it_shrinks_the_cards() {
    let natural = layout(&full_issue(), None);
    let tight = fit(&full_issue(), None, natural.width - 20.);
    assert!(tight.short && !natural.short);
    assert!(close(card(&tight, "coder").rect.w, CARD_W));
    assert!(tight.width <= natural.width - 20.);
}

#[test]
fn a_stage_a_loop_leaves_and_another_reaches_keeps_the_two_apart() {
    let graph = layout(
        &flow(&[(&["coder"], None), (&["reviewer"], Some((1, "findings"))), (&["tester"], Some((2, "bugs")))]),
        None,
    );
    let (leaving, arriving) = (wire(&graph, Edge::Loop { stage: 1 }), wire(&graph, Edge::Loop { stage: 2 }));
    let reviewer = card(&graph, "reviewer").rect;
    assert!(leaving.curves[0].from.y < arriving.tip.y);
    assert!(leaving.curves[0].from.y > reviewer.y && arriving.tip.y < reviewer.y + CARD_H);
    let label = graph.labels.iter().find(|l| l.stage == 1).expect("label");
    assert!(close(label.at.y, leaving.curves[0].from.y));
    assert!(label.at.x > wire(&graph, Edge::Loop { stage: 2 }).curves[2].from.x);
}
