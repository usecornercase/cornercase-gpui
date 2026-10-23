use gpui::{
    AnyElement, Context, Div, Focusable as _, IntoElement, SharedString, Stateful, Window, div, prelude::*, px,
};

use super::data::{self, Choice, Kind, Setting};
use super::pages::{AgentItem, Slot};
use super::{Pending, Role, SettingsPanel, role_id, widgets};
use crate::theme::colours;
use crate::ui;

fn text_width(kind: Kind) -> f32 {
    match kind {
        Kind::Number => 90.,
        Kind::Key => 160.,
        _ => 300.,
    }
}

fn chip(id: String, label: String, on: bool) -> Stateful<Div> {
    ui::button_coloured(SharedString::from(id), label, if on { colours::text() } else { colours::faint() })
        .when(on, |d| d.border_color(colours::accent()))
}

fn choice_row(setting: &Setting, choice: &Choice, cx: &mut Context<SettingsPanel>) -> Stateful<Div> {
    let (key, value) = (setting.key.clone(), choice.value.clone());
    let selected = choice.value == setting.text();
    ui::row(SharedString::from(format!("option-{key}-{value}")), selected)
        .child(div().w(px(14.)).flex_none().text_color(colours::accent()).child(if selected { "✓" } else { "" }))
        .child(
            div()
                .w(px(220.))
                .flex_none()
                .text_color(if choice.dangerous { colours::failed() } else { colours::text() })
                .child(choice.value.clone()),
        )
        .child(div().flex_1().text_size(px(12.)).text_color(colours::faint()).child(choice.note.clone()))
        .on_click(cx.listener(move |this, _, _, cx| this.choose(key.clone(), value.clone(), cx)))
}

struct ListEntry {
    key: String,
    items: Vec<String>,
    index: usize,
}

impl ListEntry {
    fn item(&self) -> &str {
        &self.items[self.index]
    }

    fn shift(&self, by: isize, glyph: &'static str, cx: &mut Context<SettingsPanel>) -> Stateful<Div> {
        let side = if by < 0 { "left" } else { "right" };
        let (key, list, index) = (self.key.clone(), self.items.clone(), self.index);
        ui::icon_button(SharedString::from(format!("{side}-{key}-{}", self.item())), glyph).on_click(
            cx.listener(move |this, _, _, cx| this.save(&key, &data::list_move(&list, index, by).join(","), cx)),
        )
    }

    fn view(&self, cx: &mut Context<SettingsPanel>) -> Div {
        let (key, list, name) = (self.key.clone(), self.items.clone(), self.item().to_string());
        let toggle = chip(format!("item-{key}-{name}"), format!("{name} ✕"), true).on_click(
            cx.listener(move |this, _, _, cx| this.save(&key, &data::list_toggle(&list, &name).join(","), cx)),
        );
        div()
            .flex()
            .items_center()
            .child(self.shift(-1, "‹", cx).when(self.index == 0, Styled::invisible))
            .child(toggle)
            .child(self.shift(1, "›", cx).when(self.index + 1 == self.items.len(), Styled::invisible))
    }
}

impl SettingsPanel {
    pub(super) fn render_control(&self, setting: &Setting, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let role = Role::Setting(setting.key.clone());
        if self.busy.contains(&role) && !setting.edited_as_text() {
            return ui::dim("saving…").into_any_element();
        }
        match setting.kind {
            Kind::Switch => Self::switch(setting, cx).into_any_element(),
            Kind::Choice if setting.as_segments() => Self::segments(setting, cx).into_any_element(),
            Kind::Choice => self.render_choice_button(setting, cx).into_any_element(),
            Kind::List => Self::render_list(setting, cx).into_any_element(),
            Kind::Text | Kind::Number | Kind::Key | Kind::Other => {
                self.render_text(&role, text_width(setting.kind), window, cx).into_any_element()
            }
        }
    }

    fn switch(setting: &Setting, cx: &mut Context<Self>) -> Div {
        let on = setting.on();
        let key = setting.key.clone();
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().text_size(px(12.)).text_color(colours::muted()).child(setting.switch_text().to_string()))
            .child(
                widgets::switch(SharedString::from(format!("switch-{key}")), on)
                    .on_click(cx.listener(move |this, _, _, cx| this.save(&key, &(!on).to_string(), cx))),
            )
    }

    fn segments(setting: &Setting, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let current = setting.text();
        let segments = setting
            .choices
            .iter()
            .map(|choice| {
                let (key, value) = (setting.key.clone(), choice.value.clone());
                ui::segment(
                    SharedString::from(format!("choice-{key}-{value}")),
                    choice.value.replace('_', " "),
                    choice.value == current,
                )
                .on_click(cx.listener(move |this, _, _, cx| this.choose(key.clone(), value.clone(), cx)))
                .into_any_element()
            })
            .collect();
        ui::segments(segments)
    }

    pub(super) fn render_text(
        &self,
        role: &Role,
        width: f32,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let Some(field) = self.fields.get(role) else { return div() };
        let focused = field.entity.read(cx).focus_handle(cx).is_focused(window);
        let edited = self.edited(role, cx);
        let busy = self.busy.contains(role);
        let id = role_id(role);
        let (save, revert) = (role.clone(), role.clone());
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(ui::input_box(field.entity.clone(), focused).w(px(width)).overflow_hidden())
            .when(busy, |d| d.child(ui::dim("saving…")))
            .when(edited && !busy, |d| {
                d.child(
                    ui::button(SharedString::from(format!("revert-{id}")), "Revert")
                        .on_click(cx.listener(move |this, _, _, cx| this.revert(&revert, cx))),
                )
                .child(
                    ui::primary_button(SharedString::from(format!("save-{id}")), "Save")
                        .on_click(cx.listener(move |this, _, _, cx| this.submit(&save, cx))),
                )
            })
    }

    fn render_choice_button(&self, setting: &Setting, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let key = setting.key.clone();
        let open = self.open_choice.as_deref() == Some(key.as_str());
        let colour = if setting.dangerous { colours::failed() } else { colours::text() };
        ui::button_coloured(
            SharedString::from(format!("pick-{key}")),
            format!("{}  {}", setting.text(), if open { "▴" } else { "▾" }),
            colour,
        )
        .when(open, |d| d.border_color(colours::accent()))
        .on_click(cx.listener(move |this, _, _, cx| {
            this.open_choice = if this.open_choice.as_deref() == Some(key.as_str()) { None } else { Some(key.clone()) };
            cx.notify();
        }))
    }

    pub(super) fn render_choice_list(&self, setting: &Setting, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.open_choice.as_deref() != Some(setting.key.as_str()) || setting.as_segments() {
            return None;
        }
        let rows: Vec<_> = setting.choices.iter().map(|choice| choice_row(setting, choice, cx)).collect();
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(1.))
                .mb(px(9.))
                .p(px(4.))
                .rounded(px(6.))
                .bg(colours::panel())
                .border_1()
                .border_color(colours::border())
                .when_some(setting.question.clone(), |d, q| d.child(ui::dim(q).px(px(8.)).py(px(3.))))
                .children(rows)
                .into_any_element(),
        )
    }

    pub(super) fn render_pending_choice(&self, setting: &Setting, cx: &mut Context<Self>) -> Option<AnyElement> {
        let Some(Pending::Choice { key, value }) = &self.pending else { return None };
        if *key != setting.key {
            return None;
        }
        let note = setting.choice(value).map(|c| c.note.clone()).unwrap_or_default();
        let question = format!("“{value}” lets agents act without asking ({note}). Use it?");
        let (key, value) = (key.clone(), value.clone());
        let go = ui::danger_button("pending-go", "Use it")
            .on_click(cx.listener(move |this, _, _, cx| this.save(&key, &value, cx)));
        Some(Self::ask(question, go, cx).into_any_element())
    }

    fn render_list(setting: &Setting, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let items = setting.items();
        let key = setting.key.clone();
        let mut row = div().flex().flex_wrap().justify_end().gap(px(4.)).max_w(px(460.));
        for index in 0..items.len() {
            row = row.child(ListEntry { key: key.clone(), items: items.clone(), index }.view(cx));
        }
        for choice in setting.choices.iter().filter(|c| !items.contains(&c.value)) {
            let (k, list, name) = (key.clone(), items.clone(), choice.value.clone());
            row = row.child(chip(format!("add-{key}-{name}"), format!("+ {name}"), false).on_click(
                cx.listener(move |this, _, _, cx| this.save(&k, &data::list_toggle(&list, &name).join(","), cx)),
            ));
        }
        row
    }

    pub(super) fn render_agent(&self, agent: &AgentItem<'_>, slot: Slot<'_>, cx: &mut Context<Self>) -> AnyElement {
        let mode = agent.mode;
        let mode_control = match mode {
            Some(mode) if self.busy.contains(&Role::Setting(mode.key.clone())) => ui::dim("saving…").into_any_element(),
            Some(mode) => self.render_choice_button(mode, cx).into_any_element(),
            None => ui::dim("no modes").into_any_element(),
        };
        let note = mode.and_then(|m| m.choice(&m.text())).map(|c| c.note.clone()).unwrap_or_default();
        let extra_control = agent.extra.map(|e| self.render_text(&Role::Setting(e.key.clone()), 240., slot.window, cx));
        let roles: Vec<Role> =
            [mode, agent.extra].into_iter().flatten().map(|s| Role::Setting(s.key.clone())).collect();
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .py(px(8.))
                    .when(!slot.first, |d| d.border_t_1().border_color(colours::border()))
                    .child(widgets::label(agent.kind.to_string(), note).flex_1().min_w(px(0.)))
                    .child(div().w(px(150.)).flex().justify_end().child(mode_control))
                    .children(extra_control),
            )
            .children(mode.and_then(|m| self.render_choice_list(m, cx)))
            .children(mode.and_then(|m| self.render_pending_choice(m, cx)))
            .children(roles.iter().filter_map(|r| self.error_of(r)))
            .into_any_element()
    }
}
