use super::*;

#[test]
fn the_version_line_names_the_program() {
    assert_eq!(version_line(), format!("cornercase-gpui {}", env!("CARGO_PKG_VERSION")));
}
