use std::io;
use std::process::{Child, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use windows::Win32::System::JobObjects::{
    JOB_OBJECT_LIMIT, JOB_OBJECT_LIMIT_BREAKAWAY_OK, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, QueryInformationJobObject,
};

use super::{Job, let_go, tie};

const WAIT: Duration = Duration::from_secs(20);

fn tree() -> (Child, mpsc::Receiver<()>) {
    let mut child = crate::platform::command("cmd")
        .args(["/d", "/c", "ping", "-n", "120", "127.0.0.1"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("cmd starts");
    let mut out = child.stdout.take().expect("stdout is piped");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = io::copy(&mut out, &mut io::sink());
        let _ = tx.send(());
    });
    (child, rx)
}

fn ping() -> Child {
    crate::platform::command("ping")
        .args(["-n", "120", "127.0.0.1"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("ping starts")
}

fn limits(job: &Job) -> JOB_OBJECT_LIMIT {
    let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    let size = u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()).expect("the size fits");
    unsafe {
        QueryInformationJobObject(
            Some(job.handle()),
            JobObjectExtendedLimitInformation,
            std::ptr::from_mut(&mut info).cast(),
            size,
            None,
        )
    }
    .expect("the limits are read");
    info.BasicLimitInformation.LimitFlags
}

#[test]
fn closed_without_letting_go_it_ends_the_command_and_what_it_started() {
    let (mut child, gone) = tree();
    let job = tie(&child).expect("the command joins its job");
    drop(job);
    assert!(gone.recv_timeout(WAIT).is_ok());
    child.wait().expect("the command is reaped");
}

#[test]
fn it_nests_in_a_job_the_command_is_already_in() {
    let (mut child, gone) = tree();
    let launcher = tie(&child).expect("the launcher's job");
    let job = tie(&child).expect("nested");
    drop(job);
    assert!(gone.recv_timeout(WAIT).is_ok());
    let_go(&launcher);
    child.wait().expect("the command is reaped");
}

#[test]
fn only_a_process_that_asks_leaves_it() {
    let mut child = ping();
    let job = tie(&child).expect("the command joins its job");
    assert_eq!(limits(&job), JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_BREAKAWAY_OK);
    drop(job);
    child.wait().expect("the job ended the command");
}

#[test]
fn let_go_it_no_longer_ends_what_it_holds() {
    let mut child = ping();
    let job = tie(&child).expect("the command joins its job");
    let_go(&job);
    assert_eq!(limits(&job), JOB_OBJECT_LIMIT_BREAKAWAY_OK);
    child.kill().expect("the command is killed");
    child.wait().expect("the command is reaped");
}
