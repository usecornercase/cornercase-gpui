use super::{loopback_port, pick_local, rewrite, wanted};

#[test]
fn a_url_on_this_machine_names_its_port() {
    assert_eq!(loopback_port("http://localhost:3000/admin"), Some(3000));
    assert_eq!(loopback_port("https://127.0.0.1:8443"), Some(8443));
    assert_eq!(loopback_port("http://[::1]:5173?x=1"), Some(5173));
    assert_eq!(loopback_port("http://0.0.0.0:8000#top"), Some(8000));
}

#[test]
fn a_url_elsewhere_or_without_a_port_names_none() {
    assert_eq!(loopback_port("https://usecornercase.dev:443"), None);
    assert_eq!(loopback_port("http://localhost/"), None);
    assert_eq!(loopback_port("ws://localhost:3000"), None);
    assert_eq!(loopback_port("about:blank"), None);
}

#[test]
fn rewriting_moves_the_url_to_the_local_end_and_keeps_the_rest() {
    assert_eq!(rewrite("http://localhost:3000/a/b?c=1#d", 61234), "http://localhost:61234/a/b?c=1#d");
    assert_eq!(rewrite("https://127.0.0.1:8443", 8443), "https://localhost:8443");
}

#[test]
fn the_tunnel_keeps_the_remote_number_when_it_is_free_here() {
    assert_eq!(pick_local(3000, None, |_| true, || Some(50000)), Some(3000));
}

#[test]
fn a_taken_number_falls_back_to_a_free_port() {
    assert_eq!(pick_local(3000, None, |p| p != 3000, || Some(50000)), Some(50000));
}

#[test]
fn a_restarted_tunnel_keeps_its_local_port_when_it_can() {
    assert_eq!(pick_local(3000, Some(50000), |_| true, || Some(50001)), Some(50000));
    assert_eq!(pick_local(3000, Some(50000), |p| p != 50000, || Some(50001)), Some(3000));
}

#[test]
fn no_free_port_means_no_tunnel() {
    assert_eq!(pick_local(3000, None, |_| false, || Some(50000)), None);
}

#[test]
fn the_workspace_ports_and_pending_opens_are_tunnelled() {
    let kept = wanted(&[3000, 8000], &[9000], &[], &[]);
    assert_eq!(kept.into_iter().collect::<Vec<_>>(), [3000, 8000, 9000]);
}

#[test]
fn a_tunnel_a_tab_still_uses_stays_after_its_port_goes() {
    let tunnels = [(3000, 50000), (8000, 8000)];
    let kept = wanted(&[], &[], &tunnels, &["http://localhost:50000/app", "https://example.com"]);
    assert_eq!(kept.into_iter().collect::<Vec<_>>(), [3000]);
}
