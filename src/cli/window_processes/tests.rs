use std::ffi::OsStr;

use super::command;

#[test]
fn the_command_runs_here_whatever_server_the_window_is_on() {
    let command = command();
    let args: Vec<&OsStr> = command.get_args().collect();
    assert_eq!(args, ["plugin", "windows", "--json"]);
    assert!(command.get_envs().any(|(name, value)| name == "CORNERCASE_REMOTE" && value.is_none()));
}

#[cfg(unix)]
mod on_unix {
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::Duration;

    use super::super::Kept;

    const WAIT: Duration = Duration::from_secs(20);
    const CATCHES_THE_END: &str = "trap 'echo asked to end; kill $!; exit 0' TERM; \
                                   sleep 60 >/dev/null 2>&1 & echo ready; wait $!";

    fn shell(script: &str) -> Command {
        let mut shell = crate::platform::command("/bin/sh");
        shell.args(["-c", script]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        shell
    }

    fn start(kept: &'static Kept, script: &str) -> mpsc::Receiver<String> {
        let (tx, rx) = mpsc::channel();
        let ended = tx.clone();
        let on_line = move |line: String| {
            let _ = tx.send(line);
        };
        let on_end = move |why: String| {
            let _ = ended.send(format!("ended: {why}"));
        };
        kept.start(shell(script), on_line, on_end).expect("the shell starts");
        rx
    }

    fn next(heard: &mpsc::Receiver<String>) -> String {
        heard.recv_timeout(WAIT).expect("the shell says something")
    }

    #[test]
    fn stopping_gives_the_process_the_chance_to_stop_what_it_runs() {
        static KEPT: Kept = Kept::empty();
        let heard = start(&KEPT, CATCHES_THE_END);
        assert_eq!(next(&heard), "ready\n");
        KEPT.stop();
        assert_eq!(next(&heard), "asked to end\n");
        assert_eq!(next(&heard), "ended: ");
        assert!(KEPT.children.lock().is_empty());
    }

    #[test]
    fn one_that_ends_leaves_the_next_one_there_to_be_stopped() {
        static KEPT: Kept = Kept::empty();
        let newer = start(&KEPT, CATCHES_THE_END);
        assert_eq!(next(&newer), "ready\n");
        let older = start(&KEPT, "exit 0");
        assert_eq!(next(&older), "ended: ");
        KEPT.stop();
        assert_eq!(next(&newer), "asked to end\n");
        assert_eq!(next(&newer), "ended: ");
    }

    #[test]
    fn a_process_that_ended_by_itself_says_why_and_leaves_nothing_to_stop() {
        static KEPT: Kept = Kept::empty();
        let heard = start(&KEPT, "echo no such thing >&2; exit 3");
        assert_eq!(next(&heard), "ended: no such thing\n");
        assert!(KEPT.children.lock().is_empty());
    }
}

#[cfg(windows)]
mod on_windows {
    use std::process::Stdio;
    use std::sync::mpsc;
    use std::time::Duration;

    use super::super::Kept;

    #[test]
    fn a_command_kept_on_windows_is_tied_to_the_window() {
        static KEPT: Kept = Kept::empty();
        let (tx, ended) = mpsc::channel();
        let mut ping = crate::platform::command("ping");
        ping.args(["-n", "120", "127.0.0.1"]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let untied = KEPT
            .start(
                ping,
                |_| {},
                move |_| {
                    let _ = tx.send(());
                },
            )
            .expect("ping starts");
        let held = KEPT.children.lock().iter().all(|running| running.job.is_some());
        KEPT.stop();
        ended.recv_timeout(Duration::from_secs(20)).expect("the command ends");
        assert_eq!((untied, held), (None, true));
        assert!(KEPT.children.lock().is_empty());
    }
}
