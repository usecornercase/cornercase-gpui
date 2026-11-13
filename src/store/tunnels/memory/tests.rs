use super::{forget, local_in, note_local, ports, remember};

#[test]
fn each_destination_keeps_its_own_ports() {
    let text = remember(&remember("", "devbox", 5173), "other", 3000);
    assert_eq!(ports(&text, "devbox"), vec![5173]);
    assert_eq!(ports(&text, "other"), vec![3000]);
    assert_eq!(ports(&text, "third"), Vec::<u16>::new());
}

#[test]
fn remembering_twice_keeps_one() {
    let text = remember(&remember(&remember("", "devbox", 5173), "devbox", 3000), "devbox", 5173);
    assert_eq!(ports(&text, "devbox"), vec![3000, 5173]);
}

#[test]
fn forgetting_drops_only_that_port() {
    let text = remember(&remember("", "devbox", 5173), "devbox", 3000);
    assert_eq!(ports(&forget(&text, "devbox", 5173), "devbox"), vec![3000]);
    assert!(!forget(&remember("", "devbox", 5173), "devbox", 5173).contains("devbox"));
}

#[test]
fn a_broken_entry_does_not_lose_the_others() {
    let text = r#"{"devbox":[3000,"x",70000,0,5173],"other":"nope","third":[1]}"#;
    assert_eq!(ports(text, "devbox"), vec![3000, 5173]);
    assert_eq!(ports(text, "other"), Vec::<u16>::new());
    assert_eq!(ports(&remember(text, "devbox", 8080), "third"), vec![1]);
}

#[test]
fn a_file_that_is_no_object_is_started_afresh() {
    assert_eq!(ports(&remember("[1,2]", "devbox", 5173), "devbox"), vec![5173]);
    assert_eq!(ports(&remember("not json", "devbox", 5173), "devbox"), vec![5173]);
}

#[test]
fn at_most_64_ports_are_kept() {
    let text = (1..=70).fold(String::new(), |text, port| remember(&text, "devbox", port));
    assert_eq!(ports(&text, "devbox").len(), 64);
}

#[test]
fn an_older_windows_file_is_read_with_no_local_port() {
    let text = r#"{"devbox":[3000,5173]}"#;
    assert_eq!(ports(text, "devbox"), vec![3000, 5173]);
    assert_eq!(local_in(text, "devbox", 5173), None);
}

#[test]
fn a_noted_local_port_is_read_back() {
    let text = note_local(&remember("", "devbox", 5173), "devbox", 5173, 54012).expect("it changes");
    assert_eq!(local_in(&text, "devbox", 5173), Some(54012));
    assert!(text.contains("\"local\": 54012"));
}

#[test]
fn noting_a_port_that_is_not_remembered_writes_nothing() {
    assert_eq!(note_local("", "devbox", 5173, 54012), None);
    assert_eq!(note_local(&remember("", "devbox", 3000), "devbox", 5173, 54012), None);
}

#[test]
fn noting_the_same_local_port_again_writes_nothing() {
    let text = note_local(&remember("", "devbox", 5173), "devbox", 5173, 54012).expect("it changes");
    assert_eq!(note_local(&text, "devbox", 5173, 54012), None);
    assert!(note_local(&text, "devbox", 5173, 54013).is_some());
}

#[test]
fn remembering_a_port_again_keeps_its_local_port() {
    let text = note_local(&remember("", "devbox", 5173), "devbox", 5173, 54012).expect("it changes");
    assert_eq!(local_in(&remember(&text, "devbox", 5173), "devbox", 5173), Some(54012));
}

#[test]
fn forgetting_drops_the_local_port_with_its_port() {
    let text = note_local(&remember("", "devbox", 5173), "devbox", 5173, 54012).expect("it changes");
    let text = remember(&forget(&text, "devbox", 5173), "devbox", 5173);
    assert_eq!(local_in(&text, "devbox", 5173), None);
}

#[test]
fn a_broken_local_port_keeps_its_port() {
    let text = r#"{"d":[{"port":5173,"local":0},{"port":5174,"local":"x"},{"port":5175,"local":70000},{"local":9},{"port":3000,"local":3001}]}"#;
    assert_eq!(ports(text, "d"), vec![3000, 5173, 5174, 5175]);
    assert_eq!(local_in(text, "d", 5173), None);
    assert_eq!(local_in(text, "d", 5175), None);
    assert_eq!(local_in(text, "d", 3000), Some(3001));
}

#[test]
fn a_repeated_port_keeps_the_local_port_one_of_them_had() {
    let text = r#"{"d":[5173,{"port":5173,"local":54012}]}"#;
    assert_eq!(ports(text, "d"), vec![5173]);
    assert_eq!(local_in(text, "d", 5173), Some(54012));
}
