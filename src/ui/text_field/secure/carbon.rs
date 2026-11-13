#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn EnableSecureEventInput() -> i32;
    fn DisableSecureEventInput() -> i32;
}

pub(super) fn enable() -> bool {
    unsafe { EnableSecureEventInput() == 0 }
}

pub(super) fn disable() {
    unsafe {
        DisableSecureEventInput();
    }
}
