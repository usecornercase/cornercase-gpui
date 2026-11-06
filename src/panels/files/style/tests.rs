use super::*;

fn span(start: usize, end: usize) -> Range<usize> {
    Range { start, end }
}

#[test]
fn clip_keeps_parts_inside() {
    assert_eq!(clip(&[0..3, 5..9], 4..8), vec![1..4]);
    assert_eq!(clip(&[span(0, 2)], 2..4), Vec::<Range<usize>>::new());
}
