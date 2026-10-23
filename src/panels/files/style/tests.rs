use super::*;

fn span(start: usize, end: usize) -> Range<usize> {
    Range { start, end }
}

#[test]
fn overlay_splits_styles_under_lit_ranges() {
    let red = HighlightStyle { color: Some(hsla(ANSI[1])), ..Default::default() };
    let bg = hsla(ANSI[3]);
    let out = overlay(vec![(0..6, red)], &[span(4, 8)], bg);
    let ranges: Vec<_> = out.iter().map(|(r, _)| r.clone()).collect();
    assert_eq!(ranges, vec![0..4, 4..6, 6..8]);
    assert_eq!(out[1].1.color, red.color);
    assert_eq!(out[1].1.background_color, Some(bg));
    assert_eq!(out[2].1.color, None);
    assert_eq!(overlay(vec![(0..2, red)], &[], bg).len(), 1);
}

#[test]
fn clip_keeps_parts_inside() {
    assert_eq!(clip(&[0..3, 5..9], 4..8), vec![1..4]);
    assert_eq!(clip(&[span(0, 2)], 2..4), Vec::<Range<usize>>::new());
}
