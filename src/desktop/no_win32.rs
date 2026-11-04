use std::ffi::OsStr;
use std::path::PathBuf;

pub(super) fn shell_open(_verb: Option<&str>, _file: &OsStr, _parameters: Option<&OsStr>) -> Result<(), String> {
    Err("the Windows shell runs only on Windows".to_string())
}

pub(crate) fn windows_folder() -> Option<PathBuf> {
    None
}

pub(super) fn dangerous(_extension: &OsStr) -> bool {
    false
}

pub(super) fn set_app_id(_id: &str) {}
