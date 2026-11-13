use super::{forget, ports, remember};

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
