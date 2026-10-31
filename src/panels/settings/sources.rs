use gpui::{AnyElement, Context, Div, SharedString, div, prelude::*, px};

use super::data::{self, Setting};
use super::{Role, SettingsPanel};
use crate::theme::{colours, radius};
use crate::ui::{self, Icon};

struct Source {
    key: String,
    shown: Vec<String>,
    index: usize,
    name: String,
    on: bool,
}

impl Source {
    fn toggle(&self, cx: &mut Context<SettingsPanel>) -> AnyElement {
        let (key, list, name) = (self.key.clone(), self.shown.clone(), self.name.clone());
        let id = SharedString::from(format!("source-{}-{}", self.key, self.name));
        ui::checkbox(id, self.on, self.name.clone())
            .flex_1()
            .text_color(if self.on { colours::text() } else { colours::faint() })
            .on_click(
                cx.listener(move |this, _, _, cx| this.save(&key, &data::list_toggle(&list, &name).join(","), cx)),
            )
            .into_any_element()
    }

    fn mover(&self, by: isize, cx: &mut Context<SettingsPanel>) -> AnyElement {
        let (glyph, hint) = if by < 0 { (Icon::ArrowUp, "Move up") } else { (Icon::ArrowDown, "Move down") };
        let at_end = self.index + 1 == self.shown.len();
        if !self.on || (by < 0 && self.index == 0) || (by > 0 && at_end) {
            return div().size(px(ui::ROW_ACTION)).flex_none().into_any_element();
        }
        let (key, list, index) = (self.key.clone(), self.shown.clone(), self.index);
        let id = SharedString::from(format!("source-{hint}-{}-{}", self.key, self.name));
        ui::row_action(id, glyph, hint, false)
            .on_click(
                cx.listener(move |this, _, _, cx| this.save(&key, &data::list_move(&list, index, by).join(","), cx)),
            )
            .into_any_element()
    }

    fn row(&self, cx: &mut Context<SettingsPanel>) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(4.))
            .pl(px(8.))
            .pr(px(2.))
            .py(px(1.))
            .rounded(px(radius::CONTROL))
            .hover(|s| s.bg(colours::hover()))
            .child(self.toggle(cx))
            .child(self.mover(-1, cx))
            .child(self.mover(1, cx))
    }
}

impl SettingsPanel {
    pub(super) fn render_sources(&self, setting: &Setting, cx: &mut Context<Self>) -> Div {
        let busy = self.busy.contains(&Role::Setting(setting.key.clone()));
        let shown = setting.items();
        let rows: Vec<Div> = data::sources(setting)
            .into_iter()
            .enumerate()
            .map(|(index, (name, on))| {
                Source { key: setting.key.clone(), shown: shown.clone(), index, name, on }.row(cx)
            })
            .collect();
        div().flex().flex_col().gap(px(1.)).pb(px(12.)).when(busy, |d| d.opacity(0.6)).children(rows)
    }
}
