use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::OsStringExt as _;
use std::path::PathBuf;

use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize};
use windows::Win32::System::SystemInformation::GetSystemWindowsDirectoryW;
use windows::Win32::UI::Shell::{
    AssocIsDangerous, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR};

pub(super) fn shell_open(verb: Option<&str>, file: &OsStr, parameters: Option<&OsStr>) -> Result<(), String> {
    let (verb, file, parameters) = (verb.map(HSTRING::from), HSTRING::from(file), parameters.map(HSTRING::from));
    std::thread::spawn(move || execute(verb.as_ref(), &file, parameters.as_ref()))
        .join()
        .unwrap_or_else(|_| Err("the shell's thread stopped".to_string()))
}

fn execute(verb: Option<&HSTRING>, file: &HSTRING, parameters: Option<&HSTRING>) -> Result<(), String> {
    let mut info = SHELLEXECUTEINFOW {
        cbSize: u32::try_from(size_of::<SHELLEXECUTEINFOW>()).map_err(|e| e.to_string())?,
        fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpVerb: text(verb),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: text(parameters),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    let com = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
    let opened = unsafe { ShellExecuteExW(&raw mut info) };
    if com.is_ok() {
        unsafe { CoUninitialize() };
    }
    opened.map_err(|e| e.message())
}

fn text(value: Option<&HSTRING>) -> PCWSTR {
    value.map_or(PCWSTR::null(), |value| PCWSTR(value.as_ptr()))
}

pub(crate) fn windows_folder() -> Option<PathBuf> {
    let mut buffer = [0u16; 1024];
    let length = usize::try_from(unsafe { GetSystemWindowsDirectoryW(Some(&mut buffer)) }).ok()?;
    let name = buffer.get(..length).filter(|name| !name.is_empty())?;
    Some(PathBuf::from(OsString::from_wide(name)))
}

pub(super) fn dangerous(extension: &OsStr) -> bool {
    let mut dotted = OsString::from(".");
    dotted.push(extension);
    unsafe { AssocIsDangerous(&HSTRING::from(dotted.as_os_str())) }.as_bool()
}
