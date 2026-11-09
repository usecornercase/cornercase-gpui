mod gallery;
mod picker;
mod preview;
mod shape;
mod sharing;
mod tokens;
mod transparency;

use std::path::PathBuf;
use std::sync::Arc;

use gpui::{
    AnyElement, App, Context, Div, InteractiveElement as _, IntoElement as _, MouseButton, ParentElement as _,
    Styled as _, Window, div, px,
};

use super::{SettingsPanel, widgets};
use crate::appearance::{Appearance, Listed, Part};
use crate::store::Store;
use crate::theme::Theme;
use crate::theme::colours;
use crate::ui::{self, Icon};

pub(super) use picker::OpenPicker;

#[derive(Default)]
pub(super) struct Look {
    pub(super) picker: Option<OpenPicker>,
    pub(super) import_error: Option<String>,
}

pub(super) struct Snap {
    pub(super) id: String,
    pub(super) theme: Arc<Theme>,
    pub(super) base: Option<Arc<Theme>>,
    pub(super) listed: Vec<Listed>,
    pub(super) problem: Option<String>,
    pub(super) warnings: Vec<String>,
    pub(super) follows: bool,
    pub(super) slots: (String, String),
    pub(super) mine: bool,
    pub(super) copy_of: Option<String>,
    pub(super) folder: PathBuf,
    pub(super) file: Option<PathBuf>,
}

impl Snap {
    pub(super) fn read(cx: &App) -> Snap {
        let appearance = Appearance::get(cx);
        let a = appearance.read(cx);
        let id = a.in_use_id();
        Snap {
            theme: a.in_use(),
            base: a.base(),
            listed: a.themes(),
            problem: a.problem().map(str::to_string),
            warnings: a.warnings().to_vec(),
            follows: a.follows(),
            slots: (a.slots().0.to_string(), a.slots().1.to_string()),
            mine: a.mine(),
            copy_of: a.editing_a_copy_of(),
            folder: a.folder().to_path_buf(),
            file: a.theme_file(&id),
            id,
        }
    }

    pub(super) fn clone_paths(&self) -> (PathBuf, Option<PathBuf>) {
        (self.folder.clone(), self.file.clone())
    }

    pub(super) fn changed(&self, part: Part, key: &str, now: f32) -> bool {
        let Some(base) = &self.base else { return false };
        match part {
            Part::Opacity => base.opacity.get(key).is_some_and(|was| (was - now).abs() > 1e-3),
            _ => false,
        }
    }
}

pub(super) const KEYWORDS: &str = "appearance theme themes colour colours color colors palette light dark system \
    transparency opacity glass blur contrast terminal presets import export";

impl SettingsPanel {
    pub(super) fn appearance_page(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let snap = Snap::read(cx);
        let problem = snap.problem.clone().map(|why| ui::callout(colours::failed(), "This theme cannot be used", why));
        widgets::column()
            .child(Self::appearance_heading(cx))
            .children(problem)
            .child(Self::theme_group(&snap, cx))
            .children(self.colour_groups(&snap, window, cx))
            .child(Self::transparency_group(&snap, cx))
            .child(Self::shape_group(&snap, cx))
            .child(self.share_group(&snap, cx))
            .children(Self::warning_list(&snap))
            .into_any_element()
    }

    fn appearance_heading(cx: &mut Context<Self>) -> Div {
        let about = super::data::about(super::data::APPEARANCE);
        let heading = widgets::heading(Icon::Palette, super::data::APPEARANCE.to_string(), about);
        let peek = ui::button("appearance-peek", "Hold to peek").on_mouse_down(MouseButton::Left, |_, _, cx| {
            cx.stop_propagation();
            Store::get(cx).update(cx, |store, cx| store.peek(true, cx));
        });
        let _ = cx;
        div().flex().items_start().gap(px(16.)).child(div().flex_1().min_w_0().child(heading)).child(peek)
    }

    fn warning_list(snap: &Snap) -> Option<Div> {
        if snap.warnings.is_empty() {
            return None;
        }
        let lines = snap.warnings.iter().map(|text| ui::warning_line(text.clone()));
        Some(widgets::group("Notes on this theme", div().flex().flex_col().gap(px(4.)).children(lines)))
    }
}

fn haystack() -> String {
    let presets =
        crate::theme::presets::PRESETS.iter().filter_map(|(_, text)| crate::theme::file::parse(text).ok()?.name);
    let labels = crate::theme::tokens::TOKENS.iter().map(|token| token.label.to_string());
    let words: Vec<String> = presets.chain(labels).chain([KEYWORDS.to_string()]).collect();
    words.join(" ").to_lowercase()
}

pub(crate) fn found_for(query: &str, shown: &str) -> Option<super::FoundSetting> {
    let text = haystack();
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let matched = !words.is_empty() && words.iter().all(|word| text.contains(word.as_str()));
    matched.then(|| super::FoundSetting {
        title: super::data::APPEARANCE.to_string(),
        page: super::data::APPEARANCE.to_string(),
        shown: shown.to_string(),
        ..super::FoundSetting::default()
    })
}

#[cfg(test)]
mod tests;
