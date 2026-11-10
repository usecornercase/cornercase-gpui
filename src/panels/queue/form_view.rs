use gpui::{
    AnyElement, App, ClickEvent, Context, Div, ElementId, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};
use serde_json::Value;

use super::QueuePanel;
use super::data::{ModeChip, ROUTINES_NOTE, SOURCES, agent_choices, is_repository, mode_chips, sentence};
use super::draft::When;
use super::form::{Form, Spot};
use super::repeat::{self, DAYS, UNITS};
use crate::model::Session;
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, Tone, boxed, glass};

const TRUST: &str = "Unattended: it accepts the folder's trust prompt and submits the prompt by itself.";
const EVERY_HINT: &str = "At least 30 minutes apart, at most a year.";
const FLOW_TASK: &str = "The prompt is the team's task.";
const FLOW_MODES: &str = "Set by the flow's roles.";
const NO_REPOSITORY: &str =
    "Open a git repository's folder as a project first: only git repositories can have routines.";
const FOLLOWS: &str = "Left as it is, it starts on whichever account is in use at that time.";
const CLEANUP: &str = "remove the workspace when it finishes";
const CLEANUP_NOTE: &str = "Once the agent has been done for 3 minutes, the run's workspace and branch go. The \
                            workspace stays, and the run says why, when anything would be lost: uncommitted or \
                            unpushed work, or a lock. Looking at the run's tab, or a server restart, keeps it too.";
const CLEANUP_WAITS: &str = "An agent cornercase cannot read is refused on saving; knowing which beforehand needs \
                             usecornercase/cornercase-terminal#704.";

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

fn prompt(form: &Form, window: &Window, cx: &App) -> Div {
    ui::prompt_box(&form.prompt, if form.editing() { "save" } else { "schedule" }, window, cx)
}

fn what(form: &Form, window: &Window, cx: &mut Context<QueuePanel>) -> impl IntoElement + use<> {
    if form.repeats() {
        let note = form.runs_flow().then(|| ui::dim(FLOW_TASK));
        return section(Icon::Message, "What", stack().child(prompt(form, window, cx)).children(note));
    }
    let kinds = ui::segments(vec![
        seg("q-what-prompt", "Prompt", !form.issue, cx, |f| f.issue = false),
        seg("q-what-issue", "Issue", form.issue, cx, |f| f.issue = true),
    ]);
    let body = if form.issue { issue(form, window, cx) } else { prompt(form, window, cx).into_any_element() };
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

fn when_seg(form: &Form, kind: When, cx: &mut Context<QueuePanel>) -> AnyElement {
    let segment = ui::segment(("q-when", kind as usize), kind.label(), form.when == kind).px(px(9.));
    pick(segment, cx, move |f| f.choose_when(kind))
}

fn when_track(form: &Form, kinds: &[When], cx: &mut Context<QueuePanel>) -> Div {
    let items = kinds.iter().map(|kind| when_seg(form, *kind, cx)).collect();
    hug(ui::segments(items).flex_wrap())
}

fn when(form: &Form, window: &Window, cx: &mut Context<QueuePanel>) -> impl IntoElement + use<> {
    let offered = When::offered(form.editing(), form.old.repeat.is_some());
    let tracks: Vec<Div> = offered.iter().map(|kinds| when_track(form, kinds, cx)).collect();
    section(Icon::Queue, "When", stack().children(tracks).children(when_values(form, window, cx)))
}

fn when_values(form: &Form, window: &Window, cx: &mut Context<QueuePanel>) -> Option<AnyElement> {
    let values = match form.when {
        When::At | When::Tomorrow | When::Daily | When::Weekdays => clock_box(form, window, cx),
        When::On => div()
            .flex()
            .gap(px(8.))
            .child(div().w(px(130.)).child(boxed(&form.date, window, cx)))
            .child(div().w(px(90.)).child(boxed(&form.clock, window, cx))),
        When::In => amount_and_units(form, window, cx),
        When::Every => stack().child(amount_and_units(form, window, cx)).child(ui::dim(EVERY_HINT)),
        When::Weekly => stack().child(hug(weekdays(form, cx))).child(clock_box(form, window, cx)),
        When::ClaudeReset | When::CodexReset => ui::dim("When the plan's 5-hour session resets, read once now."),
        When::Keep => ui::dim(format!("↻ {}", repeat::words(form.old.repeat.as_deref()?))),
    };
    Some(values.into_any_element())
}

fn clock_box(form: &Form, window: &Window, cx: &mut Context<QueuePanel>) -> Div {
    div().w(px(110.)).child(boxed(&form.clock, window, cx))
}

fn amount_and_units(form: &Form, window: &Window, cx: &mut Context<QueuePanel>) -> Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(8.))
        .child(div().w(px(70.)).child(boxed(&form.amount, window, cx)))
        .child(units(form, cx))
}

fn units(form: &Form, cx: &mut Context<QueuePanel>) -> Div {
    ui::segments(
        UNITS
            .into_iter()
            .map(|(unit, label)| seg(("q-unit", u32::from(unit)), label, form.unit == unit, cx, move |f| f.unit = unit))
            .collect(),
    )
}

fn weekdays(form: &Form, cx: &mut Context<QueuePanel>) -> Div {
    ui::segments(
        DAYS.into_iter()
            .enumerate()
            .map(|(at, day)| {
                let label = sentence(day.get(..3).unwrap_or(day));
                seg(("q-day", at), label, form.weekday == day, cx, move |f| f.weekday = day)
            })
            .collect(),
    )
    .flex_wrap()
}

fn choose_project(form: &mut Form, id: u64) {
    form.project = Some(id);
    form.spot = Spot::NewWorktree;
}

fn choose_agent(form: &mut Form, agent: Option<String>) {
    form.agent = agent;
    form.mode = None;
    form.flow = None;
}

fn choose_flow(form: &mut Form, flow: String) {
    form.flow = Some(flow);
    form.agent = None;
    form.mode = None;
}

fn keep_or_move(form: &Form, cx: &mut Context<QueuePanel>) -> Div {
    hug(ui::segments(vec![
        seg("q-place-keep", "unchanged", form.spot == Spot::Keep, cx, |f| f.spot = Spot::Keep),
        seg("q-place-change", "somewhere else", form.spot != Spot::Keep, cx, |f| f.spot = Spot::NewWorktree),
    ]))
}

fn place(form: &Form, session: Option<&Session>, window: &Window, cx: &mut Context<QueuePanel>) -> Div {
    if form.repeats() {
        return routine_place(form, session, cx);
    }
    let mut place = stack();
    if form.editing() {
        place = place.child(keep_or_move(form, cx));
    }
    if form.spot == Spot::Keep {
        return place;
    }
    place = place.child(ui::field("Project", projects(form, session, |_| true, cx)));
    let Some(project) = form.project else { return place };
    place = place.child(ui::field("A new tab in", workspaces(form, project, session, cx)));
    place.when(form.spot == Spot::NewWorktree, |d| {
        d.child(ui::field("Branch, made when missing", boxed(&form.branch, window, cx)))
    })
}

fn routine_place(form: &Form, session: Option<&Session>, cx: &mut Context<QueuePanel>) -> Div {
    let mut place = stack();
    if form.editing() {
        place = place.child(keep_or_move(form, cx));
    }
    if form.spot != Spot::Keep {
        place = place.child(repositories(form, session, cx));
    }
    let new_worktree = div().text_size(px(12.)).text_color(colours::muted()).child("new worktree");
    place.child(ui::field("Open in", new_worktree)).child(cleanup(form, cx)).child(ui::dim(ROUTINES_NOTE))
}

fn repositories(form: &Form, session: Option<&Session>, cx: &mut Context<QueuePanel>) -> Div {
    if session.is_none_or(|s| !s.projects.iter().any(is_repository)) {
        return ui::dim(NO_REPOSITORY);
    }
    ui::field("Project", projects(form, session, is_repository, cx))
}

fn cleanup(form: &Form, cx: &mut Context<QueuePanel>) -> Div {
    let checkbox = ui::checkbox("q-cleanup", form.cleanup, CLEANUP).text_size(px(12.5));
    stack()
        .gap(px(4.))
        .child(pick(checkbox, cx, |f| f.cleanup = !f.cleanup))
        .child(ui::dim(CLEANUP_NOTE))
        .child(ui::dim(CLEANUP_WAITS))
}

fn projects(
    form: &Form,
    session: Option<&Session>,
    shown: impl Fn(&crate::model::Project) -> bool,
    cx: &mut Context<QueuePanel>,
) -> Div {
    let projects: Vec<(u64, String)> = session
        .map(|s| s.projects.iter().filter(|p| shown(p)).map(|p| (p.id, p.name.clone())).collect())
        .unwrap_or_default();
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

fn agents(form: &Form, config: &Value, flows: Option<&[String]>, cx: &mut Context<QueuePanel>) -> Div {
    let (agents, default_agent) = agent_choices(config);
    let editing = form.editing();
    let mut items = Vec::new();
    if editing {
        let keep = form.agent.is_none() && form.flow.is_none();
        items.push(chip("q-agent-keep", "unchanged", keep, cx, |f| choose_agent(f, None)));
    }
    for agent in agents {
        let selected = match &form.agent {
            Some(a) => *a == agent,
            None => !editing && form.flow.is_none() && agent == default_agent,
        };
        let id = SharedString::from(format!("q-agent-{agent}"));
        let value = agent.clone();
        items.push(chip(id, agent, selected, cx, move |f| choose_agent(f, Some(value.clone()))));
    }
    if form.repeats() {
        items.extend(flow_chips(form, flows, cx));
    }
    chips(items)
}

fn flow_chips(form: &Form, flows: Option<&[String]>, cx: &mut Context<QueuePanel>) -> Vec<AnyElement> {
    let Some(flows) = flows else { return vec![ui::busy("flows-loading", "Loading flows…").into_any_element()] };
    let mut items = Vec::new();
    for name in flows {
        let id = SharedString::from(format!("q-flow-{name}"));
        let selected = form.flow.as_ref() == Some(name);
        let value = name.clone();
        items.push(chip(id, format!("{name} (flow)"), selected, cx, move |f| choose_flow(f, value.clone())));
    }
    items
}

fn modes(form: &Form, config: &Value, cx: &mut Context<QueuePanel>) -> Option<Div> {
    if form.runs_flow() {
        return Some(ui::field("Mode", ui::dim(FLOW_MODES)));
    }
    let agent = form.agent.clone().or_else(|| (!form.editing()).then(|| agent_choices(config).1))?;
    if agent == "auto" {
        return None;
    }
    let server = Store::get(cx).read(cx).session.as_ref().and_then(Session::release);
    let (offered, line) = mode_chips(config, &agent, server);
    if offered.is_empty() {
        return None;
    }
    let mut items = vec![chip("q-mode-settings", "as in settings", form.mode.is_none(), cx, |f| f.mode = None)];
    items.extend(offered.into_iter().enumerate().map(|(at, offer)| mode_chip(form, at, offer, cx)));
    Some(ui::field("Mode", stack().child(chips(items)).children(line.map(ui::dim))))
}

fn mode_chip(form: &Form, at: usize, offer: ModeChip, cx: &mut Context<QueuePanel>) -> AnyElement {
    let selected = form.mode.as_deref() == Some(offer.value.as_str());
    if offer.rests {
        let edge = if selected { colours::accent() } else { colours::border() };
        return ui::resting_segment(("q-mode", at), offer.label, selected)
            .border_1()
            .border_color(edge)
            .into_any_element();
    }
    let value = offer.value;
    chip(("q-mode", at), offer.label, selected, cx, move |f| f.mode = Some(value.clone()))
}

fn account_row(form: &Form, cx: &App) -> Option<Div> {
    let default = Store::get(cx).read(cx).account_settings.default_agent.clone();
    let agents = form.account_agents(default.as_deref());
    let follows = form.accounts.follow_in_use(&agents, cx);
    let fields = form.accounts.fields(&agents, cx)?;
    Some(stack().gap(px(6.)).child(fields).when(follows, |d| d.child(ui::dim(FOLLOWS))))
}

fn agent_section(form: &Form, config: Option<&Value>, flows: Option<&[String]>, cx: &mut Context<QueuePanel>) -> Div {
    let Some(config) = config else { return section(Icon::Agent, "Agent", ui::busy("agents-loading", "Loading…")) };
    let controls = stack().child(agents(form, config, flows, cx)).children(account_row(form, cx));
    section(Icon::Agent, "Agent", controls.children(modes(form, config, cx)))
}

fn header_text(editing: bool, repeats: bool) -> (&'static str, &'static str) {
    match (editing, repeats) {
        (true, true) => ("Edit routine", "Only what you change is sent, and only the runs to come follow it."),
        (true, false) => ("Edit queue entry", "Only what you change is sent; the rest stays as it was."),
        (false, true) => ("Schedule a routine", "It starts by itself every time, until you pause or remove it."),
        (false, false) => ("Schedule an agent", "It starts by itself at the time you choose."),
    }
}

fn header(form: &Form) -> Div {
    let (title, line) = header_text(form.editing(), form.repeats());
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
        .rounded(px(radius::control()))
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
        .rounded_br(px(radius::sheet() - 1.))
        .bg(colours::inset())
}

impl QueuePanel {
    pub(super) fn form_view(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let form = self.form.as_ref()?;
        let session = Store::get(cx).read(cx).session.clone();
        let flows = self.flows.names.as_deref();
        let fields = div()
            .px(px(20.))
            .pb(px(18.))
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(what(form, window, cx))
            .child(when(form, window, cx))
            .child(section(Icon::Branch, "Where", place(form, session.as_ref(), window, cx)))
            .child(agent_section(form, self.config.as_ref(), flows, cx))
            .child(section(Icon::Pencil, "Name", boxed(&form.name, window, cx)));
        let body =
            div().id("queue-form").flex_1().min_h_0().overflow_y_scroll().track_scroll(&form.scroller).child(fields);
        Some(frame(self.form_width.size()).child(header(form)).child(body).child(footer(form, cx)).into_any_element())
    }
}
