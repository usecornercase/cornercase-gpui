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
