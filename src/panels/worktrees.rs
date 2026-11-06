mod actions;
mod data;
mod rows;

use std::collections::HashSet;

use gpui::{
    AnyElement, AnyView, App, AppContext as _, ClickEvent, Context, Entity, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::panels::PanelKind;
use crate::panels::poll::parse_text;
use crate::panels::queue::now;
use crate::store::{self, Store, ToastKind};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};
use data::{List, Order, Worktree, list_command, scope_note, shown, total};
use rows::activity;

const NO_WORKTREES: &str = "No worktrees on disk. Workspaces made with a worktree of their own show here.";
const NO_MATCH: &str = "No worktree matches the filter.";
const CLOSED_NOTE: &str = "Worktrees of closed projects, and leftovers git no longer knows, are listed to show what \
                           they take; removing them here needs a `worktrees remove` command in cornercase.";

pub(crate) struct WorktreesPanel {
    project: Option<u64>,
    worktrees: Option<Vec<Worktree>>,
    error: Option<String>,
    measuring: bool,
    generation: u64,
    order: Order,
    ticked: HashSet<String>,
    filter: Entity<TextField>,
    open: bool,
}

impl WorktreesPanel {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| TextField::new("Filter by project or branch", cx).clears_on_escape());
        cx.subscribe_in(&filter, window, |_, _, _: &TextFieldEvent, _, cx| cx.notify()).detach();
        let store = Store::get(cx);
        cx.observe(&store, |this, store, cx| {
            let open = store.read(cx).sheet == Some(PanelKind::Worktrees);
            let asked = store.update(cx, |s, _| s.take_project_sheet(PanelKind::Worktrees));
            if (open && !this.open) || asked.is_some() {
                this.show_project(asked, cx);
            }
            this.open = open;
        })
        .detach();
        let project = store.update(cx, |s, _| s.take_project_sheet(PanelKind::Worktrees));
        let mut this = Self {
            project,
            worktrees: None,
            error: None,
            measuring: false,
            generation: 0,
            order: Order::default(),
            ticked: HashSet::new(),
            filter,
            open: true,
        };
        this.reload(cx);
        this
    }

    fn show_project(&mut self, project: Option<u64>, cx: &mut Context<Self>) {
        if self.project != project {
            self.project = project;
            self.worktrees = None;
            self.error = None;
            self.ticked.clear();
        }
        self.reload(cx);
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        self.measuring = true;
        self.load(false, cx);
        self.load(true, cx);
        cx.notify();
    }

    fn load(&mut self, sizes: bool, cx: &mut Context<Self>) {
        let generation = self.generation;
        store::run_partial(cx, list_command(sizes, self.project), move |this, result, cx| {
            if this.generation != generation || (!sizes && !this.measuring) {
                return;
            }
            this.measuring &= !sizes;
            match parse_text::<List>(result) {
                Ok(list) => this.loaded(list.worktrees),
                Err(e) if sizes && this.worktrees.is_some() => {
                    Store::get(cx).update(cx, |s, cx| s.toast(e, ToastKind::Error, cx));
                }
                Err(e) => this.error = Some(e),
            }
            cx.notify();
        });
    }

    fn loaded(&mut self, worktrees: Vec<Worktree>) {
        let paths: HashSet<&str> = worktrees.iter().map(|w| w.path.as_str()).collect();
        self.ticked.retain(|p| paths.contains(p.as_str()));
        self.worktrees = Some(worktrees);
        self.error = None;
    }

    fn toggle(&mut self, path: String, cx: &mut Context<Self>) {
        if !self.ticked.remove(&path) {
            self.ticked.insert(path);
        }
        cx.notify();
    }

    fn set_order(&mut self, order: Order, cx: &mut Context<Self>) {
        self.order = order;
        cx.notify();
    }

    fn order_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let tab = |order: Order, label: &'static str, cx: &mut Context<Self>| {
            ui::segment(label, label, self.order == order)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_order(order, cx)))
                .into_any_element()
        };
        ui::segments(vec![tab(Order::SafeFirst, "Safe first", cx), tab(Order::LargestFirst, "Largest first", cx)])
    }

    fn toolbar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let focused = self.filter.read(cx).focus_handle(cx).is_focused(window);
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(div().flex_1().child(ui::input_box(self.filter.clone(), focused)))
            .child(self.order_tabs(cx))
            .child(
                ui::icon_button("reload", "↻")
                    .py(px(3.))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.reload(cx))),
            )
    }

    fn scope_line(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let project = self.project?;
        let session = Store::get(cx).read(cx).session.clone();
        let name = session.as_ref().and_then(|s| s.projects.iter().find(|p| p.id == project)).map(|p| p.name.clone());
        Some(
            div().flex().items_center().gap(px(10.)).child(ui::dim(scope_note(name.as_deref()))).child(
                ui::button("worktrees-every", "Show every worktree")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.show_project(None, cx))),
            ),
        )
    }

    fn footer(&self, worktrees: &[Worktree], cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let store = Store::get(cx);
        let ticked: Vec<Worktree> = worktrees
            .iter()
            .filter(|w| self.ticked.contains(&w.path) && w.removable() && activity(w, store.read(cx)).is_none())
            .cloned()
            .collect();
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(ui::dim(total(worktrees, now())))
            .when(self.measuring, |d| d.child(ui::busy("measuring", "measuring").text_size(px(11.))))
            .child(div().flex_1())
            .when(!ticked.is_empty(), |d| {
                let label = format!("Remove {}…", ticked.len());
                d.child(
                    ui::danger_button("remove-ticked", label).on_click(
                        cx.listener(move |_, _: &ClickEvent, window, cx| Self::ask_remove(&ticked, window, cx)),
                    ),
                )
            })
    }

    fn list(&self, worktrees: &[Worktree], cx: &mut Context<Self>) -> Vec<AnyElement> {
        let filter = self.filter.read(cx).text().clone();
        let indexes = shown(worktrees, &filter, self.order);
        if indexes.is_empty() {
            let text = if worktrees.is_empty() { NO_WORKTREES } else { NO_MATCH };
            return vec![ui::note(text).into_any_element()];
        }
        let now = now();
        indexes.into_iter().map(|i| self.row(&worktrees[i], now, cx)).collect()
    }

    fn body(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        match (&self.worktrees, &self.error) {
            (_, Some(e)) => vec![ui::note(e.clone()).text_color(colours::failed()).into_any_element()],
            (None, None) => vec![ui::loading("Looking for worktrees…").into_any_element()],
            (Some(worktrees), None) => {
                let mut body = self.list(worktrees, cx);
                body.push(self.footer(worktrees, cx).into_any_element());
                body
            }
        }
    }
}

impl Render for WorktreesPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let toolbar = self.toolbar(window, cx);
        let scope = self.scope_line(cx);
        let body = self.body(cx);
        div()
            .id("worktrees")
            .size_full()
            .overflow_y_scroll()
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(toolbar)
            .children(scope)
            .children(body)
            .child(ui::dim(CLOSED_NOTE))
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| WorktreesPanel::new(window, cx)).into()
}

#[cfg(test)]
mod tests;
