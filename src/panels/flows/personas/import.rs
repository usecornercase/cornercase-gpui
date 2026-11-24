use std::collections::BTreeSet;

use gpui::{
    AppContext as _, ClickEvent, Context, Div, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::{PersonasPage, described};
use crate::cli::{self, Feature};
use crate::dialogs::{self, Question};
use crate::panels::flows::actions::toast;
use crate::panels::flows::catalogue::{
    Catalogue, Chosen, Group, Offered, Ticked, catalogue_args, confirmation, only_args, parse_catalogue, sentence,
};
use crate::panels::flows::dialog::{Submit, boxed, buttons, close_dialog, field, show_on, warning};
use crate::panels::flows::library::{count, kib, parse_preview};
use crate::panels::flows::widgets::{alert, error_text, panel_box};
use crate::platform::Os;
use crate::store::{self, Store, ToastKind};
use crate::theme::colours;
use crate::ui::{self, Icon, TextField};

const SOURCES: &str = "A .md file, a folder, https://… or owner/repo[/path] on GitHub";
const RISK: &str = "A persona is prompt text that a flow's agents follow without asking: Check lists what the source \
                    holds, and nothing is imported until you pick and confirm.";
const SHOWN: usize = 200;

pub(super) struct ImportPersonas {
    page: WeakEntity<PersonasPage>,
    source: Entity<TextField>,
    filter: Entity<TextField>,
    busy: bool,
    read: Option<(String, Catalogue)>,
    chosen: Chosen,
    open: BTreeSet<String>,
    error: Option<String>,
}

pub(super) fn open(page: WeakEntity<PersonasPage>, window: &mut Window, cx: &mut gpui::App) {
    let view = cx.new(|cx| ImportPersonas {
        page,
        source: field(SOURCES, cx, window),
        filter: field("Filter by name, id, file or description", cx, window),
        busy: false,
        read: None,
        chosen: Chosen::default(),
        open: BTreeSet::new(),
        error: None,
    });
    let source = view.read(cx).source.clone();
    show_on(view.into(), &source, window, cx);
}

impl Submit for ImportPersonas {
    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.busy || Self::lacks(cx).is_some() {
            return;
        }
        let source = self.source.read(cx).text().trim().to_string();
        if source.is_empty() {
            self.error = Some("Give a file, a folder, an address or owner/repo.".into());
            cx.notify();
            return;
        }
        if self.read.as_ref().is_some_and(|(from, _)| *from == source) {
            self.confirm(cx);
            return;
        }
        self.busy = true;
        self.error = None;
        cx.notify();
        store::run(cx, catalogue_args(&source), move |this, result, cx| this.read(source, result, cx));
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        let source = self.source.read(cx).text();
        if !self.busy && self.read.as_ref().is_some_and(|(from, _)| *from != source.trim()) {
            self.read = None;
            self.chosen = Chosen::default();
            self.open.clear();
        }
        cx.notify();
    }
}

impl ImportPersonas {
    fn lacks(cx: &gpui::App) -> Option<String> {
        let lacks = Store::get(cx).read(cx).releases().lacks(Feature::PersonaCatalogue)?;
        Some(format!("Importing personas needs {}.", cli::newer(Feature::PersonaCatalogue.since(), lacks, Os::CURRENT)))
    }

    fn read(&mut self, source: String, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        self.busy = false;
        match result.and_then(|out| parse_catalogue(&out)) {
            Ok(catalogue) => {
                self.read = Some((source, catalogue));
                self.chosen = Chosen::default();
                self.open.clear();
            }
            Err(e) => self.error = Some(cli::reason(&e)),
        }
        cx.notify();
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some((source, catalogue)) = &self.read else { return };
        let picked = self.chosen.picked(catalogue);
        if picked.is_empty() {
            self.error = Some("Tick the personas to import, or a division to take all of it.".into());
            cx.notify();
            return;
        }
        let title = format!("Import {} from {}?", count(picked.len()), catalogue.label);
        let command = only_args(source, &picked);
        let this = cx.entity().downgrade();
        let question = Question::new(title, confirmation(catalogue, &picked), "Import", Vec::new()).on_yes(move |cx| {
            let command = command.clone();
            let _ = this.update(cx, |this, cx| this.import(command, cx));
        });
        let back = self.source.read(cx).focus_handle(cx);
        dialogs::confirm_over(question, back, cx);
    }

    fn import(&mut self, command: Vec<String>, cx: &mut Context<Self>) {
        self.busy = true;
        self.error = None;
        cx.notify();
        store::run(cx, command, Self::imported);
    }

    fn imported(&mut self, result: anyhow::Result<String>, cx: &mut Context<Self>) {
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

    fn toggle(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some((_, catalogue)) = &self.read else { return };
        if let Some(offered) = catalogue.personas.iter().find(|offered| offered.persona.id == id) {
            self.chosen.toggle(offered);
            self.error = None;
            cx.notify();
        }
    }

    fn toggle_group(&mut self, label: &str, cx: &mut Context<Self>) {
        let Some((_, catalogue)) = &self.read else { return };
        let filter = self.filter.read(cx).text();
        if let Some(group) = catalogue.groups(&filter).iter().find(|group| group.label == label) {
            self.chosen.toggle_group(group);
            self.error = None;
            cx.notify();
        }
    }

    fn fold(&mut self, label: &str, cx: &mut Context<Self>) {
        if !self.open.remove(label) {
            self.open.insert(label.to_string());
        }
        cx.notify();
    }

    fn catalogue(&self, window: &Window, cx: &Context<Self>) -> Option<Div> {
        let (_, catalogue) = self.read.as_ref()?;
        let filter = self.filter.read(cx).text();
        let unfolded = !filter.trim().is_empty();
        let mut left = SHOWN;
        let mut list = panel_box().flex().flex_col().p(px(6.)).gap(px(2.));
        for group in catalogue.groups(&filter) {
            let open = unfolded || self.open.contains(&group.label);
            list = list.child(self.group_row(&group, open, cx));
            if open {
                let rows: Vec<&&Offered> = group.offered.iter().take(left).collect();
                left -= rows.len();
                list = list.children(rows.into_iter().map(|offered| self.persona_row(offered, cx)));
            }
        }
        let list =
            list.when(left == 0, |d| d.child(ui::dim(format!("The first {SHOWN} are shown: filter to see the rest."))));
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(self.origin(catalogue))
                .when(catalogue.personas.len() > 8, |d| d.child(boxed(&self.filter, window, cx)))
                .child(list)
                .child(alert(sentence(&catalogue.check), colours::failed())),
        )
    }

    fn origin(&self, catalogue: &Catalogue) -> Div {
        let picked = self.chosen.picked(catalogue).len();
        let head = format!("{} · {}", catalogue.label, catalogue.summary());
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .text_size(px(12.))
            .child(div().text_color(colours::soft()).child(head))
            .child(ui::dim(format!("{} picked.", count(picked))))
    }

    fn group_row(&self, group: &Group<'_>, open: bool, cx: &Context<Self>) -> impl IntoElement + use<> {
        let ticked = self.chosen.ticked(group);
        let importable = group.offered.iter().filter(|offered| offered.importable()).count();
        let picked = group.offered.iter().filter(|offered| self.chosen.has(offered)).count();
        let (tick_label, fold_label) = (group.label.clone(), group.label.clone());
        let chevron = if open { Icon::ChevronDown } else { Icon::ChevronRight };
        let tick = div()
            .id(SharedString::from(format!("division-tick-{}", group.label)))
            .cursor_pointer()
            .when(importable > 0, |d| d.child(ui::tick_box(ticked == Ticked::All)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_group(&tick_label, cx)));
        let name = div()
            .id(SharedString::from(format!("division-{}", group.label)))
            .flex_1()
            .flex()
            .items_center()
            .gap(px(6.))
            .cursor_pointer()
            .child(ui::icon(chevron, 12.).text_color(colours::faint()))
            .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(group.label.clone()))
            .child(div().text_color(colours::faint()).child(format!("{picked} of {importable}")))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.fold(&fold_label, cx)));
        div().flex().items_center().gap(px(8.)).px(px(4.)).py(px(4.)).text_size(px(12.)).child(tick).child(name)
    }

    fn persona_row(&self, offered: &Offered, cx: &Context<Self>) -> impl IntoElement + use<> {
        let id = offered.persona.id.clone();
        let facts =
            [offered.persona.id.clone(), kib(offered.persona.bytes)].into_iter().filter(|fact| !fact.is_empty());
        let head = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().flex().min_w_0().font_weight(gpui::FontWeight::MEDIUM).child(ui::one_line(offered.title())))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .text_color(colours::faint())
                    .child(ui::one_line(facts.collect::<Vec<_>>().join(" · "))),
            )
            .when(offered.persona.replaced, |d| {
                d.child(div().flex_none().text_size(px(11.)).text_color(colours::waiting()).child("replaces yours"))
            });
        let text = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(head)
            .children(described(&offered.persona))
            .children(offered.problem.clone().map(|problem| div().text_color(colours::failed()).child(problem)));
        let row = div().flex().items_start().gap(px(8.)).pl(px(22.)).pr(px(4.)).py(px(3.)).text_size(px(12.));
        if !offered.importable() {
            return row.child(ui::tick_box(false).opacity(0.3)).child(text).into_any_element();
        }
        row.id(SharedString::from(format!("persona-tick-{id}")))
            .cursor_pointer()
            .hover(|s| s.bg(colours::hover()))
            .child(ui::tick_box(self.chosen.has(offered)))
            .child(text)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle(&id, cx)))
            .into_any_element()
    }

    fn form(&self, window: &Window, cx: &Context<Self>) -> Div {
        let lacks = Self::lacks(cx);
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(ui::field("From", boxed(&self.source, window, cx)))
            .child(warning(RISK))
            .children(lacks.map(ui::dim))
            .children(self.catalogue(window, cx))
            .children(self.error.clone().map(error_text))
    }

    fn submit_label(&self) -> (String, &'static str) {
        match &self.read {
            Some((_, catalogue)) => match self.chosen.picked(catalogue).len() {
                0 => ("Import selected".into(), "Importing…"),
                n => (format!("Import {}", count(n)), "Importing…"),
            },
            None => ("Check".into(), "Checking…"),
        }
    }
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
        .w(px(620.))
    }
}
