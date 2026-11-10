use super::Revised;

#[test]
fn reading_keeps_the_revision() {
    let value = Revised::new(Some(3));
    let before = value.revision();
    assert_eq!(value.as_ref(), Some(&3));
    assert_eq!(value.revision(), before);
}

#[test]
fn setting_moves_the_revision() {
    let mut value = Revised::new(None);
    let before = value.revision();
    value.set(Some(1));
    assert_ne!(value.revision(), before);
}

#[test]
fn a_change_through_get_mut_moves_the_revision() {
    let mut value = Revised::new(vec![1]);
    let before = value.revision();
    value.get_mut().push(2);
    assert_ne!(value.revision(), before);
    assert_eq!(value.len(), 2);
}
