use super::*;

#[test]
fn without_a_title_the_card_names_the_host_then_the_path() {
    let names = names("", "http://localhost:5173/admin/users?page=2");
    assert_eq!(names, ("localhost:5173".to_string(), "/admin/users?page=2".to_string()));
}

#[test]
fn with_a_title_the_card_names_it_then_the_address() {
    let names = names("Users · Admin", "https://example.com/admin");
    assert_eq!(names, ("Users · Admin".to_string(), "example.com/admin".to_string()));
}
