use std::path::Path;

pub(super) fn play(file: &Path) -> Result<(), String> {
    Err(format!("PlaySound runs only on Windows, so {} cannot play", file.display()))
}

pub(super) fn toast(_app: (&str, &str, &str), _xml: &str) -> Result<(), String> {
    Err("Windows notifications run only on Windows".to_string())
}
