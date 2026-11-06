use gpui::{
    AnyElement, ClickEvent, Context, Div, IntoElement, Rgba, SharedString, Stateful, Window, div, prelude::*, px,
};

use super::data::{self, Doing, Listed, Note};
use super::form::Draft;
use super::{Form, MachinesPanel};
use crate::cli;
use crate::model::machines::{LinkState, Machine, MachineIcon};
use crate::panels::accounts::widgets::{self, STATUS};
use crate::theme::colours;
use crate::ui::{self, Icon, Tone};

fn state_colour(machine: &Machine) -> Option<Rgba> {
    if machine.problem.is_some() {
        return Some(colours::failed());
    }
    match machine.state {
        LinkState::Linked => Some(colours::done()),
        LinkState::Older => Some(colours::waiting()),
        LinkState::Unreachable => Some(colours::failed()),
        LinkState::Connecting => None,
    }
}

fn state_line(machine: &Machine, now: i64) -> Div {
    let colour = state_colour(machine);
    let ink = if machine.linked() { colours::soft() } else { colour.unwrap_or_else(colours::muted) };
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(7.))
        .text_size(px(12.))
        .child(ui::dot(colour))
        .child(div().flex_1().min_w_0().text_color(ink).child(ui::one_line(machine.words(now))))
}

fn lead(glyph: Icon) -> Div {
    widgets::lead(ui::icon(glyph, 16.).text_color(colours::soft()))
}

fn under(row: Listed<'_>) -> Vec<Div> {
    let line = |note: Note| match note {
        Note::Error(text) => ui::error_line(text),
        Note::Hint(text) => widgets::hint(text),
    };
    data::notes(row).into_iter().map(line).map(widgets::under_row).collect()
}

impl MachinesPanel {
    pub(super) fn list_card(&self, machines: &[Machine], window: &Window, cx: &mut Context<Self>) -> Div {
        let now = crate::panels::queue::now();
        let listed = data::listed(machines);
        let rows: Vec<AnyElement> = listed.iter().map(|row| self.machine_row(*row, now, window, cx)).collect();
        let editing = self.form.as_ref().and_then(Form::was_name);
        let under_a_row = editing.is_some_and(|name| listed.iter().any(|row| data::edits(*row, name)));
        let form = (!under_a_row).then(|| self.form_view(window, cx)).flatten();
        let rule = (!rows.is_empty()).then(widgets::rule);
        widgets::card().child(self.here_row()).children(rule).children(rows).children(form)
    }

    fn here_row(&self) -> Stateful<Div> {
        let server = cli::host();
        let (name, detail) = data::here_words(server.as_deref());
        let glyph = match &server {
            None => self.here.as_ref().map_or_else(MachineIcon::default, |here| here.icon).icon(),
            Some(_) => Icon::Server,
        };
        let version = self.here.as_ref().map(|here| format!("cornercase {}", here.version)).unwrap_or_default();
        widgets::row("machine-here", false)
            .child(lead(glyph))
            .child(widgets::name_column(widgets::title(name.to_string()), detail))
            .child(
                div()
                    .w(px(STATUS))
                    .flex_none()
                    .min_w_0()
                    .pl(px(14.))
                    .text_size(px(12.))
                    .text_color(colours::soft())
                    .child(ui::one_line(version)),
            )
            .child(widgets::actions())
    }

    fn machine_row(&self, row: Listed<'_>, now: i64, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let machine = row.machine;
        let doing = self.running_on(&machine.name).filter(|doing| data::shows_doing(*doing, row));
        let line = widgets::row(SharedString::from(format!("machine-{}", row.at)), doing.is_none())
            .child(lead(machine.icon.icon()))
            .child(widgets::name_column(
                widgets::title(machine.name.clone()),
                format!("{} · {}", machine.destination, machine.folder),
            ))
            .child(Self::status_cell(row, doing, now, cx))
            .child(Self::icon_actions(row, doing, cx))
            .when(doing == Some(Doing::Remove), |d| d.opacity(0.6));
        let edited = self.form.as_ref().and_then(Form::was_name).is_some_and(|name| data::edits(row, name));
        let form = edited.then(|| self.form_view(window, cx)).flatten();
        div().flex().flex_col().child(line).children(under(row)).children(form).into_any_element()
    }

    fn status_cell(row: Listed<'_>, doing: Option<Doing>, now: i64, cx: &mut Context<Self>) -> Div {
        let machine = row.machine;
        let cell = div().w(px(STATUS)).flex_none().min_w_0().flex().items_center().gap(px(8.));
        let spinner = SharedString::from(format!("machine-busy-{}", row.at));
        let line = match doing {
            Some(Doing::Remove | Doing::Update) => {
                let words = doing.map(Doing::words).unwrap_or_default();
                return cell.child(ui::busy(spinner, words));
            }
            _ if machine.in_progress() => {
                return cell.child(ui::busy(spinner, machine.words(now)));
            }
            _ => state_line(machine, now),
        };
        cell.child(line).children(Self::link_buttons(row, doing, cx))
    }

    fn link_buttons(row: Listed<'_>, doing: Option<Doing>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let machine = row.machine;
        let retry_id = SharedString::from(format!("machine-retry-{}", row.at));
        if doing == Some(Doing::Retry) {
            return vec![ui::busy_button(retry_id, Doing::Retry.words(), Tone::Plain).into_any_element()];
        }
        let update = machine.can_update().then(|| {
            let (ask, id) = (data::update_ask(&machine.name), SharedString::from(format!("machine-update-{}", row.at)));
            ui::button(id, "Update…")
                .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| Self::ask(ask.clone(), window, cx)))
                .into_any_element()
        });
        let retry = machine.can_retry().then(|| {
            let name = machine.name.clone();
            ui::button(retry_id, "Retry")
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(name.clone(), Doing::Retry, cx)))
                .into_any_element()
        });
        update.into_iter().chain(retry).collect()
    }

    fn icon_actions(row: Listed<'_>, doing: Option<Doing>, cx: &mut Context<Self>) -> Div {
        let actions = widgets::actions();
        if row.shadowed {
            return actions.child(widgets::slot()).child(widgets::slot());
        }
        if doing.is_some() || row.machine.updating {
            return actions.child(ui::resting_action(Icon::Pencil)).child(ui::resting_action(Icon::Trash));
        }
        let (machine, ask) = (row.machine.clone(), data::remove_ask(&row.machine.name));
        let edit = ui::row_action(SharedString::from(format!("machine-edit-{}", row.at)), Icon::Pencil, "Edit", false)
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.open_form(Some(Draft::of(&machine)), window, cx);
            }));
        let remove =
            ui::row_action(SharedString::from(format!("machine-remove-{}", row.at)), Icon::Trash, "Remove…", true)
                .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| Self::ask(ask.clone(), window, cx)));
        actions.child(edit).child(remove)
    }
}
