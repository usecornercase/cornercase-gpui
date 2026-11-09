use anyhow::{Result, bail};
use serde_json::Value;

use crate::theme::file::{Section, ThemeFile, parse};
use crate::theme::presets::is_preset;
use crate::theme::resolve::{Lookup, ROOT};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Part {
    Terminal,
    Chrome,
    Opacity,
    Shape,
}

fn section(file: &mut ThemeFile, part: Part) -> &mut Section {
    match part {
        Part::Terminal => &mut file.terminal,
        Part::Chrome => &mut file.chrome,
        Part::Opacity => &mut file.opacity,
        Part::Shape => &mut file.shape,
    }
}

pub(crate) fn set(file: &mut ThemeFile, part: Part, key: &str, value: Value) {
    section(file, part).insert(key.to_string(), value);
}

pub(crate) fn remove(file: &mut ThemeFile, part: Part, key: &str) {
    section(file, part).remove(key);
}

pub(crate) fn reset_all(file: &mut ThemeFile) {
    for part in [Part::Terminal, Part::Chrome, Part::Opacity, Part::Shape] {
        section(file, part).clear();
    }
}

pub(crate) fn copy_of(preset: &str, name: &str) -> ThemeFile {
    ThemeFile { name: Some(format!("My {name}")), extends: Some(preset.to_string()), ..ThemeFile::default() }
}

fn overlay(into: &mut Section, from: &Section) {
    for (key, value) in from {
        into.insert(key.clone(), value.clone());
    }
}

pub(crate) fn flatten(id: &str, lookup: &dyn Lookup) -> Result<ThemeFile> {
    let mut chain = Vec::new();
    let mut next = id.to_string();
    let mut preset = None;
    while chain.len() < 9 {
        let Some(text) = lookup.text(&next) else { bail!("{next} is not a theme") };
        let file = parse(&text)?;
        if is_preset(&next) {
            if chain.is_empty() {
                return Ok(file);
            }
            preset = Some(next);
            break;
        }
        let base = file.extends.clone().unwrap_or_else(|| ROOT.to_string());
        chain.push(file);
        next = base;
    }
    Ok(merged(chain, preset))
}

fn merged(mut chain: Vec<ThemeFile>, preset: Option<String>) -> ThemeFile {
    chain.reverse();
    let mut out = ThemeFile { extends: preset.filter(|base| base != ROOT), ..ThemeFile::default() };
    for file in chain {
        overlay(&mut out.terminal, &file.terminal);
        overlay(&mut out.chrome, &file.chrome);
        overlay(&mut out.opacity, &file.opacity);
        overlay(&mut out.shape, &file.shape);
        if file.name.is_some() {
            out.name = file.name;
        }
    }
    out
}

#[cfg(test)]
mod tests;
