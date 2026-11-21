use super::*;
use crate::theme::code::inks;
use crate::theme::current::fallback;
use crate::ui::highlight::Kind;

#[test]
fn each_kind_takes_its_ink() {
    let inks = inks(&fallback(), 1.);
    let styled = styles(&[(0..2, Kind::Keyword), (3..9, Kind::Comment), (10..12, Kind::Removed)], &inks);
    assert_eq!(styled[0], (0..2, HighlightStyle { color: Some(inks.ansi[5].into()), ..Default::default() }));
    assert_eq!(styled[1].1.color, Some(inks.comment.into()));
    assert_eq!(styled[1].1.font_style, Some(FontStyle::Italic));
    assert_eq!(styled[2].1.color, Some(inks.ansi[1].into()));
}
