use gpui::{
    AnyElement, App, ClickEvent, Context, Div, ElementId, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};
use serde_json::Value;

use super::QueuePanel;
use super::data::{SOURCES, agent_choices, mode_choices};
use super::draft::When;
use super::form::{Form, Spot};
use crate::model::Session;
use crate::store::Store;
use crate::theme::colours;
use crate::ui::{self, TextField};

fn seg(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    cx: &mut Context<QueuePanel>,
    change: impl Fn(&mut Form) + 'static,
) -> AnyElement {
    ui::segment(id, label, selected)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.update_form(&change, cx)))
        .into_any_element()
}

fn wrap_row(items: Vec<AnyElement>) -> Div {
    ui::segments(items).flex_wrap()
}

fn boxed(field: &Entity<TextField>, window: &Window, cx: &App) -> Div {
    ui::input_box(field.clone(), field.read(cx).focus_handle(cx).is_focused(window))
}

fn stack() -> Div {
    div().flex().flex_col().gap(px(8.))
}

fn what(form: &Form, window: &Window, cx: &mut Context<QueuePanel>) -> impl IntoElement + use<> {
    let kinds = ui::segments(vec![
        seg("q-what-prompt", "Prompt", !form.issue, cx, |f| f.issue = false),
        seg("q-what-issue", "Issue", form.issue, cx, |f| f.issue = true),
    ]);
    let body = if form.issue { issue(form, window, cx) } else { boxed(&form.prompt, window, cx).into_any_element() };
    ui::field("What", stack().child(kinds).child(body))
}

fn issue(form: &Form, window: &Window, cx: &mut Context<QueuePanel>) -> AnyElement {
    let mut sources = vec![seg("q-src-auto", "auto", form.source.is_none(), cx, |f| f.source = None)];
    for source in SOURCES {
        let id = SharedString::from(format!("q-src-{source}"));
        let selected = form.source.as_deref() == Some(source);
        sources.push(seg(id, source, selected, cx, move |f| f.source = Some(source.to_string())));
    }
    stack()
        .child(boxed(&form.issue_key, window, cx))
        .child(ui::field("Tracker, for an ambiguous key or a self-hosted URL", wrap_row(sources)))
        .into_any_element()
}

fn when(form: &Form, window: &Window, cx: &mut Context<QueuePanel>) -> impl IntoElement + use<> {
    let mut kinds = Vec::new();
    if form.editing() {
        kinds.push(seg("q-when-keep", When::Keep.label(), form.when == When::Keep, cx, |f| f.when = When::Keep));
    }
    for kind in When::NEW {
        kinds.push(seg(("q-when", kind as usize), kind.label(), form.when == kind, cx, move |f| f.when = kind));
    }
    ui::field("When", stack().child(wrap_row(kinds)).children(when_values(form, window, cx)))
}

fn when_values(form: &Form, window: &Window, cx: &mut Context<QueuePanel>) -> Option<AnyElement> {
    let values = match form.when {
        When::At | When::Tomorrow => div().w(px(110.)).child(boxed(&form.clock, window, cx)),
        When::On => div()
            .flex()
            .gap(px(8.))
            .child(div().w(px(130.)).child(boxed(&form.date, window, cx)))
            .child(div().w(px(90.)).child(boxed(&form.clock, window, cx))),
        When::In => div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().w(px(70.)).child(boxed(&form.amount, window, cx)))
            .child(units(form, cx)),
        When::ClaudeReset | When::CodexReset => ui::dim("when the plan's 5-hour session resets, read once now"),
        When::Keep => return None,
    };
    Some(values.into_any_element())
}

fn units(form: &Form, cx: &mut Context<QueuePanel>) -> Div {
    ui::segments(
        [('m', "minutes"), ('h', "hours"), ('d', "days")]
            .into_iter()
            .map(|(unit, label)| seg(("q-unit", u32::from(unit)), label, form.unit == unit, cx, move |f| f.unit = unit))
            .collect(),
    )
}

fn choose_project(form: &mut Form, id: u64) {
    form.project = Some(id);
    form.spot = Spot::NewWorktree;
}

fn choose_agent(form: &mut Form, agent: Option<String>) {
    form.agent = agent;
    form.mode = None;
}

fn place(form: &Form, session: Option<&Session>, window: &Window, cx: &mut Context<QueuePanel>) -> Div {
    let mut place = stack();
    if form.editing() {
        place = place.child(ui::segments(vec![
            seg("q-place-keep", "unchanged", form.spot == Spot::Keep, cx, |f| f.spot = Spot::Keep),
            seg("q-place-change", "somewhere else", form.spot != Spot::Keep, cx, |f| f.spot = Spot::NewWorktree),
        ]));
    }
    if form.spot == Spot::Keep {
        return place;
    }
    place = place.child(projects(form, session, cx));
    let Some(project) = form.project else { return place };
    place = place.child(ui::field("A new tab in", workspaces(form, project, session, cx)));
    place.when(form.spot == Spot::NewWorktree, |d| {
        d.child(ui::field("Branch, made when missing", boxed(&form.branch, window, cx)))
    })
}

fn projects(form: &Form, session: Option<&Session>, cx: &mut Context<QueuePanel>) -> Div {
    let projects: Vec<(u64, String)> =
        session.map(|s| s.projects.iter().map(|p| (p.id, p.name.clone())).collect()).unwrap_or_default();
    wrap_row(
        projects
            .into_iter()
            .map(|(id, name)| {
                seg(("q-project", id), name, form.project == Some(id), cx, move |f| choose_project(f, id))
            })
            .collect(),
    )
}

fn workspaces(form: &Form, project: u64, session: Option<&Session>, cx: &mut Context<QueuePanel>) -> Div {
    let workspaces: Vec<(u64, String)> = session
        .and_then(|s| s.projects.iter().find(|p| p.id == project))
        .map(|p| p.workspaces.iter().map(|w| (w.id, w.name.clone())).collect())
        .unwrap_or_default();
    let mut choices: Vec<AnyElement> = workspaces
        .into_iter()
        .map(|(id, name)| {
            seg(("q-ws", id), name, form.spot == Spot::Workspace(id), cx, move |f| f.spot = Spot::Workspace(id))
        })
        .collect();
    let new = form.spot == Spot::NewWorktree;
    choices.push(seg("q-ws-new", "+ new worktree", new, cx, |f| f.spot = Spot::NewWorktree));
    wrap_row(choices)
}

fn agents(form: &Form, config: Option<&Value>, cx: &mut Context<QueuePanel>) -> impl IntoElement + use<> {
    let Some(config) = config else { return ui::field("Agent", ui::dim("Loading…")) };
    let (agents, default_agent) = agent_choices(config);
    let editing = form.editing();
    let mut segs = Vec::new();
    if editing {
        segs.push(seg("q-agent-keep", "unchanged", form.agent.is_none(), cx, |f| choose_agent(f, None)));
    }
    for agent in agents {
        let selected = match &form.agent {
            Some(a) => *a == agent,
            None => !editing && agent == default_agent,
        };
        let id = SharedString::from(format!("q-agent-{agent}"));
        let value = agent.clone();
        segs.push(seg(id, agent, selected, cx, move |f| choose_agent(f, Some(value.clone()))));
    }
    ui::field("Agent", wrap_row(segs))
}

fn modes(form: &Form, config: Option<&Value>, cx: &mut Context<QueuePanel>) -> Option<Div> {
    let config = config?;
    let agent = form.agent.clone().or_else(|| (!form.editing()).then(|| agent_choices(config).1))?;
    if agent == "auto" {
        return None;
    }
    let (modes, chosen) = mode_choices(config, &agent);
    if modes.is_empty() {
        return None;
    }
    let mut segs = vec![seg("q-mode-default", "as in settings", form.mode.is_none(), cx, |f| f.mode = None)];
    for mode in modes {
        let label = if Some(&mode) == chosen.as_ref() { format!("{mode} ✓") } else { mode.clone() };
        let id = SharedString::from(format!("q-mode-{mode}"));
        let selected = form.mode.as_ref() == Some(&mode);
        segs.push(seg(id, label, selected, cx, move |f| f.mode = Some(mode.clone())));
    }
    Some(ui::field("Mode", wrap_row(segs)))
}

fn buttons(form: &Form, cx: &mut Context<QueuePanel>) -> impl IntoElement + use<> {
    let save = match (form.busy, form.editing()) {
        (true, _) => "Saving…",
        (false, true) => "Save",
        (false, false) => "Schedule",
    };
    div()
        .flex()
        .justify_end()
        .gap(px(8.))
        .child(
            ui::button("q-cancel", "Cancel").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_form(cx))),
        )
        .child(ui::primary_button("q-save", save).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.save(cx))))
}

fn frame(editing: bool) -> gpui::Stateful<Div> {
    div()
        .id("queue-form")
        .w(px(420.))
        .flex_none()
        .h_full()
        .overflow_y_scroll()
        .border_l_1()
        .border_color(colours::border())
        .bg(colours::sidebar())
        .p(px(16.))
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(ui::title(if editing { "Edit queue entry" } else { "Schedule for later" }))
}

impl QueuePanel {
    pub(super) fn form_view(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let form = self.form.as_ref()?;
        let session = Store::get(cx).read(cx).session.clone();
        let config = self.config.as_ref();
        Some(
            frame(form.editing())
                .child(what(form, window, cx))
                .child(when(form, window, cx))
                .child(ui::field("Where", place(form, session.as_ref(), window, cx)))
                .child(agents(form, config, cx))
                .children(modes(form, config, cx))
                .child(ui::field("Name", boxed(&form.name, window, cx)))
                .child(ui::dim(
                    "Queued agents accept the folder's trust prompt, and the prompt is submitted by itself.",
                ))
                .when_some(form.error.clone(), |d, e| {
                    d.child(div().text_size(px(12.)).text_color(colours::failed()).child(e))
                })
                .child(buttons(form, cx))
                .into_any_element(),
        )
    }
}
