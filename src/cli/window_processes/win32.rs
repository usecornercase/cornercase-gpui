use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle};
use std::process::Child;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT, JOB_OBJECT_LIMIT_BREAKAWAY_OK,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_BASIC_LIMIT_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectExtendedLimitInformation, SetInformationJobObject,
};
use windows::core::PCWSTR;

pub(super) struct Job(OwnedHandle);

impl Job {
    fn handle(&self) -> HANDLE {
        HANDLE(self.0.as_raw_handle())
    }
}

pub(super) fn tie(child: &Child) -> Result<Job, String> {
    let made = unsafe { CreateJobObjectW(None, PCWSTR::null()) }
        .map_err(|e| format!("Windows could not make a job object: {}", e.message()))?;
    let job = Job(unsafe { OwnedHandle::from_raw_handle(made.0) });
    limit(&job, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_BREAKAWAY_OK)
        .map_err(|e| format!("Windows could not set the job object's limits: {e}"))?;
    unsafe { AssignProcessToJobObject(job.handle(), HANDLE(child.as_raw_handle())) }
        .map_err(|e| format!("Windows could not put the command in its job object: {}", e.message()))?;
    Ok(job)
}

pub(super) fn let_go(job: &Job) {
    let _ = limit(job, JOB_OBJECT_LIMIT_BREAKAWAY_OK);
}

fn limit(job: &Job, flags: JOB_OBJECT_LIMIT) -> Result<(), String> {
    let info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
        BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION { LimitFlags: flags, ..Default::default() },
        ..Default::default()
    };
    let size = u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()).map_err(|e| e.to_string())?;
    unsafe {
        SetInformationJobObject(job.handle(), JobObjectExtendedLimitInformation, std::ptr::from_ref(&info).cast(), size)
    }
    .map_err(|e| e.message())
}

#[cfg(test)]
pub(super) fn copy(job: &Job) -> Job {
    Job(job.0.try_clone().expect("the handle is duplicated"))
}

#[cfg(test)]
pub(super) fn limits(job: &Job) -> JOB_OBJECT_LIMIT {
    use windows::Win32::System::JobObjects::QueryInformationJobObject;

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

#[cfg(test)]
mod tests;
