use super::*;
use crate::panels::settings::data;

#[test]
fn appearance_is_found_by_theme_dark_or_a_preset_s_name() {
    for query in ["theme", "dark", "Gruvbox", "nord", "rosé pine", "contrast", "colours"] {
        assert!(found_for(query, "Nord").is_some(), "{query}");
    }
}

#[test]
fn appearance_is_not_found_by_words_it_does_not_hold() {
    assert!(found_for("worktrees", "Nord").is_none());
    assert!(found_for("", "Nord").is_none());
}

#[test]
fn a_found_appearance_is_a_page_with_the_theme_in_use_as_its_value() {
    let found = found_for("theme", "Nord").expect("found");
    assert!(found.is_tab());
    assert_eq!((found.page.as_str(), found.shown.as_str()), (data::APPEARANCE, "Nord"));
}

#[test]
fn the_sheet_s_own_pages_follow_the_line_appearance_first() {
    assert_eq!(data::OWN_PAGES[0], data::APPEARANCE);
    assert_eq!(data::OWN_PAGES.len(), 4);
}

#[test]
fn every_page_of_the_sheet_says_what_it_holds() {
    for page in data::OWN_PAGES {
        assert!(!data::about(page).is_empty(), "{page}");
    }
}

#[test]
fn four_cards_share_a_line_where_four_fit() {
    assert_eq!(gallery::per_line(686.), 4);
    assert_eq!(gallery::per_line(710.), 4);
}

#[test]
fn a_pixel_under_four_cards_a_line_holds_three() {
    assert_eq!(gallery::per_line(685.), 3);
}

#[test]
fn the_least_card_holds_one_theme_a_line() {
    assert_eq!(gallery::per_line(229.), 1);
}

#[test]
fn a_card_of_no_width_still_holds_one_theme_a_line() {
    assert_eq!(gallery::per_line(0.), 1);
}
