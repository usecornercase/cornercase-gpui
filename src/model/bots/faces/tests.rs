use super::*;

#[test]
fn a_bot_with_no_kept_face_gets_the_colour_of_its_id() {
    assert_eq!(Faces::default().of(&BotKey::here("prs")), face_colour("prs"));
}

#[test]
fn a_kept_face_is_the_bot_s_colour() {
    let mut faces = Faces::default();
    assert!(faces.set(&BotKey::here("prs"), 13));
    assert_eq!(faces.of(&BotKey::here("prs")), 13);
}

#[test]
fn a_machine_s_bot_keeps_its_face_apart_from_one_here_with_the_same_id() {
    let mut faces = Faces::default();
    faces.set(&BotKey::on("vps", "prs"), 13);
    assert_eq!(faces.of(&BotKey::on("vps", "prs")), 13);
    assert_eq!(faces.of(&BotKey::here("prs")), face_colour("prs"));
}

#[test]
fn setting_the_same_colour_again_changes_nothing() {
    let mut faces = Faces::default();
    faces.set(&BotKey::here("prs"), 13);
    assert!(!faces.set(&BotKey::here("prs"), 13));
}

#[test]
fn a_colour_outside_the_faces_is_refused() {
    let mut faces = Faces::default();
    assert!(!faces.set(&BotKey::here("prs"), 0));
    assert!(!faces.set(&BotKey::here("prs"), 7));
}

#[test]
fn a_kept_colour_outside_the_faces_reads_as_none() {
    let faces = Faces::parse(r#"{"here": {"prs": 15}}"#);
    assert_eq!(faces.of(&BotKey::here("prs")), face_colour("prs"));
}

#[test]
fn faces_survive_their_file() {
    let mut faces = Faces::default();
    faces.set(&BotKey::here("prs"), 13);
    faces.set(&BotKey::on("vps", "news"), 2);
    assert_eq!(Faces::parse(&faces.text()), faces);
}

#[test]
fn a_file_that_does_not_read_is_no_faces() {
    assert_eq!(Faces::parse("not json"), Faces::default());
}

#[test]
fn a_forgotten_face_goes_back_to_the_id_s_colour() {
    let mut faces = Faces::default();
    faces.set(&BotKey::here("prs"), 13);
    assert!(faces.forget(&BotKey::here("prs")));
    assert_eq!(faces.of(&BotKey::here("prs")), face_colour("prs"));
}

#[test]
fn a_name_s_face_ignores_case_and_blanks() {
    assert_eq!(face_of_name("  PRs "), face_colour("prs"));
}
