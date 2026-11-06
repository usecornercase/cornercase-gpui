use gpui::{
    AnyElement, ClickEvent, Context, Div, Entity, IntoElement, SharedString, Stateful, Window, div, prelude::*, px,
};

use super::form::{self, Draft, Refusals};
use super::{Form, MachinesPanel};
use crate::cli;
use crate::model::machines::{Machine, MachineIcon};
use crate::panels::accounts::widgets;
use crate::panels::settings::widgets::capitalised;
use crate::store::Store;
use crate::theme::colours;
use crate::ui::{self, TextField, Tone};

const NAME_RULE: &str = "Letters, digits, `.`, `_` and `-`, at most 20";
const FOLDER_RULE: &str = "Where its clones go there: an absolute path or one starting with ~/";

fn labelled(label: &'static str, control: impl IntoElement, refusal: Option<String>, rule: String) -> Div {
    let note = match refusal {
        Some(refusal) => ui::error_line(refusal),
        None => widgets::hint(rule),
    };
    div().flex_1().min_w(px(240.)).flex().flex_col().gap(px(5.)).child(ui::field(label, control)).child(note)
}

fn fields_line() -> Div {
    widgets::form_line().items_start()
}

fn boxed(field: &Entity<TextField>, window: &Window, cx: &gpui::App) -> Div {
    ui::boxed(field, window, cx).overflow_hidden()
}

fn setup_note(here: Option<&str>, draft: &Draft) -> String {
    form::setup_note(here, &draft.destination, cli::host().as_deref())
}

impl MachinesPanel {
    pub(super) fn form_view(&self, window: &Window, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let form = self.form.as_ref()?;
        let draft = form.draft(cx);
        let store = Store::get(cx);
        let machines = store.read(cx).machines.clone();
        let others = form::others(&machines, form.was_name());
        let refused = form::refusals(&draft, &others, form.tried);
        let version = self.here.as_ref().map(|here| here.version.as_str());
        Some(
            widgets::form()
                .id("machine-form")
                .anchor_scroll(Some(self.form_place.clone()))
                .gap(px(12.))
                .child(Self::name_and_icon(form, &refused, window, cx))
                .child(Self::destination_and_folder(form, &refused, window, cx))
                .children(Self::hosts_line(form, &draft, &others, cx))
                .child(widgets::note(setup_note(version, &draft)))
                .children(form.error.clone().map(ui::error_line))
                .child(Self::form_buttons(form, cx)),
        )
    }

    fn name_and_icon(form: &Form, refused: &Refusals, window: &Window, cx: &mut Context<Self>) -> Div {
        let name = labelled("Name", boxed(&form.name, window, cx), refused.name.clone(), NAME_RULE.to_string());
        fields_line().child(name.max_w(px(320.))).child(ui::field("Icon", Self::icon_choices(form.icon, cx)))
    }

    fn icon_choices(chosen: MachineIcon, cx: &mut Context<Self>) -> Div {
        let choices: Vec<AnyElement> = MachineIcon::ALL
            .into_iter()
            .map(|icon| {
                let id = SharedString::from(format!("machine-icon-{}", icon.name()));
                ui::icon_segment(id, icon.icon(), capitalised(icon.name()), icon == chosen)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.mark_form(|form| form.icon = icon, cx);
                    }))
                    .into_any_element()
            })
            .collect();
        ui::segments(choices).flex_wrap()
    }

    fn destination_and_folder(form: &Form, refused: &Refusals, window: &Window, cx: &mut Context<Self>) -> Div {
        let rule = form::destination_rule(cli::host().as_deref());
        let destination = boxed(&form.destination, window, cx);
        fields_line().child(labelled("SSH destination", destination, refused.destination.clone(), rule)).child(
            labelled("Folder for clones", boxed(&form.folder, window, cx), refused.folder.clone(), FOLDER_RULE.into()),
        )
    }

    fn hosts_line(form: &Form, draft: &Draft, others: &[&Machine], cx: &mut Context<Self>) -> Option<Div> {
        let offered = form::hosts_offered(&form.hosts, &draft.destination, others);
        if offered.is_empty() {
            return None;
        }
        let chips: Vec<AnyElement> = offered.into_iter().map(|host| Self::host_chip(form, host, cx)).collect();
        Some(widgets::form_line().gap(px(6.)).child(widgets::hint("From ~/.ssh/config:")).children(chips))
    }

    fn host_chip(form: &Form, host: String, cx: &mut Context<Self>) -> AnyElement {
        let id = SharedString::from(format!("machine-host-{host}"));
        let field = form.destination.clone();
        ui::quiet_chip(host.clone())
            .id(id)
            .cursor_pointer()
            .hover(|s| s.bg(colours::hover()))
            .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                field.update(cx, |f, cx| f.set_text(host.clone(), cx));
            }))
            .into_any_element()
    }

    fn form_buttons(form: &Form, cx: &mut Context<Self>) -> Div {
        let buttons = div().flex().justify_end().items_center().gap(px(8.));
        let adding = form.was.is_none();
        if form.saving {
            let doing = if adding { "Adding…" } else { "Saving…" };
            return buttons.child(ui::busy_button("machine-saving", doing, Tone::Primary));
        }
        let go = if adding { "Add machine" } else { "Save" };
        buttons
            .child(ui::button("machine-cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| this.close_form(cx))))
            .child(ui::primary_button("machine-save", go).on_click(cx.listener(|this, _, _, cx| this.submit_form(cx))))
    }
}
