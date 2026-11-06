use super::back_label;

#[test]
fn a_file_from_the_flow_panel_goes_back_to_it() {
    assert_eq!(back_label(true, false), "‹ Flow");
    assert_eq!(back_label(true, true), "‹ Flow");
}

#[test]
fn a_file_from_a_search_goes_back_to_its_results() {
    assert_eq!(back_label(false, true), "‹ Results");
}

#[test]
fn any_other_file_goes_back_to_the_tree() {
    assert_eq!(back_label(false, false), "‹ Files");
}
