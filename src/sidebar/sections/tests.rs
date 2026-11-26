use super::*;

const ARRANGEMENTS: [Arrangement; 4] =
    [Arrangement::Tree, Arrangement::ProjectsOnTop, Arrangement::WorkspacesOnTop, Arrangement::SideBySide];
const STACKED: [Arrangement; 2] = [Arrangement::ProjectsOnTop, Arrangement::WorkspacesOnTop];

fn folds(projects: bool, workspaces: bool, agents: bool) -> SectionFolds {
    SectionFolds { projects, workspaces, agents }
}

fn agents_room(arrangement: Arrangement, folds: SectionFolds) -> AgentsRoom {
    Room::of(arrangement, folds, true).agents
}

#[test]
fn the_agents_line_drags_only_while_a_list_above_it_is_open() {
    for arrangement in ARRANGEMENTS {
        assert_eq!(agents_room(arrangement, folds(false, false, false)), AgentsRoom::Split, "{arrangement:?}");
    }
    for arrangement in STACKED {
        for one_open in [folds(true, false, false), folds(false, true, false)] {
            assert_eq!(agents_room(arrangement, one_open), AgentsRoom::Split, "{arrangement:?} {one_open:?}");
        }
    }
}

#[test]
fn a_folded_list_gives_its_room_to_the_agents() {
    assert_eq!(agents_room(Arrangement::Tree, folds(true, false, false)), AgentsRoom::Rest);
    for arrangement in STACKED {
        assert_eq!(agents_room(arrangement, folds(true, true, false)), AgentsRoom::Rest, "{arrangement:?}");
    }
    assert_eq!(agents_room(Arrangement::SideBySide, folds(false, true, false)), AgentsRoom::Rest);
}

#[test]
fn folded_agents_keep_only_their_title() {
    for arrangement in ARRANGEMENTS {
        for lists in [folds(false, false, true), folds(true, true, true)] {
            assert_eq!(agents_room(arrangement, lists), AgentsRoom::Title, "{arrangement:?} {lists:?}");
        }
    }
}

#[test]
fn the_stack_line_drags_only_while_both_lists_are_open() {
    for arrangement in STACKED {
        let drags = |folds| Room::of(arrangement, folds, true).stack_drags;
        assert!(drags(folds(false, false, true)), "{arrangement:?}");
        for folded in [folds(true, false, false), folds(false, true, false), folds(true, true, false)] {
            assert!(!drags(folded), "{arrangement:?} {folded:?}");
        }
    }
}

#[test]
fn only_stacked_lists_have_a_stack_line() {
    for arrangement in [Arrangement::Tree, Arrangement::SideBySide] {
        assert!(!Room::of(arrangement, SectionFolds::default(), true).stack_drags, "{arrangement:?}");
    }
}

#[test]
fn side_by_side_folded_projects_leave_the_workspaces_card_as_it_was() {
    let open = Room::of(Arrangement::SideBySide, folds(false, false, false), true);
    assert_eq!(Room::of(Arrangement::SideBySide, folds(true, false, false), true), open);
}

#[test]
fn the_tree_ignores_the_workspaces_fold() {
    let open = Room::of(Arrangement::Tree, folds(false, false, false), true);
    assert_eq!(Room::of(Arrangement::Tree, folds(false, true, false), true), open);
}

#[test]
fn with_the_agents_section_off_its_fold_changes_nothing() {
    for arrangement in ARRANGEMENTS {
        let without = |folds| Room::of(arrangement, folds, false);
        assert_eq!(without(folds(false, false, true)), without(folds(false, false, false)), "{arrangement:?}");
        assert_eq!(without(folds(true, true, true)).agents, AgentsRoom::Hidden, "{arrangement:?}");
    }
}

fn tags(count: usize, attention: Option<&str>) -> SectionTags<'_> {
    SectionTags { count, attention }
}

fn shown(count: Option<usize>, mark: Option<&str>) -> Figures<'_> {
    Figures { count, mark }
}

#[test]
fn an_open_title_of_a_list_says_nothing_at_its_end() {
    for section in [Section::Projects, Section::Workspaces] {
        let figures = Figures::of(section, false, true, tags(3, Some("waiting")));
        assert_eq!(figures, shown(None, None), "{section:?}");
    }
}

#[test]
fn a_folded_title_says_how_many_rows_it_hides_and_what_needs_you() {
    for section in [Section::Projects, Section::Workspaces, Section::Agents] {
        let figures = Figures::of(section, true, true, tags(3, Some("waiting")));
        assert_eq!(figures, shown(Some(3), Some("waiting")), "{section:?}");
    }
}

#[test]
fn a_folded_list_title_leaves_its_count_out_while_counts_are_off() {
    for section in [Section::Projects, Section::Workspaces] {
        let figures = Figures::of(section, true, false, tags(3, Some("done")));
        assert_eq!(figures, shown(None, Some("done")), "{section:?}");
    }
}

#[test]
fn the_agents_title_keeps_its_count_open_and_folded_whatever_counts_says() {
    for folded in [false, true] {
        let figures = Figures::of(Section::Agents, folded, false, tags(2, None));
        assert_eq!(figures, shown(Some(2), None), "folded {folded}");
    }
}
