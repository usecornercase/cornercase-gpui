use std::ffi::OsStr;

use super::{Ended, args};
use crate::cli::{Remote, Target};

#[test]
fn a_hold_runs_tunnel_hold_with_the_windows_remote_flags() {
    let target = Target::Remote(Remote::new("devbox", "~/bin/cornercase").expect("a destination"));
    let command = target.command(&args(5173));
    let all: Vec<&OsStr> = command.get_args().collect();
    assert_eq!(all, ["--remote", "devbox", "--remote-command", "~/bin/cornercase", "tunnel", "hold", "5173"]);
    assert!(command.get_envs().any(|(name, value)| name == "CORNERCASE_REMOTE" && value.is_none()));
}

#[cfg(unix)]
mod on_unix {
    use std::process::Stdio;
    use std::sync::mpsc;
    use std::time::Duration;

    use super::Ended;
    use crate::cli::kept::{End, Kept};

    const WAIT: Duration = Duration::from_secs(20);

    fn start(kept: &'static Kept, script: &str) -> (u64, mpsc::Receiver<String>) {
        let mut shell = crate::platform::command("/bin/sh");
        shell.args(["-c", script]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let (tx, rx) = mpsc::channel();
        let ended = tx.clone();
        let on_line = move |line: String| {
            let _ = tx.send(line);
        };
        let on_end = move |why: String, code: Option<i32>| {
            let _ = ended.send(format!("{:?}", Ended { code, said: why.trim().to_string() }));
        };
        let (id, _) = kept.start(shell, on_line, on_end).expect("the shell starts");
        (id, rx)
    }

    #[test]
    fn ending_a_hold_asks_it_to_stop_what_it_runs() {
        static HOLDS: Kept = Kept::empty(End::TakeAlong);
        let script = "trap 'echo asked to end; kill $!; exit 0' TERM; sleep 60 >/dev/null 2>&1 & echo ready; wait $!";
        let (id, heard) = start(&HOLDS, script);
        assert_eq!(heard.recv_timeout(WAIT).expect("it is ready"), "ready\n");
        HOLDS.stop_one(id);
        assert_eq!(heard.recv_timeout(WAIT).expect("it was asked"), "asked to end\n");
        assert!(heard.recv_timeout(WAIT).expect("it ended").starts_with("Ended { code: Some(0)"));
    }

    #[test]
    fn a_hold_that_ends_by_itself_gives_its_status_and_stderr() {
        static HOLDS: Kept = Kept::empty(End::TakeAlong);
        let (_, heard) = start(&HOLDS, "echo could not tunnel port 1 >&2; exit 1");
        let ended = heard.recv_timeout(WAIT).expect("it ended");
        assert_eq!(ended, format!("{:?}", Ended { code: Some(1), said: "could not tunnel port 1".to_string() }));
    }
}
