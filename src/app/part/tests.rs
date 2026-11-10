use super::sidebar_told;
use crate::sidebar::key::SidebarKey;

#[test]
fn the_sidebar_redraws_only_when_its_key_moves() {
    let key = SidebarKey::of_session(0);
    assert!(!sidebar_told(Some(&key), &key));
    assert!(sidebar_told(Some(&key), &SidebarKey::of_session(1)));
}

#[test]
fn the_first_notification_redraws_the_sidebar() {
    assert!(sidebar_told(None, &SidebarKey::of_session(0)));
}
