use gpui::{
    AnyElement, ClickEvent, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px,
};

use crate::model::Session;
use crate::store::Store;
use crate::theme::{FONT_FAMILY, colours, radius};
use crate::ui;

use super::pieces::plural;
use super::selection::switching_base;
use super::wire::{Base, Mode, State};
use super::{ChangesPanel, Scope};

struct Totals {
    files: usize,
    added: usize,
    removed: usize,
}

fn chosen_dot(chosen: bool) -> impl IntoElement {
    div().w(px(12.)).flex_none().text_size(px(10.)).text_color(colours::accent()).child(if chosen { "●" } else { "" })
}

fn totals_view(totals: &Totals) -> impl IntoElement + use<> {
    div()
        .flex()
        .flex_none()
        .gap(px(6.))
        .child(div().text_color(colours::muted()).child(plural(totals.files, "file")))
        .child(div().text_color(colours::done()).child(format!("+{}", totals.added)))
        .child(div().text_color(colours::failed()).child(format!("−{}", totals.removed)))
}

fn place_label(scope: Scope, shown: Option<u64>, session: Option<&Session>) -> String {
    match (scope, shown.and_then(|w| session?.workspace(w))) {
        (Scope::Every, _) => "uncommitted changes of every project".to_string(),
        (_, Some((p, w))) if p.workspaces.len() > 1 => format!("{} › {}", p.name, w.name),
        (_, Some((p, _))) => p.name.clone(),
        (_, None) => String::new(),
    }
}

fn default_label(base: &Base) -> String {
    match &base.default {
        Some(d) => format!("The default branch ({d})"),
        None => "The default branch".to_string(),
    }
}

fn branch_name(branch: &str) -> impl IntoElement + use<> {
    div().flex_1().min_w(px(0.)).font_family(FONT_FAMILY).text_size(px(12.)).child(ui::one_line(branch.to_string()))
}

impl ChangesPanel {
    pub(super) fn render_header(&self, session: Option<&Session>, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(10.))
            .px(px(12.))
            .pt(px(12.))
            .pb(px(10.))
            .border_b_1()
            .border_color(colours::border())
            .child(self.scope_tabs(cx))
            .child(self.summary_line(session, cx))
            .when(self.base_open, |d| d.child(self.render_bases(cx)))
    }

    fn scope_tab(&self, id: &'static str, label: &'static str, scope: Scope, cx: &mut Context<Self>) -> AnyElement {
        ui::segment(id, label, scope == self.scope)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_scope(scope, cx)))
            .into_any_element()
    }

    fn scope_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        ui::segments(vec![
            self.scope_tab("mode-uncommitted", "Uncommitted", Scope::Project(Mode::Uncommitted), cx),
            self.scope_tab("mode-commits", "Commits", Scope::Project(Mode::Commits), cx),
            self.scope_tab("mode-all", "All", Scope::Project(Mode::All), cx),
            self.scope_tab("mode-every", "Every project", Scope::Every, cx),
        ])
    }

    fn totals(&self) -> Option<Totals> {
        let summary = self.summary.as_ref().filter(|s| s.state == State::Ready)?;
        Some(Totals {
            files: summary.workspaces.iter().map(|w| w.files.len()).sum(),
            added: summary.workspaces.iter().map(|w| w.added).sum(),
            removed: summary.workspaces.iter().map(|w| w.removed).sum(),
        })
    }

    fn base_label(&self) -> Option<String> {
        if !matches!(self.scope, Scope::Project(Mode::Commits | Mode::All)) {
            return None;
        }
        let base = self.summary.as_ref().and_then(|s| s.workspaces.first()).and_then(|w| w.base.clone());
        Some(format!("vs {} ▾", base.unwrap_or_else(|| "…".into())))
    }

    fn base_button(&self, label: String, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let workspace = self.follow.and_then(|f| f.workspace);
        if workspace.is_some_and(|w| Store::get(cx).read(cx).busy(&switching_base(w))) {
            return ui::busy_button("base", "switching…", ui::Tone::Plain).py(px(1.)).text_size(px(11.));
        }
        ui::button("base", label)
            .py(px(1.))
            .text_size(px(11.))
            .when(self.base_open, |b| b.border_color(colours::accent()))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_base(cx)))
    }

    fn summary_line(&self, session: Option<&Session>, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let place = place_label(self.scope, self.shown, session);
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .text_size(px(11.))
            .child(div().flex_1().min_w(px(0.)).text_color(colours::muted()).child(ui::one_line(place)))
            .when_some(self.totals(), |d, totals| d.child(totals_view(&totals)))
            .when_some(self.base_label(), |d, label| d.child(self.base_button(label, cx)))
    }

    fn render_bases(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let Some(base) = &self.base else {
            return div().child(ui::busy("bases-loading", "reading the branches…").text_size(px(11.)));
        };
        let default = ui::row("base-default", false)
            .py(px(3.))
            .child(chosen_dot(base.chosen.is_none()))
            .child(div().flex_1().child(default_label(base)))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.choose_base(None, cx)));
        let branches: Vec<_> =
            base.branches.iter().enumerate().map(|(i, branch)| Self::base_row(base, i, branch, cx)).collect();
        div().child(
            div()
                .id("bases")
                .max_h(px(220.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .p(px(4.))
                .rounded(px(radius::CONTROL))
                .bg(colours::panel())
                .border_1()
                .border_color(colours::border())
                .child(default)
                .children(branches),
        )
    }

    fn base_row(base: &Base, i: usize, branch: &str, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let chosen = base.chosen.as_deref() == Some(branch);
        let name = branch.to_string();
        ui::row(SharedString::from(format!("base-{i}")), false)
            .py(px(3.))
            .child(chosen_dot(chosen))
            .child(branch_name(branch))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.choose_base(Some(name.clone()), cx)))
    }
}
