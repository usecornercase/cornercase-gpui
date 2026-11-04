use std::path::Path;
use std::sync::OnceLock;

use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_FILENAME, SND_NODEFAULT};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RegSetKeyValueW};
use windows::core::{HSTRING, w};

static REGISTERED: OnceLock<Result<(), String>> = OnceLock::new();

pub(super) fn play(file: &Path) -> Result<(), String> {
    let started = unsafe { PlaySoundW(&HSTRING::from(file), None, SND_FILENAME | SND_ASYNC | SND_NODEFAULT) };
    if started.as_bool() { Ok(()) } else { Err(format!("PlaySound could not play {}", file.display())) }
}

pub(super) fn toast((id, key, name): (&str, &str, &str), xml: &str) -> Result<(), String> {
    REGISTERED.get_or_init(|| register(key, name)).clone()?;
    show(id, xml).map_err(|e| e.message())
}

fn register(key: &str, name: &str) -> Result<(), String> {
    let value = HSTRING::from(name);
    let bytes = u32::try_from((value.len() + 1) * size_of::<u16>()).map_err(|e| e.to_string())?;
    let data = Some(value.as_ptr().cast());
    let written =
        unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, &HSTRING::from(key), w!("DisplayName"), REG_SZ.0, data, bytes) };
    if written.is_ok() {
        Ok(())
    } else {
        Err(format!("could not name cornercase for Windows: {}", written.to_hresult().message()))
    }
}

fn show(id: &str, xml: &str) -> windows::core::Result<()> {
    let content = XmlDocument::new()?;
    content.LoadXml(&HSTRING::from(xml))?;
    let toast = ToastNotification::CreateToastNotification(&content)?;
    ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(id))?.Show(&toast)
}
