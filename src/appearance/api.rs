use anyhow::{Result, bail};
use gpui::{Context, Rgba};
use serde_json::{Value, json};

use super::edit::{self, Part};
use super::*;

impl Appearance {
    pub(crate) fn themes(&self) -> Vec<Listed> {
        let listed = |id: &String, entry: &Result<Entry, String>| {
            let preset = is_preset(id);
            let name = entry.as_ref().map_or_else(|_| self.file_name(id), |e| e.theme.name.clone());
            let light = entry.as_ref().is_ok_and(|e| e.theme.is_light());
            Listed { id: id.clone(), name, preset, light, problem: entry.as_ref().err().cloned() }
        };
        let mut presets: Vec<Listed> = PRESETS
            .iter()
            .filter_map(|(id, _)| self.resolved.get_key_value(*id))
            .map(|(id, entry)| listed(id, entry))
            .collect();
        let mut mine: Vec<Listed> = self
            .resolved
            .iter()
            .filter(|(id, _)| !PRESETS.iter().any(|(p, _)| p == id))
            .map(|(id, e)| listed(id, e))
            .collect();
        mine.sort_by_key(|l| l.name.to_lowercase());
        presets.extend(mine);
        presets
    }

    fn file_name(&self, id: &str) -> String {
        let named = self.user.get(id).and_then(|text| parse(text).ok()).and_then(|file| file.name);
        named.unwrap_or_else(|| id.to_string())
    }

    pub(crate) fn theme_of(&self, id: &str) -> Option<Arc<Theme>> {
        self.resolved.get(id)?.as_ref().ok().map(|entry| entry.theme.clone())
    }

    pub(crate) fn in_use_id(&self) -> String {
        self.choice.in_use(self.system_dark).to_string()
    }

    pub(crate) fn in_use(&self) -> Arc<Theme> {
        self.applied.clone().unwrap_or_else(|| Arc::new(fallback()))
    }

    pub(crate) fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }

    pub(crate) fn warnings(&self) -> &[String] {
        &self.warnings
    }

    pub(crate) fn follows(&self) -> bool {
        self.choice.follow_system
    }

    pub(crate) fn slots(&self) -> (&str, &str) {
        (&self.choice.light, &self.choice.dark)
    }

    pub(crate) fn settled(&self) -> u64 {
        self.settled
    }

    pub(crate) fn take_notices(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notices)
    }

    pub(crate) fn folder(&self) -> &std::path::Path {
        &self.folder
    }

    pub(crate) fn theme_file(&self, id: &str) -> Option<std::path::PathBuf> {
        self.user.contains_key(id).then(|| files::theme_path(&self.folder, id))
    }

    pub(crate) fn base(&self) -> Option<Arc<Theme>> {
        let id = self.in_use_id();
        let text = self.user.get(&id)?;
        let base = parse(text).ok()?.extends.unwrap_or_else(|| ROOT.to_string());
        self.theme_of(&base)
    }

    pub(crate) fn base_name(&self) -> Option<String> {
        self.base().map(|theme| theme.name.clone())
    }

    pub(crate) fn editing_a_copy_of(&self) -> Option<String> {
        let id = self.in_use_id();
        is_preset(&id).then(|| self.in_use().name.clone())
    }

    pub(crate) fn system_changed(&mut self, dark_now: bool, cx: &mut Context<Self>) {
        if self.system_dark != dark_now {
            self.system_dark = dark_now;
            if self.choice.follow_system {
                self.apply(cx);
            }
        }
    }

    pub(crate) fn use_theme(&mut self, id: &str, cx: &mut Context<Self>) {
        let light = self.theme_of(id).is_some_and(|theme| theme.is_light());
        self.choice.choose(id, light);
        self.save_choice();
        self.apply(cx);
    }

    pub(crate) fn follow_system(&mut self, on: bool, cx: &mut Context<Self>) {
        let light = self.in_use().is_light();
        self.choice.follow(on, self.system_dark, light);
        self.save_choice();
        self.apply(cx);
    }

    pub(crate) fn mine(&self) -> bool {
        self.user.contains_key(&self.in_use_id())
    }

    fn target(&mut self) -> String {
        let id = self.in_use_id();
        if self.user.contains_key(&id) {
            return id;
        }
        let name = self.in_use().name.clone();
        let copy = self.new_copy(&id, &name);
        let light = self.in_use().is_light();
        self.choice.choose(&copy, light);
        self.save_choice();
        copy
    }

    fn new_copy(&mut self, base: &str, name: &str) -> String {
        let file = edit::copy_of(base, name);
        let wanted = file.name.clone().unwrap_or_default();
        let user = &self.user;
        let stem = files::free_stem(&wanted, &|stem| user.contains_key(stem));
        self.save_theme(&stem, file.text());
        stem
    }

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut ThemeFile)) {
        let id = self.target();
        let mut file = self.user.get(&id).and_then(|text| parse(text).ok()).unwrap_or_default();
        change(&mut file);
        self.save_theme(&id, file.text());
        self.apply(cx);
    }

    pub(crate) fn set_colour(&mut self, part: Part, key: &str, colour: Option<Rgba>, cx: &mut Context<Self>) {
        let key = key.to_string();
        self.edit(cx, |file| match colour {
            Some(colour) => edit::set(file, part, &key, json!(written(&key, colour))),
            None => edit::remove(file, part, &key),
        });
    }

    pub(crate) fn set_opacity(&mut self, key: &str, value: Option<f32>, cx: &mut Context<Self>) {
        let key = key.to_string();
        self.edit(cx, |file| match value {
            Some(value) => edit::set(file, Part::Opacity, &key, json!((f64::from(value) * 100.).round() / 100.)),
            None => edit::remove(file, Part::Opacity, &key),
        });
    }

    pub(crate) fn set_radius(&mut self, radius: Radius, cx: &mut Context<Self>) {
        self.edit(cx, |file| edit::set(file, Part::Shape, "radius", json!(radius.key())));
    }

    pub(crate) fn set_glow(&mut self, on: bool, cx: &mut Context<Self>) {
        self.edit(cx, |file| edit::set(file, Part::Shape, "glow", Value::Bool(on)));
    }

    pub(crate) fn reset_all(&mut self, cx: &mut Context<Self>) {
        self.edit(cx, edit::reset_all);
    }

    pub(crate) fn duplicate(&mut self, cx: &mut Context<Self>) -> String {
        let id = self.in_use_id();
        let name = self.in_use().name.clone();
        let copy = self.new_copy(&id, &name);
        self.use_theme(&copy, cx);
        copy
    }

    pub(crate) fn delete(&mut self, cx: &mut Context<Self>) {
        let id = self.in_use_id();
        if !self.user.contains_key(&id) {
            return;
        }
        self.user.remove(&id);
        self.pending.insert(files::theme_path(&self.folder, &id), None);
        self.choice = Choice { follow_system: self.choice.follow_system, ..Choice::default() };
        self.save_choice();
        self.apply(cx);
    }

    pub(crate) fn warn(&mut self, text: String) {
        self.notices.push(text);
    }
}

fn written(key: &str, colour: Rgba) -> String {
    let opaque_only = OPAQUE.contains(&key) || !is_chrome_key(key);
    hex(if opaque_only { opaque(colour) } else { colour }, !opaque_only)
}

fn is_chrome_key(key: &str) -> bool {
    crate::theme::tokens::TOKENS.iter().any(|token| token.key == key)
}

pub(super) fn refuse(text: &str) -> Result<()> {
    if text.trim().is_empty() {
        bail!("the file is empty");
    }
    Ok(())
}
