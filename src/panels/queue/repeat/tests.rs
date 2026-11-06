use super::{Repeat, words};

#[test]
fn the_servers_texts_read_in_words() {
    assert_eq!(words("30m"), "every 30 minutes");
    assert_eq!(words("2h"), "every 2 hours");
    assert_eq!(words("3d 01:25"), "every 3 days at 01:25");
    assert_eq!(words("2w 10:00"), "every 2 weeks at 10:00");
    assert_eq!(words("day 09:00"), "every day at 09:00");
    assert_eq!(words("weekday 09:00"), "every weekday at 09:00");
    assert_eq!(words("monday 09:00"), "every Monday at 09:00");
}

#[test]
fn a_single_unit_loses_its_number() {
    assert_eq!(words("1h"), "every hour");
    assert_eq!(words("1d 09:00"), "every day at 09:00");
    assert_eq!(words("1w 09:00"), "every week at 09:00");
}

#[test]
fn a_text_from_a_newer_server_still_reads() {
    assert_eq!(words("fortnight 09:00"), "every fortnight 09:00");
    assert_eq!(words("first monday 09:00"), "every first monday 09:00");
    assert_eq!(Repeat::parse("day"), None);
    assert_eq!(Repeat::parse("monday"), None);
    assert_eq!(Repeat::parse("h"), None);
}

#[test]
fn the_clock_comes_from_the_repeat() {
    assert_eq!(Repeat::parse("3d 01:25").as_ref().and_then(Repeat::clock), Some("01:25"));
    assert_eq!(Repeat::parse("friday 18:00").as_ref().and_then(Repeat::clock), Some("18:00"));
    assert_eq!(Repeat::parse("2h").as_ref().and_then(Repeat::clock), None);
}
