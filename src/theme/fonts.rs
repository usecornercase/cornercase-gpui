use std::panic::{self, AssertUnwindSafe};
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
    pub(crate) monospaced: bool,
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
        let fonts = CHOSEN.get_or_init(|| choose(Os::CURRENT, &[], |_| false, true));
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

pub(crate) fn choose(os: Os, families: &[String], installed: impl Fn(&str) -> bool, falls_back: bool) -> Fonts {
    if os == Os::Mac {
        return Fonts { mono: MAC_MONO.into(), sans: SYSTEM_SANS.into(), monospaced: true };
    }
    let mono = LINUX_MONO.into_iter().chain(own(families).filter(|name| is_mono(name))).find(|name| installed(name));
    let sans = LINUX_SANS
        .into_iter()
        .find(|name| installed(name))
        .or(falls_back.then_some(SYSTEM_SANS))
        .or_else(|| {
            let named = own(families).filter(|name| is_sans(name));
            named.chain(own(families).filter(|name| !is_mono(name))).find(|name| installed(name))
        })
        .or(mono)
        .unwrap_or(SYSTEM_SANS);
    Fonts { mono: mono.unwrap_or(sans).into(), sans: sans.into(), monospaced: mono.is_some() }
}

pub(crate) fn missing_mono(fonts: &Fonts) -> Option<&'static str> {
    (!fonts.monospaced).then_some(
        "No monospace font is installed, so the terminal draws in a proportional one: install DejaVu Sans Mono or Noto Sans Mono",
    )
}

fn own(families: &[String]) -> impl Iterator<Item = &str> {
    families.iter().map(String::as_str).filter(|name| !name.starts_with('.'))
}

fn named(family: &str, kind: &str) -> bool {
    family.split_whitespace().any(|word| word.ends_with(kind))
}

fn is_mono(family: &str) -> bool {
    named(family, "Mono")
}

fn is_sans(family: &str) -> bool {
    named(family, "Sans") && !is_mono(family)
}

pub(crate) fn settle(text: &TextSystem) -> Option<&'static str> {
    if Os::CURRENT == Os::Mac {
        return None;
    }
    let families = text.all_font_names();
    let resolved = |name: &str| quietly(|| text.resolve_font(&font(name.to_string())));
    let installed = |name: &str| {
        families.iter().any(|family| family == name)
            && resolved(name).and_then(|id| text.get_font_for_id(id)).is_some_and(|f| f.family == name)
    };
    let fonts = choose(Os::CURRENT, &families, installed, resolved(SYSTEM_SANS).is_some());
    let missing = missing_mono(&fonts);
    let _ = CHOSEN.set(fonts);
    missing
}

fn quietly<T>(probe: impl FnOnce() -> T) -> Option<T> {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let result = panic::catch_unwind(AssertUnwindSafe(probe)).ok();
    panic::set_hook(hook);
    result
}

#[cfg(test)]
mod tests;
