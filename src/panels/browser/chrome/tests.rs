use std::path::Path;

use super::*;

#[test]
fn the_active_port_file_gives_the_browser_address() {
    let endpoint = active_port("9222\n/devtools/browser/1b2c\n").expect("parses");
    assert_eq!(endpoint, Endpoint { port: 9222, url: "ws://127.0.0.1:9222/devtools/browser/1b2c".into() });
}

#[test]
fn an_active_port_file_without_a_path_is_not_ready() {
    assert_eq!(active_port("9222\n"), None);
    assert_eq!(active_port("0\n/devtools/browser/x\n"), None);
}

#[test]
fn chrome_runs_headless_on_its_own_profile_and_port_without_a_first_page() {
    let args = args(Os::CURRENT, Path::new("/tmp/profile"), 9333);
    assert!(args.contains(&"--headless".to_string()));
    assert!(args.contains(&"--user-data-dir=/tmp/profile".to_string()));
    assert!(args.contains(&"--remote-debugging-port=9333".to_string()));
    assert_eq!(args.last().map(String::as_str), Some("--no-startup-window"));
}

#[test]
fn chrome_never_asks_for_the_keychain_or_the_keyring() {
    let mac = args(Os::Mac, Path::new("/p"), 9222);
    assert!(mac.contains(&"--use-mock-keychain".to_string()));
    let linux = args(Os::Linux, Path::new("/p"), 9222);
    assert!(linux.contains(&"--password-store=basic".to_string()));
    assert!(!linux.contains(&"--use-mock-keychain".to_string()));
}

#[test]
fn a_snap_is_known_by_its_command_or_by_the_script_that_runs_it() {
    let snap = Path::new("/usr/bin/snap");
    let command = |exe: &str, real: &Path, head: &[u8]| snap_command(Path::new(exe), real, head);
    assert_eq!(command("/snap/bin/chromium", snap, b"\x7fELF").as_deref(), Some("chromium"));
    assert_eq!(command("/home/me/bin/chromium.chromedriver", snap, b"").as_deref(), Some("chromium.chromedriver"));
    let wrapper = b"#!/bin/sh\nif ! [ -x /snap/bin/chromium ]; then exit 1; fi\nexec /snap/bin/chromium \"$@\"\n";
    let real = Path::new("/usr/bin/chromium-browser");
    assert_eq!(command("/usr/bin/chromium-browser", real, wrapper).as_deref(), Some("chromium"));
    assert_eq!(command("/usr/bin/x", real, b"#!/bin/sh\n  \"/snap/bin/brave\" \"$@\"\n").as_deref(), Some("brave"));
    let debian = Path::new("/usr/lib/chromium/chromium");
    assert_eq!(command("/usr/bin/chromium", debian, b"\x7fELF"), None);
    let plain_script = b"#!/bin/sh\nexec /usr/lib/chromium/chromium \"$@\"\n";
    assert_eq!(command("/usr/bin/chromium-browser", real, plain_script), None);
}

#[test]
fn a_script_that_only_checks_for_a_snap_runs_its_own_browser() {
    let real = Path::new("/usr/local/bin/chromium");
    let script = b"#!/bin/sh\nif [ -x /snap/bin/chromium ]; then echo 'the snap is here too' >&2; fi\n\
                   exec /usr/lib/chromium/chromium \"$@\"\n";
    assert_eq!(snap_command(real, real, script), None);
}

#[test]
fn a_snap_is_named_by_its_command_or_by_the_command_its_alias_points_to() {
    assert_eq!(snap_name("chromium", Some(Path::new("/usr/bin/snap"))).as_deref(), Some("chromium"));
    assert_eq!(snap_name("chromium.chromedriver", None).as_deref(), Some("chromium"));
    assert_eq!(snap_name("google-chrome", Some(Path::new("chromium"))).as_deref(), Some("chromium"));
    assert_eq!(snap_name("chromedriver", Some(Path::new("chromium.chromedriver"))).as_deref(), Some("chromium"));
    assert_eq!(snap_name("", None), None);
}

#[test]
fn a_browser_that_stops_as_it_starts_gives_its_last_error() {
    let said = "\n[5887:5887:1104/112642.487358:ERROR:zygote_host_impl_linux.cc:130] No usable sandbox! Install it.\n\
                [5887:5900:1104/112642.5:WARNING:bus.cc:408] Failed to connect to the bus\nDevTools listening on ws://x\n";
    assert_eq!(last_error(said).as_deref(), Some("No usable sandbox! Install it."));
    assert_eq!(last_error("DevTools listening on ws://127.0.0.1:9222/devtools/browser/x\n"), None);
}

#[test]
fn a_snap_keeps_its_profile_where_its_confinement_lets_it_write() {
    let profile = snap_profile(Path::new("/home/me"), "chromium");
    assert_eq!(profile, Path::new("/home/me/snap/chromium/common/cornercase-gpui-browser"));
}

#[test]
fn the_browser_and_its_profile_are_chosen_once_one_is_found() {
    use std::cell::Cell;
    use std::path::PathBuf;

    let cell = OnceLock::new();
    assert!(chosen_in(&cell, || None).is_none());
    let found = chosen_in(&cell, || Some(PathBuf::from("/nowhere/chromium"))).expect("a browser");
    assert_eq!(found.exe, Path::new("/nowhere/chromium"));
    let looked = Cell::new(false);
    let kept = chosen_in(&cell, || {
        looked.set(true);
        Some(PathBuf::from("/elsewhere/chrome"))
    });
    assert_eq!(kept.map(|chosen| chosen.exe.as_path()), Some(Path::new("/nowhere/chromium")));
    assert!(!looked.get());
}

#[test]
fn the_profile_is_the_one_chosen_with_the_browser() {
    use std::path::PathBuf;

    let cell = OnceLock::new();
    let chosen = Chosen { exe: PathBuf::from("/nowhere/chromium"), profile: PathBuf::from("/chosen/profile") };
    assert!(cell.set(chosen).is_ok());
    assert_eq!(profile_in(&cell), Path::new("/chosen/profile"));
}

#[test]
fn the_singleton_lock_names_the_browser_pid() {
    assert_eq!(lock_pid("my-mac.local-48213"), Some(48213));
    assert_eq!(lock_pid("nohyphen"), None);
}

#[cfg(target_os = "macos")]
#[test]
fn on_a_mac_chrome_is_looked_for_in_both_applications_folders() {
    use std::path::PathBuf;

    let found = candidates(Path::new("/Users/me"));
    assert!(found.contains(&PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")));
    assert!(found.contains(&PathBuf::from("/Users/me/Applications/Chromium.app/Contents/MacOS/Chromium")));
}

#[test]
fn the_version_answer_names_the_browser_socket_on_localhost() {
    let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"Browser\": \"Chrome/156\", \"webSocketDebuggerUrl\": \"ws://localhost:9222/devtools/browser/abc\"}";
    assert_eq!(
        browser_url(response, 9222),
        Some(Endpoint { port: 9222, url: "ws://127.0.0.1:9222/devtools/browser/abc".into() })
    );
}

#[test]
fn an_answer_without_a_socket_is_not_a_browser() {
    assert_eq!(browser_url("HTTP/1.1 404 Not Found\r\n\r\nnope", 9222), None);
}

#[test]
fn a_response_is_read_up_to_its_length_even_on_an_open_connection() {
    let text = "HTTP/1.1 200 OK\r\nContent-Length:5\r\n\r\nhello and more";
    assert_eq!(read_response(&mut std::io::Cursor::new(text)).as_deref(), Some("hello"));
}
