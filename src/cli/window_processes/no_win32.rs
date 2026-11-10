use std::process::Child;

pub(super) enum Job {}

pub(super) fn tie(_child: &Child) -> Result<Job, String> {
    Err("Job objects exist only on Windows".to_string())
}

pub(super) fn let_go(job: &Job) {
    match *job {}
}
