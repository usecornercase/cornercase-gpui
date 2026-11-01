use super::*;

const WIDTH: f32 = 200.;
const FIGURES: f32 = 40.;

fn spans(buttons: u8) -> Vec<(f32, f32)> {
    (0..buttons).map(|slot| (-(button_margin(slot) + BUTTON), -button_margin(slot))).collect()
}

fn figures_start(end: End) -> f32 {
    if end.figures { WIDTH - FIGURES } else { WIDTH }
}

fn name_end(end: End, hovered: bool) -> f32 {
    figures_start(end) - end.gap - end_margin(end, hovered)
}

fn ends() -> Vec<End> {
    [(1, true), (1, false), (2, true), (2, false)]
        .into_iter()
        .map(|(buttons, figures)| End { gap: 5., buttons, figures })
        .collect()
}

#[test]
fn entering_a_row_hovers_it() {
    assert_eq!(after_hover(Some("ws-1".into()), "ws-2".into(), true), Some("ws-2".into()));
}

#[test]
fn leaving_the_hovered_row_clears_it() {
    assert_eq!(after_hover(Some("ws-1".into()), "ws-1".into(), false), None);
}

#[test]
fn leaving_a_row_after_entering_the_next_keeps_the_next() {
    assert_eq!(after_hover(Some("ws-2".into()), "ws-1".into(), false), Some("ws-2".into()));
}

#[test]
fn a_rows_buttons_stand_apart_and_left_of_its_figures() {
    for buttons in 1..=2 {
        let spans = spans(buttons);
        assert!(spans.iter().all(|(_, end)| *end < 0.), "{spans:?} reach the figures");
        assert!(spans.windows(2).all(|pair| pair[1].1 + BUTTON_GAP <= pair[0].0), "{spans:?} overlap");
    }
}

#[test]
fn a_hovered_rows_name_ends_a_gap_before_its_buttons() {
    for end in ends() {
        let leftmost = spans(end.buttons).iter().map(|(start, _)| figures_start(end) + start).fold(WIDTH, f32::min);
        let gap = leftmost - name_end(end, true);
        assert!((gap - end.gap).abs() < f32::EPSILON, "{gap} before the buttons of {:?}", (end.buttons, end.figures));
    }
}

#[test]
fn a_resting_rows_name_runs_to_its_figures_or_the_rows_end() {
    for end in ends() {
        let reach = if end.figures { WIDTH - FIGURES - end.gap } else { WIDTH };
        assert!((name_end(end, false) - reach).abs() < f32::EPSILON, "{:?}", (end.buttons, end.figures));
    }
}
