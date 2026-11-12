use std::ffi::OsStr;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use super::{Ended, args, end_hold, hold_with};
use crate::cli::{Remote, Target};

const WAIT: Duration = Duration::from_secs(20);

#[test]
fn a_hold_runs_tunnel_hold_with_the_windows_remote_flags() {
    let target = Target::Remote(Remote::new("devbox", "~/bin/cornercase").expect("a destination"));
    let command = target.command(&args(5173));
    let all: Vec<&OsStr> = command.get_args().collect();
    assert_eq!(all, ["--remote", "devbox", "--remote-command", "~/bin/cornercase", "tunnel", "hold", "5173"]);
    assert!(command.get_envs().any(|(name, value)| name == "CORNERCASE_REMOTE" && value.is_none()));
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

fn next(heard: &mpsc::Receiver<Heard>) -> Heard {
    heard.recv_timeout(WAIT).expect("the hold says something")
}

#[cfg(unix)]
mod on_unix {
    use super::*;

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
        assert_eq!(ended, Ended { code: Some(1), said: "could not tunnel port 1".to_string() });
    }
}

#[cfg(windows)]
mod on_windows {
    use super::*;

    #[test]
    fn ending_a_hold_takes_what_it_started_along() {
        let mut tree = crate::platform::command("cmd");
        tree.args(["/d", "/c", "ping", "-n", "120", "127.0.0.1"]);
        let (id, heard) = start(tree);
        end_hold(id);
        let mut said = String::new();
        let ended = loop {
            match next(&heard) {
                Heard::Line(line) => said.push_str(&line),
                Heard::End(ended) => break ended,
            }
        };
        assert!(!said.contains("Error") && !ended.said.contains("Error"), "the hold said {said:?}");
    }
}
