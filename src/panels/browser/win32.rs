use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, ERROR_INSUFFICIENT_BUFFER, NO_ERROR, STILL_ACTIVE};
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
};
use windows::Win32::Networking::WinSock::AF_INET;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::Shell::{FOLDERID_Downloads, KF_FLAG_DONT_VERIFY, SHGetKnownFolderPath};
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

pub(crate) fn listening_pid(port: u16) -> Option<u32> {
    let table = tcp_listeners()?;
    let head = table.as_ptr().cast::<MIB_TCPTABLE_OWNER_PID>();
    let count = usize::try_from(unsafe { (*head).dwNumEntries }).ok()?;
    let rows = unsafe { std::slice::from_raw_parts((&raw const (*head).table).cast::<MIB_TCPROW_OWNER_PID>(), count) };
    let on = |address: [u8; 4]| {
        rows.iter().find(|row| {
            row.dwLocalAddr.to_ne_bytes() == address && row.dwLocalPort.to_ne_bytes()[..2] == port.to_be_bytes()
        })
    };
    on([127, 0, 0, 1]).or_else(|| on([0, 0, 0, 0])).map(|row| row.dwOwningPid)
}

fn tcp_listeners() -> Option<Vec<u32>> {
    let mut size = 0u32;
    let family = u32::from(AF_INET.0);
    for _ in 0..4 {
        let mut table = vec![0u32; usize::try_from(size).ok()?.div_ceil(4).max(1)];
        let asked = unsafe {
            GetExtendedTcpTable(
                Some(table.as_mut_ptr().cast()),
                &raw mut size,
                false,
                family,
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        match asked {
            code if code == NO_ERROR.0 => return Some(table),
            code if code == ERROR_INSUFFICIENT_BUFFER.0 => {}
            _ => return None,
        }
    }
    None
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

pub(crate) fn downloads_folder() -> Option<PathBuf> {
    let found = unsafe { SHGetKnownFolderPath(&FOLDERID_Downloads, KF_FLAG_DONT_VERIFY, None) }.ok()?;
    let path = PathBuf::from(OsString::from_wide(unsafe { found.as_wide() }));
    unsafe { CoTaskMemFree(Some(found.0.cast_const().cast())) };
    Some(path).filter(|path| path.is_absolute())
}

#[cfg(test)]
mod tests;
