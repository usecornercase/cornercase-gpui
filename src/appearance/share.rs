use anyhow::{Context as _, Result, bail};
use gpui::Context;

use super::api::refuse;
use super::edit;
use super::{Appearance, ROOT, Texts, files, is_preset, parse, resolve};

impl Appearance {
    pub(crate) fn export_text(&self) -> Result<String> {
        let id = self.in_use_id();
        let file = edit::flatten(&id, &Texts(&self.user))?;
        Ok(file.text())
    }

    pub(crate) fn import(&mut self, text: &str, fallback_name: &str, cx: &mut Context<Self>) -> Result<String> {
        refuse(text)?;
        let file = parse(text).context("this is not a theme")?;
        let name = file.name.clone().unwrap_or_else(|| fallback_name.to_string());
        let mut named = file.clone();
        named.name = Some(name.clone());
        let base = named.extends.clone().unwrap_or_else(|| ROOT.to_string());
        if !is_preset(&base) && !self.user.contains_key(&base) {
            bail!("{name} extends {base}, which this window does not have");
        }
        let user = &self.user;
        let stem = files::free_stem(&name, &|stem| user.contains_key(stem));
        let mut probe = self.user.clone();
        probe.insert(stem.clone(), named.text());
        resolve(&stem, &Texts(&probe)).map_err(|why| anyhow::anyhow!("{why:#}"))?;
        self.save_theme(&stem, named.text());
        self.apply(cx);
        self.use_theme(&stem, cx);
        Ok(name)
    }
}
