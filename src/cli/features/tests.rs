use super::*;

#[test]
fn a_server_has_machine_tunnels_from_the_release_that_brought_them() {
    let on = |patch| Feature::MachineTunnels.on(Some(Version::new(0, 13, patch)));
    assert_eq!(on(66), Has::No(Some(Version::new(0, 13, 66))));
    assert_eq!(on(67), Has::Yes);
    assert_eq!(on(72), Has::Yes);
}

#[test]
fn a_server_has_each_gated_feature_from_the_release_that_brought_it() {
    let features = [
        (Feature::ModeDefault, 28),
        (Feature::FlowCloseWhenMerged, 33),
        (Feature::QueueResults, 51),
        (Feature::FlowFreeBuilds, 64),
        (Feature::Bots, 77),
        (Feature::BotRoutines, 85),
        (Feature::BotSetup, 89),
        (Feature::BotAsks, 101),
        (Feature::BotChatApps, 107),
        (Feature::BotSecrets, 100),
        (Feature::BotAboutRead, 96),
        (Feature::BotFollow, 91),
    ];
    for (feature, patch) in features {
        let before = Version::new(0, 13, patch - 1);
        assert_eq!(feature.on(Some(before)), Has::No(Some(before)), "{feature:?}");
        assert_eq!(feature.on(Some(Version::new(0, 13, patch))), Has::Yes, "{feature:?}");
        assert_eq!(feature.on(Some(Version::new(0, 14, 0))), Has::Yes, "{feature:?}");
    }
}

#[test]
fn a_server_whose_release_cannot_be_read_does_not_have_a_feature() {
    assert_eq!(Feature::MachineTunnels.on(None), Has::No(None));
}

#[test]
fn the_words_for_this_computer_name_what_it_has_on_each_platform() {
    let since = Version::new(0, 13, 33);
    let has = Some(Version::new(0, 13, 27));
    assert_eq!(newer_here(since, has, Os::Mac), "cornercase 0.13.33 or newer on this Mac (it has 0.13.27)");
    assert_eq!(newer_here(since, has, Os::Linux), "cornercase 0.13.33 or newer on this computer (it has 0.13.27)");
    assert_eq!(newer_here(since, has, Os::Windows), "cornercase 0.13.33 or newer on this PC (it has 0.13.27)");
    assert_eq!(newer_here(since, None, Os::Mac), "cornercase 0.13.33 or newer on this Mac");
}

#[test]
fn it_has_says_nothing_for_a_release_that_could_not_be_read() {
    assert_eq!(it_has(Some(Version::new(0, 13, 27))), " (it has 0.13.27)");
    assert_eq!(it_has(None), "");
}

fn all(server: &str, here: Release, machine: Option<&str>) -> Releases<'static> {
    let release = |text: &str| Version::parse(text);
    Releases { server: release(server), here, machine: machine.map(|text| ("vps", release(text))) }
}

#[test]
fn this_computers_release_is_asked_after_the_servers_and_before_the_machines() {
    let old = Release::Read(Some(Version::new(0, 13, 27)));
    let feature = Feature::FlowCloseWhenMerged;
    let server = all("0.13.21", old, Some("0.13.21")).lacks(feature);
    assert_eq!(server, Some(Lacks::Server(Some(Version::new(0, 13, 21)))));
    let here = all("0.13.76", old, Some("0.13.21")).lacks(feature);
    assert_eq!(here, Some(Lacks::ThisComputer(Some(Version::new(0, 13, 27)))));
    let there = all("0.13.76", Release::Read(Some(Version::new(0, 13, 76))), Some("0.13.21")).lacks(feature);
    assert_eq!(there, Some(Lacks::Machine("vps", Some(Version::new(0, 13, 21)))));
}

#[test]
fn an_unread_command_line_is_not_asked() {
    let feature = Feature::FlowFreeBuilds;
    assert_eq!(all("0.13.76", Release::Unread, None).lacks(feature), None);
    let below = all("0.13.76", Release::Unread, Some("0.13.63")).lacks(feature);
    assert_eq!(below, Some(Lacks::Machine("vps", Some(Version::new(0, 13, 63)))));
}

#[test]
fn a_command_line_whose_release_cannot_be_read_lacks_every_feature() {
    for feature in [Feature::FlowCloseWhenMerged, Feature::QueueResults, Feature::FlowFreeBuilds] {
        assert_eq!(all("0.13.76", Release::Read(None), None).lacks(feature), Some(Lacks::ThisComputer(None)));
    }
}

#[test]
fn this_computers_command_line_has_a_feature_from_the_release_that_brought_it() {
    for (feature, patch) in
        [(Feature::FlowCloseWhenMerged, 33), (Feature::QueueResults, 51), (Feature::FlowFreeBuilds, 64)]
    {
        let read = |minor, patch| Release::Read(Some(Version::new(0, minor, patch)));
        let lacks = |here| Releases { server: Some(Version::new(0, 14, 0)), here, machine: None }.lacks(feature);
        assert_eq!(lacks(read(13, patch - 1)), Some(Lacks::ThisComputer(Some(Version::new(0, 13, patch - 1)))));
        assert_eq!(lacks(read(13, patch)), None, "{feature:?}");
        assert_eq!(lacks(read(14, 0)), None, "{feature:?}");
    }
}

#[test]
fn an_older_command_line_lacks_bots_on_a_newer_server() {
    let releases = Releases {
        server: Some(Version::new(0, 13, 80)),
        here: Release::Read(Some(Version::new(0, 13, 73))),
        machine: None,
    };
    assert_eq!(releases.lacks(Feature::Bots), Some(Lacks::ThisComputer(Some(Version::new(0, 13, 73)))));
}

#[test]
fn a_setup_again_asks_the_server_and_then_this_computers_command_line() {
    let read = |patch| Release::Read(Some(Version::new(0, 13, patch)));
    let feature = Feature::BotSetup;
    assert_eq!(all("0.13.88", read(109), None).lacks(feature), Some(Lacks::Server(Some(Version::new(0, 13, 88)))));
    let here = all("0.13.109", read(80), None).lacks(feature);
    assert_eq!(here, Some(Lacks::ThisComputer(Some(Version::new(0, 13, 80)))));
    assert_eq!(all("0.13.89", read(89), None).lacks(feature), None);
    assert_eq!(all("0.13.109", Release::Unread, None).lacks(feature), None);
}

#[test]
fn bot_asks_came_in_0_13_101() {
    let releases = Releases {
        server: Some(Version::new(0, 13, 109)),
        here: Release::Read(Some(Version::new(0, 13, 100))),
        machine: None,
    };
    assert_eq!(releases.lacks(Feature::BotAsks), Some(Lacks::ThisComputer(Some(Version::new(0, 13, 100)))));
    let newer = Releases { here: Release::Read(Some(Version::new(0, 13, 101))), ..releases };
    assert_eq!(newer.lacks(Feature::BotAsks), None);
}
