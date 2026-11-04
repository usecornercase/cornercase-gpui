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
    let args = args(Os::CURRENT, Path::new("/tmp/profile"), 9333);
    assert!(args.contains(&"--headless".into()));
    assert!(args.contains(&"--user-data-dir=/tmp/profile".into()));
    assert!(args.contains(&"--remote-debugging-port=9333".into()));
    assert_eq!(args.last().and_then(|arg| arg.to_str()), Some("--no-startup-window"));
}

#[cfg(unix)]
#[test]
fn a_profile_whose_path_is_not_utf8_reaches_chrome_byte_for_byte() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt as _;

    let profile = Path::new(OsStr::from_bytes(b"/home/caf\xe9/snap/chromium/common/cornercase-gpui-browser"));
    let args = args(Os::Linux, profile, 9222);
    let dir = args.iter().find(|arg| arg.as_bytes().starts_with(b"--user-data-dir="));
    assert_eq!(dir.map(|arg| &arg.as_bytes()[16..]), Some(profile.as_os_str().as_bytes()));
}

#[test]
fn chrome_never_asks_for_the_keychain_or_the_keyring() {
    let mac = args(Os::Mac, Path::new("/p"), 9222);
    assert!(mac.contains(&"--use-mock-keychain".into()));
    let linux = args(Os::Linux, Path::new("/p"), 9222);
    assert!(linux.contains(&"--password-store=basic".into()));
    assert!(!linux.contains(&"--use-mock-keychain".into()));
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
fn a_snap_run_after_its_check_on_one_line_still_counts() {
    let real = Path::new("/usr/local/bin/chromium");
    let one_line = b"#!/bin/sh\n[ -x /snap/bin/chromium ] && exec /snap/bin/chromium \"$@\"\nexec /usr/bin/false\n";
    assert_eq!(snap_command(real, real, one_line).as_deref(), Some("chromium"));
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

fn errors_kept(said: &[u8]) -> Option<String> {
    let kept = Mutex::new(Said::default());
    keep_errors(std::io::Cursor::new(said), &kept);
    assert!(kept.lock().ended);
    kept.into_inner().error
}

#[test]
fn a_browser_that_stops_as_it_starts_gives_its_last_error() {
    let said = b"\n[5887:5887:1104/112642.487358:ERROR:zygote_host_impl_linux.cc:130] No usable sandbox! Install it.\n\
                [5887:5900:1104/112642.5:WARNING:bus.cc:408] Failed to connect to the bus\nDevTools listening on ws://x\n";
    assert_eq!(errors_kept(said).as_deref(), Some("No usable sandbox! Install it."));
    assert_eq!(errors_kept(b"DevTools listening on ws://127.0.0.1:9222/devtools/browser/x\n"), None);
}

#[test]
fn only_the_last_error_is_kept_however_long_the_browser_talks() {
    let mut said = Vec::new();
    for n in 0..2000 {
        said.extend(format!("[1:1:1104/1.0:ERROR:x.cc:1] error {n}\n[1:1:1104/1.0:INFO:x.cc:1] chatter\n").bytes());
    }
    said.extend(std::iter::repeat_n(b'x', 100_000));
    said.extend(b"\n[1:1:1104/1.0:FATAL:y.cc:2] the end\n");
    assert_eq!(errors_kept(&said).as_deref(), Some("the end"));
}

#[test]
fn a_snap_keeps_its_profile_where_its_confinement_lets_it_write() {
    let profile = snap_profile(Path::new("/home/me"), "chromium");
    assert_eq!(profile, Path::new("/home/me/snap/chromium/common/cornercase-gpui-browser"));
}

#[test]
fn the_browser_and_its_profile_are_chosen_once_one_is_found() {
    use std::cell::Cell;

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

#[test]
fn on_a_mac_chrome_is_looked_for_in_both_applications_folders() {
    let found = candidates(Os::Mac, Path::new("/Users/me"), &|_| None);
    assert!(found.contains(&PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")));
    assert!(found.contains(&PathBuf::from("/Users/me/Applications/Chromium.app/Contents/MacOS/Chromium")));
}

fn windows_folders(name: &str) -> Option<OsString> {
    match name {
        "ProgramFiles" => Some(r"C:\Program Files".into()),
        "ProgramFiles(x86)" => Some(r"C:\Program Files (x86)".into()),
        "LOCALAPPDATA" => Some(r"C:\Users\me\AppData\Local".into()),
        _ => None,
    }
}

fn windows_candidates() -> Vec<String> {
    let found = candidates(Os::Windows, Path::new(r"C:\Users\me"), &windows_folders);
    found.iter().map(|path| path.to_string_lossy().into_owned()).collect()
}

#[test]
fn on_windows_each_browser_is_looked_for_per_machine_then_per_user() {
    let found = windows_candidates();
    assert_eq!(
        found[..3],
        [
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            r"C:\Users\me\AppData\Local\Google\Chrome\Application\chrome.exe",
        ]
    );
    assert!(found.contains(&r"C:\Users\me\AppData\Local\Google\Chrome SxS\Application\chrome.exe".to_string()));
    assert!(found.contains(&r"C:\Users\me\AppData\Local\Chromium\Application\chrome.exe".to_string()));
    assert!(found.contains(&r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe".to_string()));
}

#[test]
fn on_windows_edge_comes_after_every_browser_installed_on_purpose() {
    let found = windows_candidates();
    let first_edge = found.iter().position(|path| path.ends_with(r"\msedge.exe")).expect("edge is looked for");
    assert!(found[first_edge..].iter().all(|path| path.ends_with(r"Microsoft\Edge\Application\msedge.exe")));
    assert!(found.contains(&r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe".to_string()));
}

#[test]
fn on_windows_a_folder_that_is_not_set_is_skipped_and_one_ending_in_a_separator_is_not_doubled() {
    let only_root = |name: &str| (name == "ProgramFiles").then(|| OsString::from(r"D:\"));
    let found = candidates(Os::Windows, Path::new(r"C:\Users\me"), &only_root);
    assert_eq!(found.first().map(PathBuf::as_path), Some(Path::new(r"D:\Google\Chrome\Application\chrome.exe")));
    assert_eq!(found.len(), WINDOWS_APPS.len());
}

#[test]
fn on_linux_chrome_is_only_looked_for_on_the_path() {
    assert_eq!(candidates(Os::Linux, Path::new("/home/me"), &windows_folders), Vec::<PathBuf>::new());
}

#[test]
fn on_windows_the_path_is_searched_for_exe_names() {
    assert!(path_names(Os::Windows).iter().all(|name| Path::new(name).extension().is_some_and(|e| e == "exe")));
    assert!(path_names(Os::Windows).contains(&"msedge.exe"));
    assert!(path_names(Os::Linux).contains(&"google-chrome-stable"));
}

#[test]
fn on_windows_chrome_logs_its_errors_to_the_pipe_and_asks_for_no_keyring() {
    let windows = args(Os::Windows, Path::new(r"C:\p"), 9222);
    assert!(windows.contains(&"--enable-logging=stderr".into()));
    assert!(windows.contains(&"--log-level=2".into()));
    assert!(!windows.iter().any(|arg| arg == "--use-mock-keychain" || arg == "--password-store=basic"));
    assert_eq!(windows.last().and_then(|arg| arg.to_str()), Some("--no-startup-window"));
    assert!(!args(Os::Linux, Path::new("/p"), 9222).contains(&"--enable-logging=stderr".into()));
}

#[test]
fn on_windows_a_profile_with_spaces_and_accents_is_one_argument() {
    let profile = Path::new(r"C:\Users\José Núñez\AppData\Local\cornercase-gpui\browser");
    let args = args(Os::Windows, profile, 9222);
    let dirs: Vec<&OsString> = args.iter().filter(|arg| arg.to_string_lossy().contains("José")).collect();
    assert_eq!(dirs, [&OsString::from(r"--user-data-dir=C:\Users\José Núñez\AppData\Local\cornercase-gpui\browser")]);
}

#[cfg(windows)]
#[test]
fn a_profile_whose_path_is_not_unicode_reaches_chrome_unit_for_unit() {
    use std::os::windows::ffi::{OsStrExt as _, OsStringExt as _};

    let wide: Vec<u16> = r"C:\caf".encode_utf16().chain([0xd800]).collect();
    let profile = PathBuf::from(OsString::from_wide(&wide));
    let args = args(Os::Windows, &profile, 9222);
    let flag: Vec<u16> = "--user-data-dir=".encode_utf16().collect();
    let dir = args.iter().map(|arg| arg.encode_wide().collect::<Vec<u16>>()).find(|arg| arg.starts_with(&flag));
    assert_eq!(dir.map(|arg| arg[flag.len()..].to_vec()), Some(wide));
}

fn skip_head(reader: &mut impl BufRead) {
    let mut line = String::new();
    while reader.read_line(&mut line).is_ok_and(|read| read > 0) && !line.trim().is_empty() {
        line.clear();
    }
}

fn fake_devtools() -> (u16, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("a port");
    let port = listener.local_addr().expect("its address").port();
    let served = std::thread::spawn(move || {
        let mut asked = BufReader::new(listener.accept().expect("asked for its version").0);
        skip_head(&mut asked);
        let body = format!(r#"{{"webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/browser/b"}}"#);
        let answer = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}", body.len());
        asked.get_mut().write_all(answer.as_bytes()).expect("answers");
        let mut socket = BufReader::new(listener.accept().expect("opened its socket").0);
        skip_head(&mut socket);
        socket.get_mut().write_all(b"HTTP/1.1 101 Switching Protocols\r\n\r\n").expect("upgrades");
        let Ok(Message::Text(request)) = socket::read_message(&mut socket) else { return String::new() };
        socket.get_mut().write_all(&socket::encode(1, br#"{"id":1,"result":{}}"#, None)).expect("replies");
        request
    });
    (port, served)
}

fn scratch_profile(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cornercase-gpui-{name}-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("a profile");
    dir
}

#[test]
fn on_windows_the_browser_is_asked_to_close_through_its_devtools_socket() {
    let (port, served) = fake_devtools();
    assert_eq!(close_through_devtools(port), Some(()));
    let request: serde_json::Value = serde_json::from_str(&served.join().expect("served")).expect("json");
    assert_eq!(request["method"], "Browser.close");
}

#[test]
fn a_browser_that_does_not_answer_is_not_asked_to_close() {
    let port = TcpListener::bind(("127.0.0.1", 0)).expect("a port").local_addr().expect("its address").port();
    assert_eq!(close_through_devtools(port), None);
}

#[test]
fn on_windows_stopping_asks_the_profile_s_browser_on_its_port_rather_than_signalling_a_pid() {
    let (port, served) = fake_devtools();
    let profile = scratch_profile("stop");
    fs::write(profile.join(PORT_FILE), port.to_string()).expect("the port file");
    assert!(ask_to_stop(Os::Windows, 0, &profile));
    assert!(served.join().expect("served").contains("Browser.close"));
    fs::remove_file(profile.join(PORT_FILE)).expect("the port file goes");
    assert!(!ask_to_stop(Os::Windows, 0, &profile));
    let _ = fs::remove_dir_all(&profile);
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
