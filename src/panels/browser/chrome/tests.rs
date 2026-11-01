use std::path::{Path, PathBuf};

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
    let args = args(Path::new("/tmp/profile"), 9333);
    assert!(args.contains(&"--headless".to_string()));
    assert!(args.contains(&"--user-data-dir=/tmp/profile".to_string()));
    assert!(args.contains(&"--remote-debugging-port=9333".to_string()));
    assert_eq!(args.last().map(String::as_str), Some("--no-startup-window"));
}

#[test]
fn the_singleton_lock_names_the_browser_pid() {
    assert_eq!(lock_pid("my-mac.local-48213"), Some(48213));
    assert_eq!(lock_pid("nohyphen"), None);
}

#[cfg(target_os = "macos")]
#[test]
fn on_a_mac_chrome_is_looked_for_in_both_applications_folders() {
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
