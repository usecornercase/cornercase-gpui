use gpui::{
    AnyElement, ClickEvent, Context, Div, ElementId, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};
use serde_json::Value;

use super::QueuePanel;
use super::data::{SOURCES, agent_choices, mode_choices, sentence};
use super::draft::When;
use super::form::{Form, Spot};
use crate::model::Session;
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, Tone, boxed, glass};

const TRUST: &str = "Unattended: it accepts the folder's trust prompt and submits the prompt by itself.";

fn pick(el: Stateful<Div>, cx: &mut Context<QueuePanel>, change: impl Fn(&mut Form) + 'static) -> AnyElement {
    el.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.update_form(&change, cx))).into_any_element()
}

fn seg(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    cx: &mut Context<QueuePanel>,
    change: impl Fn(&mut Form) + 'static,
) -> AnyElement {
    pick(ui::segment(id, label, selected), cx, change)
}

fn chip(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    cx: &mut Context<QueuePanel>,
    change: impl Fn(&mut Form) + 'static,
) -> AnyElement {
    let edge = if selected { colours::accent() } else { colours::border() };
    pick(ui::segment(id, label, selected).border_1().border_color(edge), cx, change)
}

fn chips(items: Vec<AnyElement>) -> Div {
    div().flex().flex_wrap().gap(px(4.)).children(items)
}

fn stack() -> Div {
    div().flex().flex_col().gap(px(8.))
}

fn hug(control: Div) -> Div {
    div().flex().child(control)
}

fn section(glyph: Icon, title: &'static str, control: impl IntoElement) -> Div {
    let head = div()
        .flex()
        .items_center()
        .gap(px(7.))
        .text_size(px(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colours::soft())
        .child(ui::icon(glyph, 13.).text_color(colours::accent()))
        .child(title);
    stack().child(head).child(control)
}

fn what(form: &Form, window: &Window, cx: &mut Context<QueuePanel>) -> impl IntoElement + use<> {
    let kinds = ui::segments(vec![
        seg("q-what-prompt", "Prompt", !form.issue, cx, |f| f.issue = false),
        seg("q-what-issue", "Issue", form.issue, cx, |f| f.issue = true),
    ]);
    let body = if form.issue { issue(form, window, cx) } else { boxed(&form.prompt, window, cx).into_any_element() };
    section(Icon::Message, "What", stack().child(hug(kinds)).child(body))
}

fn issue(form: &Form, window: &Window, cx: &mut Context<QueuePanel>) -> AnyElement {
    let mut sources = vec![chip("q-src-auto", "auto", form.source.is_none(), cx, |f| f.source = None)];
    for source in SOURCES {
        let id = SharedString::from(format!("q-src-{source}"));
        let selected = form.source.as_deref() == Some(source);
        sources.push(chip(id, source, selected, cx, move |f| f.source = Some(source.to_string())));
    }
    stack()
        .child(boxed(&form.issue_key, window, cx))
        .child(ui::field("Tracker, for an ambiguous key or a self-hosted URL", chips(sources)))
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
    let kinds = hug(ui::segments(kinds).flex_wrap());
    section(Icon::Queue, "When", stack().child(kinds).children(when_values(form, window, cx)))
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
        When::ClaudeReset | When::CodexReset => ui::dim("When the plan's 5-hour session resets, read once now."),
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
        place = place.child(hug(ui::segments(vec![
            seg("q-place-keep", "unchanged", form.spot == Spot::Keep, cx, |f| f.spot = Spot::Keep),
            seg("q-place-change", "somewhere else", form.spot != Spot::Keep, cx, |f| f.spot = Spot::NewWorktree),
        ])));
    }
    if form.spot == Spot::Keep {
        return place;
    }
    place = place.child(ui::field("Project", projects(form, session, cx)));
    let Some(project) = form.project else { return place };
    place = place.child(ui::field("A new tab in", workspaces(form, project, session, cx)));
    place.when(form.spot == Spot::NewWorktree, |d| {
        d.child(ui::field("Branch, made when missing", boxed(&form.branch, window, cx)))
    })
}

fn projects(form: &Form, session: Option<&Session>, cx: &mut Context<QueuePanel>) -> Div {
    let projects: Vec<(u64, String)> =
        session.map(|s| s.projects.iter().map(|p| (p.id, p.name.clone())).collect()).unwrap_or_default();
    chips(
        projects
            .into_iter()
            .map(|(id, name)| {
                chip(("q-project", id), name, form.project == Some(id), cx, move |f| choose_project(f, id))
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
            chip(("q-ws", id), name, form.spot == Spot::Workspace(id), cx, move |f| f.spot = Spot::Workspace(id))
        })
        .collect();
    let new = form.spot == Spot::NewWorktree;
    choices.push(chip("q-ws-new", "+ new worktree", new, cx, |f| f.spot = Spot::NewWorktree));
    chips(choices)
}

fn agents(form: &Form, config: &Value, cx: &mut Context<QueuePanel>) -> Div {
    let (agents, default_agent) = agent_choices(config);
    let editing = form.editing();
    let mut items = Vec::new();
    if editing {
        items.push(chip("q-agent-keep", "unchanged", form.agent.is_none(), cx, |f| choose_agent(f, None)));
    }
    for agent in agents {
        let selected = match &form.agent {
            Some(a) => *a == agent,
            None => !editing && agent == default_agent,
        };
        let id = SharedString::from(format!("q-agent-{agent}"));
        let value = agent.clone();
        items.push(chip(id, agent, selected, cx, move |f| choose_agent(f, Some(value.clone()))));
    }
    chips(items)
}

fn modes(form: &Form, config: &Value, cx: &mut Context<QueuePanel>) -> Option<Div> {
    let agent = form.agent.clone().or_else(|| (!form.editing()).then(|| agent_choices(config).1))?;
    if agent == "auto" {
        return None;
    }
    let (modes, chosen) = mode_choices(config, &agent);
    if modes.is_empty() {
        return None;
    }
    let mut items = vec![chip("q-mode-default", "as in settings", form.mode.is_none(), cx, |f| f.mode = None)];
    for mode in modes {
        let label = if Some(&mode) == chosen.as_ref() { format!("{mode} ✓") } else { mode.clone() };
        let id = SharedString::from(format!("q-mode-{mode}"));
        let selected = form.mode.as_ref() == Some(&mode);
        items.push(chip(id, label, selected, cx, move |f| f.mode = Some(mode.clone())));
    }
    Some(ui::field("Mode", chips(items)))
}

fn agent_section(form: &Form, config: Option<&Value>, cx: &mut Context<QueuePanel>) -> Div {
    let Some(config) = config else { return section(Icon::Agent, "Agent", ui::busy("agents-loading", "Loading…")) };
    section(Icon::Agent, "Agent", stack().child(agents(form, config, cx)).children(modes(form, config, cx)))
}

fn header(editing: bool) -> Div {
    let (title, line) = if editing {
        ("Edit queue entry", "Only what you change is sent; the rest stays as it was.")
    } else {
        ("Schedule an agent", "It starts by itself at the time you choose.")
    };
    div()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(3.))
        .px(px(20.))
        .pt(px(18.))
        .pb(px(14.))
        .child(ui::title(title).text_size(px(15.)))
        .child(div().text_size(px(12.)).text_color(colours::muted()).child(line))
}

fn trust_note() -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(8.))
        .text_size(px(11.5))
        .text_color(colours::muted())
        .child(ui::icon(Icon::Agent, 13.).text_color(colours::muted()).mt(px(1.)))
        .child(div().flex_1().min_w(px(0.)).child(TRUST))
}

fn error_note(text: String) -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(8.))
        .bg(glass::tint(colours::failed(), 0.08))
        .rounded(px(radius::CONTROL))
        .px(px(10.))
        .py(px(7.))
        .text_size(px(12.))
        .text_color(colours::failed())
        .child(ui::icon(Icon::Alert, 13.).text_color(colours::failed()).mt(px(1.)))
        .child(div().flex_1().min_w(px(0.)).child(text))
}

fn save_button(form: &Form, cx: &mut Context<QueuePanel>) -> Stateful<Div> {
    match (form.busy, form.editing()) {
        (true, true) => ui::busy_button("q-save", "Saving…", Tone::Primary),
        (true, false) => ui::busy_button("q-save", "Scheduling…", Tone::Primary),
        (false, editing) => {
            let (glyph, label) = if editing { (Icon::Check, "Save") } else { (Icon::Queue, "Schedule") };
            ui::icon_text_button("q-save", glyph, label, Tone::Primary)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.save(cx)))
        }
    }
}

fn footer(form: &Form, cx: &mut Context<QueuePanel>) -> Div {
    let cancel =
        ui::button("q-cancel", "Cancel").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_form(cx)));
    div()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(12.))
        .px(px(20.))
        .py(px(14.))
        .border_t_1()
        .border_color(colours::border())
        .child(trust_note())
        .children(form.error.as_deref().map(|e| error_note(sentence(e))))
        .child(div().flex().justify_end().gap(px(8.)).child(cancel).child(save_button(form, cx)))
}

fn frame(width: f32) -> Div {
    div()
        .w(px(width))
        .flex_none()
        .h_full()
        .flex()
        .flex_col()
        .border_l_1()
        .border_color(colours::border())
        .rounded_br(px(radius::SHEET - 1.))
        .bg(colours::sidebar())
}

impl QueuePanel {
    pub(super) fn form_view(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let form = self.form.as_ref()?;
        let session = Store::get(cx).read(cx).session.clone();
        let fields = div()
            .px(px(20.))
            .pb(px(18.))
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(what(form, window, cx))
            .child(when(form, window, cx))
            .child(section(Icon::Branch, "Where", place(form, session.as_ref(), window, cx)))
            .child(agent_section(form, self.config.as_ref(), cx))
            .child(section(Icon::Pencil, "Name", boxed(&form.name, window, cx)));
        let body = div().id("queue-form").flex_1().min_h_0().overflow_y_scroll().child(fields);
        Some(
            frame(self.form_width.size())
                .child(header(form.editing()))
                .child(body)
                .child(footer(form, cx))
                .into_any_element(),
        )
    }
}
