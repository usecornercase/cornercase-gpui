use gpui::{
    AppContext as _, Context, Div, Entity, IntoElement, ParentElement as _, Render, Styled as _, WeakEntity, Window,
    div, prelude::FluentBuilder as _, px,
};

use super::{PersonasPage, described};
use crate::cli;
use crate::dialogs;
use crate::panels::flows::actions::toast;
use crate::panels::flows::dialog::{Submit, boxed, buttons, close_dialog, field, show_on, warning};
use crate::panels::flows::library::{Persona, Preview, count, import_args, kib, parse_preview};
use crate::panels::flows::widgets::{alert, error_text, panel_box};
use crate::store::{self, ToastKind};
use crate::theme::colours;
use crate::ui::{self, Icon, TextField};

const SOURCES: &str = "A .md file, a folder, https://… or owner/repo[/path] on GitHub";
const PICKING_WAITS: &str = "Picking some of them needs usecornercase/cornercase-terminal#939: \
                             for now an import takes every persona the source holds.";
const SHOWN: usize = 200;

pub(super) struct ImportPersonas {
    page: WeakEntity<PersonasPage>,
    source: Entity<TextField>,
    filter: Entity<TextField>,
    busy: bool,
    checked: Option<(String, Preview)>,
    error: Option<String>,
}

pub(super) fn open(page: WeakEntity<PersonasPage>, window: &mut Window, cx: &mut gpui::App) {
    let view = cx.new(|cx| ImportPersonas {
        page,
        source: field(SOURCES, cx, window),
        filter: field("Filter by name, id or description", cx, window),
        busy: false,
        checked: None,
        error: None,
    });
    let source = view.read(cx).source.clone();
    show_on(view.into(), &source, window, cx);
}

impl Submit for ImportPersonas {
    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let source = self.source.read(cx).text().trim().to_string();
        if source.is_empty() {
            self.error = Some("Give a file, a folder, an address or owner/repo.".into());
            cx.notify();
            return;
        }
        let checked = self.checked.as_ref().filter(|(from, _)| *from == source);
        let command = match checked {
            Some((_, preview)) => import_args(&source, false, preview.replaces()),
            None => import_args(&source, true, false),
        };
        let then = if checked.is_some() { Self::imported } else { Self::checked };
        self.busy = true;
        self.error = None;
        cx.notify();
        store::run(cx, command, move |this, result, cx| then(this, source, result, cx));
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        let source = self.source.read(cx).text();
        if !self.busy && self.checked.as_ref().is_some_and(|(from, _)| *from != source.trim()) {
            self.checked = None;
        }
        cx.notify();
    }
}

impl ImportPersonas {
    fn checked(&mut self, source: String, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        self.busy = false;
        match result.and_then(|out| parse_preview(&out)) {
            Ok(preview) => self.checked = Some((source, preview)),
            Err(e) => self.error = Some(cli::reason(&e)),
        }
        cx.notify();
    }

    fn imported(&mut self, _: String, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        self.busy = false;
        match result.and_then(|out| parse_preview(&out)) {
            Ok(preview) => {
                toast(format!("Imported {}", count(preview.personas.len())), ToastKind::Done, cx);
                close_dialog(cx);
                let _ = self.page.update(cx, |_, cx| PersonasPage::reload(cx));
            }
            Err(e) => self.error = Some(cli::reason(&e)),
        }
        cx.notify();
    }

    fn preview(&self, window: &Window, cx: &Context<Self>) -> Option<Div> {
        let (_, preview) = self.checked.as_ref()?;
        let shown = preview.matching(&self.filter.read(cx).text());
        let more = shown.len().saturating_sub(SHOWN);
        let replaced = preview.personas.iter().filter(|persona| persona.replaced).count();
        let head = match replaced {
            0 => format!("{} to import.", count(preview.personas.len())),
            n => format!("{} to import; {n} of them replace yours.", count(preview.personas.len())),
        };
        let list = panel_box()
            .flex()
            .flex_col()
            .p(px(8.))
            .gap(px(6.))
            .children(shown.into_iter().take(SHOWN).map(persona_line))
            .when(more > 0, |d| d.child(ui::dim(format!("and {more} more: filter to see them"))));
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(div().text_size(px(12.)).text_color(colours::soft()).child(head))
                .when(preview.personas.len() > 8, |d| d.child(boxed(&self.filter, window, cx)))
                .child(list)
                .child(alert(preview.check.clone(), colours::failed()))
                .child(ui::dim(PICKING_WAITS)),
        )
    }

    fn form(&self, window: &Window, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(ui::field("From", boxed(&self.source, window, cx)))
            .child(warning(
                "A persona is prompt text that a flow's agents follow without asking: Check lists what the source \
                 holds before anything is imported.",
            ))
            .children(self.preview(window, cx))
            .children(self.error.clone().map(error_text))
    }

    fn submit_label(&self) -> (String, &'static str) {
        match &self.checked {
            Some((_, preview)) => (format!("Import {}", count(preview.personas.len())), "Importing…"),
            None => ("Check".into(), "Checking…"),
        }
    }
}

fn persona_line(persona: &Persona) -> Div {
    let facts = [Some(persona.id.clone()), Some(kib(persona.bytes))].into_iter().flatten().collect::<Vec<_>>();
    let head = div()
        .flex()
        .items_center()
        .gap(px(8.))
        .text_size(px(12.))
        .child(div().flex().min_w_0().font_weight(gpui::FontWeight::MEDIUM).child(ui::one_line(persona.label())))
        .child(div().flex_1().min_w_0().flex().text_color(colours::faint()).child(ui::one_line(facts.join(" · "))))
        .when(persona.replaced, |d| {
            d.child(div().flex_none().text_size(px(11.)).text_color(colours::waiting()).child("replaces yours"))
        });
    div().flex().flex_col().gap(px(2.)).child(head).children(described(persona))
}

impl Render for ImportPersonas {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (label, busy) = self.submit_label();
        dialogs::frame(
            "Import personas",
            dialogs::fitted("persona-import-body", window, self.form(window, cx)),
            buttons(
                ("persona-import-cancel", "persona-import-run"),
                (Icon::Download, label),
                self.busy.then_some(busy),
                cx,
            ),
        )
        .w(px(560.))
    }
}
