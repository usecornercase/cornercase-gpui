use std::sync::OnceLock;

use gpui::{SharedString, TextSystem, font};

use crate::platform::Os;

const MAC_MONO: &str = "Menlo";
const SYSTEM_SANS: &str = ".SystemUIFont";
const LINUX_MONO: [&str; 9] = [
    "DejaVu Sans Mono",
    "Adwaita Mono",
    "Noto Sans Mono",
    "Ubuntu Mono",
    "Ubuntu Sans Mono",
    "Liberation Mono",
    "JetBrains Mono",
    "Source Code Pro",
    "Noto Mono",
];
const LINUX_SANS: [&str; 8] =
    ["Ubuntu Sans", "Ubuntu", "Adwaita Sans", "Cantarell", "Noto Sans", "Inter", "DejaVu Sans", "Liberation Sans"];

static CHOSEN: OnceLock<Fonts> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fonts {
    pub(crate) mono: String,
    pub(crate) sans: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    Mono,
    Sans,
}

pub(crate) const FONT_FAMILY: Family = Family::Mono;
pub(crate) const UI_FAMILY: Family = Family::Sans;

impl Family {
    pub(crate) fn name(self) -> &'static str {
        let fonts = CHOSEN.get_or_init(|| choose(Os::CURRENT, &[], |_| false));
        match self {
            Family::Mono => &fonts.mono,
            Family::Sans => &fonts.sans,
        }
    }
}

impl From<Family> for SharedString {
    fn from(family: Family) -> Self {
        SharedString::new_static(family.name())
    }
}

pub(crate) fn choose(os: Os, families: &[String], installed: impl Fn(&str) -> bool) -> Fonts {
    if os == Os::Mac {
        return Fonts { mono: MAC_MONO.into(), sans: SYSTEM_SANS.into() };
    }
    let mono = LINUX_MONO
        .into_iter()
        .find(|name| installed(name))
        .or_else(|| families.iter().map(String::as_str).find(|name| is_mono(name) && installed(name)))
        .unwrap_or(LINUX_MONO[0]);
    let sans = LINUX_SANS.into_iter().find(|name| installed(name)).unwrap_or(SYSTEM_SANS);
    Fonts { mono: mono.into(), sans: sans.into() }
}

fn is_mono(family: &str) -> bool {
    family.split_whitespace().any(|word| word == "Mono")
}

pub(crate) fn settle(text: &TextSystem) {
    if Os::CURRENT == Os::Mac {
        return;
    }
    let families = text.all_font_names();
    let installed =
        |name: &str| text.get_font_for_id(text.resolve_font(&font(name.to_string()))).is_some_and(|f| f.family == name);
    let _ = CHOSEN.set(choose(Os::CURRENT, &families, installed));
}

#[cfg(test)]
mod tests;
