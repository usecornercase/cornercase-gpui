use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const FILE: &str = "settings.json";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OpenLinks {
    BrowserPanel,
    #[default]
    #[serde(other)]
    DefaultBrowser,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PreviewPages {
    EveryAddress,
    WhenAsked,
    #[default]
    #[serde(other)]
    LocalAddresses,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Prefs {
    pub open_links: OpenLinks,
    pub preview_pages: PreviewPages,
}

impl Prefs {
    pub(crate) fn parse(text: &str) -> Prefs {
        serde_json::from_str(text).unwrap_or_default()
    }

    pub(crate) fn values(&self) -> serde_json::Map<String, Value> {
        match serde_json::to_value(self) {
            Ok(Value::Object(map)) => map,
            _ => serde_json::Map::new(),
        }
    }

    pub(crate) fn with(&self, key: &str, value: &str) -> Result<Prefs> {
        let mut values = self.values();
        let Some(slot) = values.get_mut(key) else { bail!("this app has no setting {key}") };
        *slot = Value::String(value.to_string());
        let changed: Prefs = serde_json::from_value(Value::Object(values))?;
        if changed.values().get(key) != Some(&Value::String(value.to_string())) {
            bail!("{value} is not a value of {key}");
        }
        Ok(changed)
    }

    pub(crate) fn opens_in_panel(&self, reachable_outside: bool) -> bool {
        self.open_links == OpenLinks::BrowserPanel || !reachable_outside
    }

    pub(crate) fn previews(&self, local: bool) -> bool {
        match self.preview_pages {
            PreviewPages::EveryAddress => true,
            PreviewPages::LocalAddresses => local,
            PreviewPages::WhenAsked => false,
        }
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default()
}

fn data_dir(home: &Path) -> PathBuf {
    let var = |name: &str| std::env::var_os(name).map(PathBuf::from).filter(|p| p.is_absolute());
    if cfg!(target_os = "macos") {
        return home.join("Library/Application Support");
    }
    if cfg!(windows) {
        return var("LOCALAPPDATA").unwrap_or_else(|| home.join("AppData\\Local"));
    }
    var("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share"))
}

pub(crate) fn app_dir() -> PathBuf {
    data_dir(&home()).join("cornercase-gpui")
}

pub(crate) fn path() -> PathBuf {
    std::env::var_os("CORNERCASE_GPUI_SETTINGS").map_or_else(|| app_dir().join(FILE), PathBuf::from)
}

pub(crate) fn load() -> Prefs {
    std::fs::read_to_string(path()).map(|text| Prefs::parse(&text)).unwrap_or_default()
}

pub(crate) fn save(prefs: &Prefs) -> Result<()> {
    let path = path();
    let folder = path.parent().context("the settings file has no folder")?;
    std::fs::create_dir_all(folder).with_context(|| format!("could not create {}", folder.display()))?;
    let written = path.with_extension("json.new");
    std::fs::write(&written, serde_json::to_string_pretty(prefs)? + "\n")
        .with_context(|| format!("could not write {}", written.display()))?;
    std::fs::rename(&written, &path).with_context(|| format!("could not save {}", path.display()))
}

#[cfg(test)]
mod tests;
