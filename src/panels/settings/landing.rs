use std::cell::Cell;
use std::rc::Rc;

use gpui::{AnyElement, Bounds, Context, Div, IntoElement, Pixels, canvas, div, point, prelude::*, px};
use serde::Deserialize;

use super::data::{self, Described, Item, Section};
use super::{SettingsPanel, widgets};
use crate::panels::PanelKind;
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui::{TextField, glass, motion};

const MARK_BEYOND: f32 = 8.;
const MARK_TINT: f32 = 0.2;
const MARK_EDGE: f32 = 0.55;
const ROW_FROM_TOP: f32 = 3.;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct FoundSetting {
    pub(crate) title: String,
    pub(crate) page: String,
    pub(crate) section: String,
    pub(crate) label: String,
    pub(crate) key: Option<String>,
    pub(crate) shown: String,
    pub(crate) note: String,
    pub(crate) hidden: bool,
}

impl FoundSetting {
    pub(crate) fn is_tab(&self) -> bool {
        self.key.is_none() && self.title == self.page
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Answer {
    results: Vec<FoundSetting>,
}

pub(crate) fn parse_found(text: &str) -> anyhow::Result<Vec<FoundSetting>> {
    Ok(serde_json::from_str::<Answer>(text)?.results)
}

pub(crate) fn sheet_of(page: &str) -> Option<PanelKind> {
    if data::ORDER.contains(&page) {
        return None;
    }
    PanelKind::ALL.into_iter().find(|kind| !kind.docked() && kind.title() == page)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Spot {
    pub(super) item: Option<Item>,
    pub(super) notice: Option<String>,
}

pub(super) struct Landed {
    found: FoundSetting,
    serial: u64,
    row: Rc<Cell<Option<Bounds<Pixels>>>>,
    scrolled: bool,
}

fn elsewhere(found: &FoundSetting) -> String {
    format!("{} is in cornercase's own window, Settings › {}.", widgets::capitalised(&found.label), found.page)
}

fn holds_key(d: &Described, item: &Item, key: &str) -> bool {
    match item {
        Item::Setting(i) => d.settings[*i].key == key,
        Item::Connection(i) => d.connections[*i].key == key,
        Item::Agent { mode, extra, .. } => [mode, extra].into_iter().flatten().any(|i| d.settings[*i].key == key),
    }
}

fn row_with_key(d: &Described, sections: &[Section], key: &str) -> Option<Item> {
    sections.iter().flat_map(|s| &s.items).find(|item| holds_key(d, item, key)).cloned()
}

fn first_of_section(sections: &[Section], title: &str) -> Option<Item> {
    sections.iter().find(|s| !title.is_empty() && s.title == title)?.items.first().cloned()
}

pub(super) fn spot(d: &Described, found: &FoundSetting) -> Spot {
    if found.page == data::APPEARANCE {
        return Spot::default();
    }
    if !data::pages(d).contains(&found.page) {
        return Spot { item: None, notice: Some(elsewhere(found)) };
    }
    if found.is_tab() {
        return Spot::default();
    }
    let sections = data::sections(d, &found.page);
    if let Some(item) = found.key.as_deref().and_then(|key| row_with_key(d, &sections, key)) {
        return Spot { item: Some(item), notice: None };
    }
    let near = first_of_section(&sections, &found.section);
    let notice = match (found.hidden, &near) {
        (true, _) => Some(format!("{} {}", widgets::capitalised(&found.label), found.note)),
        (false, None) => Some(elsewhere(found)),
        (false, Some(_)) => None,
    };
    Spot { item: near, notice }
}

pub(super) fn offset_showing(row: (f32, f32), view: (f32, f32), offset: f32) -> f32 {
    if row.0 >= view.0 && row.1 <= view.1 {
        return offset;
    }
    offset - (row.0 - view.0 - (view.1 - view.0) / ROW_FROM_TOP)
}

fn mark() -> Div {
    let accent = colours::accent();
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(px(-MARK_BEYOND))
        .right(px(-MARK_BEYOND))
        .rounded(px(radius::control()))
        .bg(glass::tint(accent, MARK_TINT))
        .border_1()
        .border_color(glass::tint(accent, MARK_EDGE))
}

impl SettingsPanel {
    pub(super) fn take_landing(&mut self, cx: &mut Context<Self>) {
        let Some(found) = Store::get(cx).update(cx, |s, _| s.landing.take()) else { return };
        self.landings += 1;
        self.choice = None;
        self.search.update(cx, TextField::clear);
        self.to_top();
        if found.page == data::APPEARANCE
            || self.described.as_ref().is_none_or(|d| data::pages(d).contains(&found.page))
        {
            self.page.clone_from(&found.page);
        }
        self.landed = Some(Landed { found, serial: self.landings, row: Rc::default(), scrolled: false });
        cx.notify();
    }

    pub(super) fn settle_landing(&mut self) {
        let landed = self.landed.as_ref().zip(self.described.as_ref());
        self.spot = landed.map(|(landed, d)| spot(d, &landed.found)).unwrap_or_default();
        let Some(landed) = self.landed.as_mut().filter(|landed| !landed.scrolled) else { return };
        let Some(row) = landed.row.take() else { return };
        landed.scrolled = true;
        let (view, offset) = (self.scroll.bounds(), self.scroll.offset());
        let y = offset_showing(
            (f32::from(row.top()), f32::from(row.bottom())),
            (f32::from(view.top()), f32::from(view.bottom())),
            f32::from(offset.y),
        );
        self.scroll.set_offset(point(offset.x, px(y)));
    }

    pub(super) fn landed_row(&self, row: AnyElement) -> AnyElement {
        let Some(landed) = &self.landed else { return row };
        let (at, waiting) = (landed.row.clone(), !landed.scrolled);
        let measure = canvas(
            move |bounds, window, _| {
                if waiting {
                    at.set(Some(bounds));
                    window.request_animation_frame();
                }
            },
            |_, (), _, _| {},
        );
        div()
            .relative()
            .flex()
            .flex_col()
            .child(motion::mark(mark(), ("settings-landed", landed.serial)))
            .child(measure.absolute().size_full())
            .child(row)
            .children(self.spot.notice.clone().map(|notice| widgets::under(widgets::hint(notice))))
            .into_any_element()
    }

    pub(super) fn page_notice(&self) -> Option<Div> {
        self.spot.notice.clone().filter(|_| self.spot.item.is_none()).map(widgets::hint)
    }
}

#[cfg(test)]
mod tests;
