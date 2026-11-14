use std::ffi::OsStr;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use super::{Ended, Holding, end_hold, hold_with};
use crate::cli::{Remote, Target};

const WAIT: Duration = Duration::from_secs(20);

#[test]
fn a_hold_runs_tunnel_hold_with_the_windows_remote_flags() {
    let target = Target::Remote(Remote::new("devbox", "~/bin/cornercase").expect("a destination"));
    let command = target.command(&Holding::Forward { port: 5173, local: None }.args());
    let all: Vec<&OsStr> = command.get_args().collect();
    assert_eq!(all, ["--remote", "devbox", "--remote-command", "~/bin/cornercase", "tunnel", "hold", "5173"]);
    assert!(command.get_envs().any(|(name, value)| name == "CORNERCASE_REMOTE" && value.is_none()));
}

#[test]
fn a_hold_started_again_asks_for_its_local_port() {
    let target = Target::Remote(Remote::new("devbox", "~/bin/cornercase").expect("a destination"));
    let command = target.command(&Holding::Forward { port: 5173, local: Some(54012) }.args());
    let all: Vec<&OsStr> = command.get_args().collect();
    assert_eq!(all[4..], ["tunnel", "hold", "5173", "--local", "54012"].map(OsStr::new));
}

#[test]
fn a_reverse_hold_names_the_local_port_and_no_port_there() {
    let target = Target::Remote(Remote::new("devbox", "~/bin/cornercase").expect("a destination"));
    let command = target.command(&Holding::Reverse { local: 9222 }.args());
    let all: Vec<&OsStr> = command.get_args().collect();
    assert_eq!(all[4..], ["tunnel", "hold", "--reverse", "9222"].map(OsStr::new));
    assert!(all.iter().all(|arg| *arg != "--there"));
}

enum Heard {
    Line(String),
    End(Ended),
}

fn start(mut command: Command) -> (u64, mpsc::Receiver<Heard>) {
    command.stdin(Stdio::null());
    let (tx, rx) = mpsc::channel();
    let ended = tx.clone();
    let on_line = move |line: String| {
        let _ = tx.send(Heard::Line(line));
    };
    let on_end = move |end: Ended| {
        let _ = ended.send(Heard::End(end));
    };
    let (id, _) = hold_with(command, on_line, on_end).expect("the hold starts");
    (id, rx)
}

#[cfg(unix)]
mod on_unix {
    use super::*;

    fn next(heard: &mpsc::Receiver<Heard>) -> Heard {
        heard.recv_timeout(WAIT).expect("the hold says something")
    }

    fn shell(script: &str) -> Command {
        let mut shell = crate::platform::command("/bin/sh");
        shell.args(["-c", script]);
        shell
    }

    #[test]
    fn ending_a_hold_asks_it_to_stop_what_it_runs() {
        let script = "trap 'echo asked to end; kill $!; exit 0' TERM; sleep 60 >/dev/null 2>&1 & echo ready; wait $!";
        let (id, heard) = start(shell(script));
        assert!(matches!(next(&heard), Heard::Line(line) if line == "ready\n"));
        end_hold(id);
        assert!(matches!(next(&heard), Heard::Line(line) if line == "asked to end\n"));
        assert!(matches!(next(&heard), Heard::End(Ended { code: Some(0), .. })));
    }

    #[test]
    fn a_hold_that_ends_by_itself_gives_its_status_and_stderr() {
        let (_, heard) = start(shell("echo could not tunnel port 1 >&2; exit 1"));
        let Heard::End(ended) = next(&heard) else { panic!("it ends") };
        assert_eq!(
            ended,
            Ended { code: Some(1), said: "could not tunnel port 1".to_string(), last_line: String::new() }
        );
    }

    fn ended_of(script: &str) -> Ended {
        let (_, heard) = start(shell(script));
        loop {
            if let Heard::End(ended) = next(&heard) {
                return ended;
            }
        }
    }

    #[test]
    fn a_hold_gives_the_last_line_it_printed_with_its_end() {
        let ended = ended_of(
            r#"echo '{"port":8765,"local":61014,"url":"http://localhost:61014"}'; echo '{"ended":"server-stopped"}'; exit 7"#,
        );
        assert_eq!(
            ended,
            Ended { code: Some(7), said: String::new(), last_line: r#"{"ended":"server-stopped"}"#.to_string() }
        );
    }

    #[test]
    fn a_partial_last_line_comes_as_it_was_printed() {
        let ended = ended_of(r#"echo first; printf '{"ended":"clo'"#);
        assert_eq!(ended.last_line, r#"{"ended":"clo"#);
    }

    #[test]
    fn an_empty_line_after_the_end_does_not_hide_it() {
        let ended = ended_of(r#"echo '{"ended":"closed"}'; echo; echo '   '"#);
        assert_eq!(ended.last_line, r#"{"ended":"closed"}"#);
    }

    #[test]
    fn a_hold_that_printed_nothing_has_no_last_line() {
        assert_eq!(ended_of("exit 0").last_line, "");
    }
}

#[cfg(windows)]
mod on_windows {
    use std::time::Instant;

    use super::*;

    #[test]
    fn ending_a_hold_takes_what_it_started_along() {
        let mut tree = crate::platform::command("cmd");
        tree.args(["/d", "/c", "ping", "-n", "120", "127.0.0.1"]);
        let (id, heard) = start(tree);
        let until = Instant::now() + WAIT;
        let mut said = String::new();
        while !said.contains("127.0.0.1") {
            match heard.recv_timeout(until.saturating_duration_since(Instant::now())) {
                Ok(Heard::Line(line)) => said.push_str(&line),
                Ok(Heard::End(_)) => panic!("the hold ended before ping said anything: {said:?}"),
                Err(why) => panic!("ping did not start ({why}): {said:?}"),
            }
        }
        end_hold(id);
        let until = Instant::now() + WAIT;
        loop {
            match heard.recv_timeout(until.saturating_duration_since(Instant::now())) {
                Ok(Heard::Line(line)) => said.push_str(&line),
                Ok(Heard::End(ended)) => {
                    assert_eq!(ended.said, ended.said.trim(), "the end's stderr is trimmed");
                    break;
                }
                Err(why) => panic!("the ping the hold's cmd started outlived the hold ({why}); it said {said:?}"),
            }
        }
    }
}
