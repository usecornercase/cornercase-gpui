use std::os::windows::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, STILL_ACTIVE};
use windows::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowExW, GetWindowThreadProcessId, HWND_MESSAGE};
use windows::core::{HRESULT, PCWSTR, w};

pub(crate) fn alive(pid: u32) -> bool {
    match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(process) => {
            let mut code = 0;
            let read = unsafe { GetExitCodeProcess(process, &raw mut code) }.is_ok();
            let _ = unsafe { CloseHandle(process) };
            read && code == STILL_ACTIVE.0.cast_unsigned()
        }
        Err(error) => error.code() == HRESULT::from_win32(ERROR_ACCESS_DENIED.0),
    }
}

fn singleton_name(profile: &Path) -> PathBuf {
    std::path::absolute(profile).unwrap_or_else(|_| profile.to_path_buf()).components().collect()
}

pub(crate) fn singleton_pid(profile: &Path) -> Option<u32> {
    let name: Vec<u16> = singleton_name(profile).as_os_str().encode_wide().chain([0]).collect();
    let class = w!("Chrome_MessageWindow");
    let window = unsafe { FindWindowExW(Some(HWND_MESSAGE), None, class, PCWSTR(name.as_ptr())) }.ok()?;
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(window, Some(&raw mut pid)) };
    (pid != 0).then_some(pid)
}

#[cfg(test)]
mod tests;
