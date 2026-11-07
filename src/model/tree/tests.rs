use super::*;

fn item(key: u64, parent: Option<u64>) -> Item<u64> {
    Item { key, parent, folded: false }
}

fn walked(items: &[Item<u64>]) -> Vec<(u64, usize)> {
    order(items).iter().map(|node| (items[node.at].key, node.depth)).collect()
}

#[test]
fn a_child_sits_under_its_parent_wherever_it_is_listed() {
    let items = [item(1, Some(3)), item(2, None), item(3, None)];
    assert_eq!(walked(&items), [(2, 0), (3, 0), (1, 1)]);
}

#[test]
fn a_parent_that_is_not_listed_leaves_its_child_a_root_in_its_place() {
    let items = [item(1, None), item(2, Some(99)), item(3, None)];
    assert_eq!(walked(&items), [(1, 0), (2, 0), (3, 0)]);
}

#[test]
fn a_pane_named_as_its_own_parent_stays_in_its_place() {
    let items = [item(1, Some(1)), item(2, None)];
    assert_eq!(walked(&items), [(1, 0), (2, 0)]);
}

#[test]
fn a_cycle_is_listed_rather_than_lost_and_has_no_way_back() {
    let items = [item(1, Some(2)), item(2, Some(1)), item(3, None)];
    let nodes = order(&items);
    assert_eq!(walked(&items), [(3, 0), (1, 0), (2, 1)]);
    assert_eq!(nodes.last().map(|node| node.children), Some(0));
}

#[test]
fn a_fold_hides_every_level_below_and_counts_only_direct_children() {
    let items = [Item { key: 1, parent: None, folded: true }, item(2, Some(1)), item(3, Some(2)), item(4, Some(1))];
    let nodes = order(&items);
    assert_eq!(walked(&items), [(1, 0)]);
    assert_eq!(nodes[0].children, 2);
    let mut hidden: Vec<u64> = nodes[0].hidden.iter().map(|at| items[*at].key).collect();
    hidden.sort_unstable();
    assert_eq!(hidden, [2, 3, 4]);
}

#[test]
fn a_child_under_a_fold_is_not_taken_for_a_root() {
    let items = [Item { key: 1, parent: None, folded: true }, item(2, Some(1)), item(3, None)];
    assert_eq!(walked(&items), [(1, 0), (3, 0)]);
}
