use gpui::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::{HWND, TRUE};
use windows::Win32::Graphics::Dwm::{
    DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWINDOWATTRIBUTE,
    DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
};
use windows::Win32::UI::Controls::MARGINS;

pub(super) fn backdrop(window: &Window) -> Result<(), String> {
    let handle = HasWindowHandle::window_handle(window).map_err(|e| e.to_string())?;
    let RawWindowHandle::Win32(win32) = handle.as_raw() else { return Err("not a Win32 window".to_string()) };
    let hwnd = HWND(win32.hwnd.get() as *mut _);
    let whole = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
    set(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &TRUE)?;
    unsafe { DwmExtendFrameIntoClientArea(hwnd, &raw const whole) }.map_err(|e| e.message())?;
    set(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &DWMSBT_TRANSIENTWINDOW)
}

fn set<T>(hwnd: HWND, attribute: DWMWINDOWATTRIBUTE, value: &T) -> Result<(), String> {
    let size = u32::try_from(size_of::<T>()).map_err(|e| e.to_string())?;
    unsafe { DwmSetWindowAttribute(hwnd, attribute, std::ptr::from_ref(value).cast(), size) }.map_err(|e| e.message())
}
