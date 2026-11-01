use super::*;

fn layer(number: u64, state: &str, checks: Option<&str>) -> Layer {
    Layer { number, state: state.into(), checks: checks.map(String::from), ..Default::default() }
}

fn stacked(state: &str, layers: Vec<Layer>, position: usize) -> PullRequest {
    let size = layers.len();
    let stack = Stack { number: 480, base: "main".into(), position, size, layers };
    PullRequest { number: 485, state: state.into(), stack: Some(stack), ..Default::default() }
}

#[test]
fn the_heading_names_the_stack_its_base_and_the_layer() {
    let pr = stacked("open", vec![layer(483, "open", None), layer(485, "open", None)], 2);
    assert_eq!(heading(pr.stack.as_ref().expect("a stack")), "#480 on main · layer 2 of 2");
}

#[test]
fn what_blocks_below_is_said_with_its_number() {
    let pr = stacked("open", vec![layer(483, "open", Some("failing")), layer(485, "open", None)], 2);
    assert_eq!(blocker_said(&pr), Some(("Cannot merge until #483 below passes its checks".into(), Block::Checks)));
    let conflicting = Layer { conflicts: true, ..layer(483, "draft", None) };
    let pr = stacked("open", vec![conflicting, layer(485, "open", None)], 2);
    assert_eq!(blocker_said(&pr).map(|b| b.0), Some("Cannot merge until #483 below has no merge conflicts".into()));
}

#[test]
fn a_merged_layer_below_asks_to_pull_only_while_behind() {
    let pr = stacked("open", vec![layer(483, "merged", None), layer(485, "open", None)], 2);
    let stack = pr.stack.as_ref().expect("a stack");
    assert_eq!(
        merged_below(&pr, stack, 2).as_deref(),
        Some("#483 below was merged and this branch is behind its remote: pull it before pushing again.")
    );
    assert_eq!(merged_below(&pr, stack, 0), None);
    let merged = stacked("merged", vec![layer(483, "merged", None), layer(485, "merged", None)], 2);
    assert_eq!(merged_below(&merged, merged.stack.as_ref().expect("a stack"), 2), None);
}
