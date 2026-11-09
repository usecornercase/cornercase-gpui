use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::prefs;
use crate::theme::presets::is_preset;

const CHOICE: &str = "theme.json";
const FOLDER: &str = "themes";

pub(crate) fn folder_of(settings: &Path) -> PathBuf {
    settings.parent().map(Path::to_path_buf).unwrap_or_default()
}

pub(crate) fn folder() -> PathBuf {
    folder_of(&prefs::path())
}

pub(crate) fn choice_path(folder: &Path) -> PathBuf {
    folder.join(CHOICE)
}

pub(crate) fn themes_dir(folder: &Path) -> PathBuf {
    folder.join(FOLDER)
}

pub(crate) fn theme_path(folder: &Path, id: &str) -> PathBuf {
    themes_dir(folder).join(format!("{id}.json"))
}

pub(crate) fn id_of(path: &Path) -> Option<String> {
    let json = path.extension().is_some_and(|ext| ext == "json");
    json.then(|| path.file_stem().map(|stem| stem.to_string_lossy().into_owned())).flatten()
}

pub(crate) fn read_themes(folder: &Path) -> BTreeMap<String, String> {
    let Ok(entries) = std::fs::read_dir(themes_dir(folder)) else { return BTreeMap::new() };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            Some((id_of(&path)?, std::fs::read_to_string(&path).ok()?))
        })
        .collect()
}

pub(crate) fn slug(name: &str) -> String {
    let words: Vec<String> =
        name.split(|c: char| !c.is_alphanumeric()).filter(|word| !word.is_empty()).map(str::to_lowercase).collect();
    if words.is_empty() { "theme".to_string() } else { words.join("-") }
}

pub(crate) fn free_stem(wanted: &str, taken: &dyn Fn(&str) -> bool) -> String {
    let base = slug(wanted);
    let blocked = |stem: &str| taken(stem) || is_preset(stem);
    if !blocked(&base) {
        return base;
    }
    (2..10_000).map(|n| format!("{base}-{n}")).find(|stem| !blocked(stem)).unwrap_or(base)
}

#[cfg(test)]
mod tests;
