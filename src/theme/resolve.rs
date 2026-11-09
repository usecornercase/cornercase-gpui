use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context as _, Result, bail};
use gpui::Rgba;
use serde_json::Value;

use super::colour::{from_bytes, luminance, opaque, parse_hex, rgb_bytes};
use super::file::{Section, ThemeFile, parse};
use super::tokens::{ANSI_NAMES, Chrome, OPAQUE, Opacity, Radius, Shape, TERMINAL_KEYS, TOKENS, derive};

pub(crate) const ROOT: &str = "cornercase-dark";
const DEEPEST: usize = 8;

pub(crate) trait Lookup {
    fn text(&self, id: &str) -> Option<String>;
}

impl Lookup for BTreeMap<String, String> {
    fn text(&self, id: &str) -> Option<String> {
        self.get(id).cloned()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Terminal {
    pub foreground: [u8; 3],
    pub background: [u8; 3],
    pub cursor: [u8; 3],
    pub ansi: [[u8; 3]; 16],
}

impl Terminal {
    pub(crate) fn palette(&self, index: u8) -> [u8; 3] {
        match index {
            0..=15 => self.ansi[usize::from(index)],
            16..=231 => {
                let i = index - 16;
                let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
                [level(i / 36), level((i / 6) % 6), level(i % 6)]
            }
            _ => {
                let v = 8 + (index - 232) * 10;
                [v, v, v]
            }
        }
    }

    pub(crate) fn json(&self) -> String {
        let palette: Vec<[u8; 3]> = (0..=255u8).map(|i| self.palette(i)).collect();
        serde_json::json!({ "foreground": self.foreground, "background": self.background, "palette": palette })
            .to_string()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Theme {
    pub id: String,
    pub name: String,
    pub chrome: Chrome,
    pub terminal: Terminal,
    pub opacity: Opacity,
    pub shape: Shape,
}

impl Theme {
    pub(crate) fn is_light(&self) -> bool {
        luminance(from_bytes(self.terminal.background)) > 0.5
    }
}

pub(crate) struct Resolved {
    pub theme: Theme,
    pub warnings: Vec<String>,
}

pub(crate) fn resolve(id: &str, lookup: &dyn Lookup) -> Result<Resolved> {
    let mut build = Build::new(id);
    for (name, file) in chain(id, lookup)? {
        build.who.clone_from(&name);
        build.layer(&file);
    }
    Ok(build.finish())
}

fn chain(id: &str, lookup: &dyn Lookup) -> Result<Vec<(String, ThemeFile)>> {
    let mut seen: Vec<String> = Vec::new();
    let mut files = Vec::new();
    let mut next = Some(id.to_string());
    while let Some(current) = next {
        if seen.contains(&current) {
            seen.push(current);
            bail!("themes extend each other in a circle: {}", seen.join(" → "));
        }
        if seen.len() >= DEEPEST {
            bail!("{id} extends more than {DEEPEST} themes deep: {}", seen.join(" → "));
        }
        let text = lookup.text(&current).with_context(|| match seen.last() {
            Some(from) => format!("{from} extends {current}, which is not a theme"),
            None => format!("{current} is not a theme"),
        })?;
        let file = parse(&text).with_context(|| current.clone())?;
        next = file.extends.clone().or_else(|| (current != ROOT).then(|| ROOT.to_string()));
        seen.push(current.clone());
        files.push((current, file));
    }
    files.reverse();
    Ok(files)
}

struct Build {
    id: String,
    name: String,
    who: String,
    chrome: Chrome,
    explicit: BTreeSet<&'static str>,
    terminal: Terminal,
    cursor: Option<[u8; 3]>,
    opacity: Opacity,
    shape: Shape,
    warnings: Vec<String>,
}

impl Build {
    fn new(id: &str) -> Build {
        Build {
            id: id.to_string(),
            name: id.to_string(),
            who: id.to_string(),
            chrome: Chrome::blank(),
            explicit: BTreeSet::new(),
            terminal: Terminal { foreground: [0; 3], background: [0; 3], cursor: [0; 3], ansi: [[0; 3]; 16] },
            cursor: None,
            opacity: Opacity { ground: 0.55, panes: 0.65, cards: 0.3, floating: 0.88, veil: 0.62 },
            shape: Shape { radius: Radius::Default, glow: true },
            warnings: Vec::new(),
        }
    }

    fn warn(&mut self, what: impl std::fmt::Display) {
        self.warnings.push(format!("{}: {what}", self.who));
    }

    fn layer(&mut self, file: &ThemeFile) {
        if let Some(name) = &file.name {
            self.name.clone_from(name);
        }
        self.chrome_section(&file.chrome);
        self.terminal_section(&file.terminal);
        self.opacity_section(&file.opacity);
        self.shape_section(&file.shape);
        for key in file.other.keys().filter(|key| !matches!(key.as_str(), "name" | "extends")) {
            self.warn(format_args!("{key} is not used by this version"));
        }
    }

    fn colour(&mut self, section: &str, key: &str, value: &Value) -> Option<Rgba> {
        match value.as_str().map(parse_hex) {
            Some(Ok(colour)) => Some(colour),
            Some(Err(error)) => {
                self.warn(format_args!("{section}.{key}: {error}; the base's value stays"));
                None
            }
            None => {
                self.warn(format_args!("{section}.{key} must be a colour written as text; the base's value stays"));
                None
            }
        }
    }

    fn chrome_section(&mut self, section: &Section) {
        for (key, value) in section {
            let Some(token) = TOKENS.iter().find(|token| token.key == key) else {
                self.warn(format_args!("chrome.{key} is not used by this version"));
                continue;
            };
            let Some(colour) = self.colour("chrome", key, value) else { continue };
            let colour = self.without_alpha(token.key, colour);
            self.chrome.set(token.key, colour);
            self.explicit.insert(token.key);
        }
    }

    fn without_alpha(&mut self, key: &str, colour: Rgba) -> Rgba {
        if !OPAQUE.contains(&key) {
            return colour;
        }
        if colour.a < 1. {
            self.warn(format_args!("chrome.{key}: its see-through-ness is set under opacity; the alpha is dropped"));
        }
        opaque(colour)
    }

    fn terminal_section(&mut self, section: &Section) {
        for (key, value) in section {
            let Some(known) = TERMINAL_KEYS.iter().find(|known| *known == key) else {
                self.warn(format_args!("terminal.{key} is not used by this version"));
                continue;
            };
            let Some(colour) = self.colour("terminal", key, value) else { continue };
            if colour.a < 1. {
                self.warn(format_args!("terminal.{key}: terminal colours are opaque; the alpha is dropped"));
            }
            self.terminal_colour(known, rgb_bytes(colour));
        }
    }

    fn terminal_colour(&mut self, key: &str, bytes: [u8; 3]) {
        match key {
            "foreground" => self.terminal.foreground = bytes,
            "background" => self.terminal.background = bytes,
            "cursor" => self.cursor = Some(bytes),
            _ => {
                if let Some(at) = ANSI_NAMES.iter().position(|name| *name == key) {
                    self.terminal.ansi[at] = bytes;
                }
            }
        }
    }

    fn opacity_section(&mut self, section: &Section) {
        for (key, value) in section {
            let Some(wanted) = value.as_f64().filter(|v| v.is_finite()) else {
                self.warn(format_args!("opacity.{key} must be a number"));
                continue;
            };
            #[expect(clippy::cast_possible_truncation, reason = "an opacity needs no more than f32")]
            let wanted = wanted as f32;
            if !self.opacity.set(key, wanted) {
                self.warn(format_args!("opacity.{key} is not used by this version"));
            } else if self.opacity.get(key).is_some_and(|kept| (kept - wanted).abs() > 1e-4) {
                self.warn(format_args!("opacity.{key}: {wanted} is outside its limits and was kept within them"));
            }
        }
    }

    fn shape_section(&mut self, section: &Section) {
        for (key, value) in section {
            match (key.as_str(), value) {
                ("radius", Value::String(text)) => {
                    self.shape.radius = serde_json::from_value(Value::String(text.clone())).unwrap_or_default();
                }
                ("glow", Value::Bool(on)) => self.shape.glow = *on,
                _ => self.warn(format_args!("shape.{key} is not used by this version")),
            }
        }
    }

    fn finish(mut self) -> Resolved {
        for token in TOKENS.iter().filter(|token| token.derived && !self.explicit.contains(token.key)) {
            if let Some(colour) = derive(&self.chrome, token.key) {
                self.chrome.set(token.key, colour);
            }
        }
        self.terminal.cursor = self.cursor.unwrap_or(self.terminal.foreground);
        let theme = Theme {
            id: self.id,
            name: self.name,
            chrome: self.chrome,
            terminal: self.terminal,
            opacity: self.opacity,
            shape: self.shape,
        };
        Resolved { theme, warnings: self.warnings }
    }
}

#[cfg(test)]
mod tests;
