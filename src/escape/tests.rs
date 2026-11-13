use super::*;

const ALL_TOPS: [Top; 5] = [Top::Choice, Top::Menu, Top::Pick, Top::Find, Top::Selection];

fn scene(focus: Focus, tops: &[Top]) -> Scene {
    Scene { focus, tops: tops.to_vec(), ..Scene::default() }
}

fn after(mut scene: Scene, step: Step) -> Scene {
    match step {
        Step::CancelDrag => scene.drag = false,
        Step::CloseDialog => scene.dialog = false,
        Step::CloseSheet => scene.sheet = false,
        Step::Close(top) => scene.tops.retain(|t| *t != top),
        Step::LeaveFullScreen => {
            scene.full_screen = false;
            scene.focus = Focus::Dock;
        }
        Step::KeysBack => scene.focus = Focus::Terminal,
        Step::ToPage | Step::ToProgram => {}
    }
    if matches!(step, Step::CloseDialog | Step::CloseSheet) && !scene.dialog && !scene.sheet {
        scene.focus = if scene.full_screen { Focus::Dock } else { Focus::Terminal };
    }
    scene
}

fn presses(mut scene: Scene) -> Vec<Step> {
    let mut steps = Vec::new();
    for _ in 0..20 {
        let step = scene.step();
        steps.push(step);
        if matches!(step, Step::ToPage | Step::ToProgram) {
            return steps;
        }
        scene = after(scene, step);
    }
    panic!("Escape never reached a program or a page: {steps:?}");
}

fn changed(before: &Scene, after: &Scene, step: Step) -> usize {
    usize::from(step == Step::KeysBack && before.focus != after.focus)
        + usize::from(before.drag != after.drag)
        + usize::from(before.dialog != after.dialog)
        + usize::from(before.sheet != after.sheet)
        + usize::from(before.tops != after.tops)
        + usize::from(before.full_screen != after.full_screen)
}

#[test]
fn a_pane_drag_is_cancelled_before_anything_else() {
    let drag = Scene { drag: true, dialog: true, sheet: true, ..scene(Focus::Dock, &[Top::Menu]) };
    assert_eq!(presses(drag)[..2], [Step::CancelDrag, Step::CloseDialog]);
}

#[test]
fn a_dialog_closes_before_the_sheet_under_it_one_per_press() {
    let both = Scene { dialog: true, sheet: true, ..scene(Focus::Dock, &[]) };
    assert_eq!(presses(both), [Step::CloseDialog, Step::CloseSheet, Step::ToProgram]);
}

#[test]
fn a_menu_on_a_docked_panel_closes_alone_then_the_keys_go_back() {
    assert_eq!(presses(scene(Focus::Dock, &[Top::Menu])), [Step::Close(Top::Menu), Step::KeysBack, Step::ToProgram]);
}

#[test]
fn no_escape_closes_a_docked_panel() {
    for focus in [Focus::Dock, Focus::Elsewhere] {
        let steps = presses(scene(focus, &ALL_TOPS));
        let at = steps.len() - 2;
        assert_eq!(steps[at..], [Step::KeysBack, Step::ToProgram]);
        assert_eq!(steps.iter().filter(|s| **s == Step::KeysBack).count(), 1);
    }
}

#[test]
fn the_chooser_closes_before_the_picked_card_under_it() {
    let steps = presses(scene(Focus::Dock, &[Top::Pick, Top::Menu]));
    assert_eq!(steps, [Step::Close(Top::Menu), Step::Close(Top::Pick), Step::KeysBack, Step::ToProgram]);
}

#[test]
fn the_browser_closes_its_select_menu_first() {
    let steps = presses(scene(Focus::Dock, &[Top::Pick, Top::Menu, Top::Choice]));
    let closing = [Top::Choice, Top::Menu, Top::Pick].map(Step::Close);
    assert_eq!(steps, [closing[0], closing[1], closing[2], Step::KeysBack, Step::ToProgram]);
}

#[test]
fn the_page_gets_escape_once_nothing_of_the_panel_is_over_it() {
    assert_eq!(presses(scene(Focus::Page, &[])), [Step::ToPage]);
    let over = presses(scene(Focus::Page, &[Top::Choice, Top::Pick]));
    assert_eq!(over, [Step::Close(Top::Choice), Step::Close(Top::Pick), Step::ToPage]);
}

#[test]
fn files_closes_its_menu_find_and_selection_then_full_screen() {
    let files = Scene { full_screen: true, ..scene(Focus::Dock, &[Top::Selection, Top::Find, Top::Menu]) };
    let closing = [Top::Menu, Top::Find, Top::Selection].map(Step::Close);
    let expected = [closing[0], closing[1], closing[2], Step::LeaveFullScreen, Step::KeysBack, Step::ToProgram];
    assert_eq!(presses(files), expected);
}

#[test]
fn escape_in_the_terminal_reaches_the_program_whatever_a_panel_has_open() {
    assert_eq!(presses(scene(Focus::Terminal, &[Top::Pick, Top::Menu, Top::Selection])), [Step::ToProgram]);
}

#[test]
fn a_file_full_screen_ends_wherever_the_focus_is() {
    let terminal = Scene { full_screen: true, ..scene(Focus::Terminal, &[]) };
    assert_eq!(presses(terminal), [Step::LeaveFullScreen, Step::KeysBack, Step::ToProgram]);
    let selected = Scene { full_screen: true, ..scene(Focus::Terminal, &[Top::Selection]) };
    let expected = [Step::LeaveFullScreen, Step::Close(Top::Selection), Step::KeysBack, Step::ToProgram];
    assert_eq!(presses(selected), expected);
    let elsewhere = Scene { full_screen: true, ..scene(Focus::Elsewhere, &[]) };
    assert_eq!(presses(elsewhere), [Step::LeaveFullScreen, Step::KeysBack, Step::ToProgram]);
}

#[test]
fn the_sidebar_gives_the_keys_back_and_leaves_a_panel_alone() {
    assert_eq!(presses(scene(Focus::Elsewhere, &[Top::Menu])), [Step::KeysBack, Step::ToProgram]);
}

fn every_scene() -> Vec<Scene> {
    let focuses = [Focus::Terminal, Focus::Page, Focus::Dock, Focus::Elsewhere];
    let mut scenes = Vec::new();
    for focus in focuses {
        for flags in 0..8u8 {
            for open in 0..32u8 {
                let tops: Vec<Top> =
                    ALL_TOPS.iter().enumerate().filter(|(i, _)| open >> i & 1 == 1).map(|(_, t)| *t).collect();
                let base = Scene {
                    focus,
                    drag: flags & 1 == 1,
                    dialog: flags & 2 == 2,
                    sheet: flags & 4 == 4,
                    tops,
                    full_screen: false,
                };
                scenes.push(Scene { full_screen: true, ..base.clone() });
                scenes.push(base);
            }
        }
    }
    scenes
}

#[test]
fn every_press_takes_exactly_one_step() {
    for scene in every_scene() {
        let steps = presses(scene.clone());
        let last = steps.len() - 1;
        assert!(matches!(steps[last], Step::ToPage | Step::ToProgram), "{scene:?}");
        let mut now = scene;
        for step in &steps[..last] {
            let next = after(now.clone(), *step);
            assert_eq!(changed(&now, &next, *step), 1, "{step:?} on {now:?}");
            now = next;
        }
    }
}

#[test]
fn the_keys_go_back_at_most_once_and_no_press_closes_a_panel() {
    for scene in every_scene() {
        let steps = presses(scene);
        assert!(steps.iter().filter(|step| **step == Step::KeysBack).count() <= 1);
    }
}

#[test]
fn the_first_top_is_the_one_drawn_over_the_others() {
    assert_eq!(first(&[Top::Selection, Top::Pick, Top::Menu]), Some(Top::Menu));
    assert_eq!(first(&[]), None);
}

#[test]
fn only_the_open_tops_are_listed() {
    assert_eq!(tops_of(&[(Top::Menu, true), (Top::Pick, false)]), [Top::Menu]);
}
