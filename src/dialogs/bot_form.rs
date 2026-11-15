use std::collections::BTreeMap;

use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Div, Entity, FocusHandle, Focusable, IntoElement,
    ParentElement as _, Render, ScrollHandle, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _,
    Window, div, prelude::FluentBuilder as _, px,
};

use super::{body_height, cancel_and, close, dialog_starts, fitted, frame, show, within};
use crate::model::bots::{AgentInfo, CORNERCASE, FULL, NotSent, Typed, agents_of, chosen_at_open, new_args};
use crate::store::{Store, args};
use crate::theme::colours;
use crate::ui::account_picker::AccountPickers;
use crate::ui::{self, TextField, TextFieldEvent, Tone};

const TITLE: &str = "New bot";
const READING: &str = "Reading the agents…";
const NO_AGENT: &str = "No agent here can be a bot yet.";
const CHOOSE_ACCESS: &str = "Choose its access.";
const THROUGH: &str = "Through cornercase";
const FULL_ACCESS: &str = "Full access";
const THROUGH_NOTE: &str = "Through cornercase, it reads anything and writes only in its own folder.";
const FULL_NOTE: &str = "Full access lets it change any file and run anything, unattended.";
const ABOUT_HINT: &str = "what it is for (optional)";
const OWN: &str = "the agent's own";

pub(crate) fn read_failed(words: &str) -> String {
    format!("Could not read the agents: {words}")
}

enum Listing {
    Reading,
    Read(Vec<AgentInfo>),
    Failed(String),
}

pub(crate) struct BotForm {
    listing: Listing,
    chosen: Option<usize>,
    name: Entity<TextField>,
    model: Entity<TextField>,
    about: Entity<TextField>,
    effort: Option<String>,
    access: Option<String>,
    accounts: AccountPickers,
    error: Option<&'static str>,
    scroller: ScrollHandle,
}

fn field(
    placeholder: &str,
    shape: impl FnOnce(TextField) -> TextField,
    cx: &mut Context<BotForm>,
    window: &mut Window,
) -> Entity<TextField> {
    let field = cx.new(|cx| shape(TextField::new(placeholder.to_string(), cx).in_tab_order()));
    cx.subscribe_in(&field, window, |this, _, event, _, cx| match event {
        TextFieldEvent::Submit => this.submit(cx),
        TextFieldEvent::Cancel => close(cx),
        TextFieldEvent::Changed => this.changed(cx),
    })
    .detach();
    field
}

pub(crate) fn new_bot(window: &mut Window, cx: &mut App) {
    if Store::get(cx).read(cx).session.is_none() {
        return;
    }
    let view = cx.new(|cx| {
        let scroller = ScrollHandle::new();
        let plain = || within(&scroller, |f| f);
        let name = field("PRs", plain(), cx, window);
        let model = field(OWN, plain(), cx, window);
        let about = field(ABOUT_HINT, within(&scroller, TextField::prompt), cx, window);
        about.update(cx, |about, _| about.set_ends_form(true));
        cx.observe(&Store::get(cx), |_, _, cx| cx.notify()).detach();
        let mut form = BotForm {
            listing: Listing::Reading,
            chosen: None,
            name,
            model,
            about,
            effort: None,
            access: None,
            accounts: AccountPickers::default().inside(scroller.clone()),
            error: None,
            scroller,
        };
        form.read_agents(cx);
        form
    });
    let focus = view.read(cx).name.read(cx).focus_handle(cx);
    show(view.into(), &focus, window, cx);
}

impl BotForm {
    fn read_agents(&mut self, cx: &mut Context<Self>) {
        self.listing = Listing::Reading;
        crate::store::run(cx, args(&["bot", "agents", "--json"]), Self::agents_read);
        cx.notify();
    }

    fn agents_read(&mut self, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        match result.and_then(|out| agents_of(&out)) {
            Ok(list) => {
                let default = Store::get(cx).read(cx).account_settings.default_agent.clone();
                self.chosen = chosen_at_open(&list, default.as_deref());
                let saved: BTreeMap<String, String> = list.iter().map(|a| (a.agent.clone(), String::new())).collect();
                self.accounts = AccountPickers::keeping(saved).inside(self.scroller.clone());
                self.listing = Listing::Read(list);
                self.fit_agent(cx);
            }
            Err(error) => self.listing = Listing::Failed(crate::cli::reason(&error)),
        }
        cx.notify();
    }

    fn agent(&self) -> Option<&AgentInfo> {
        match (&self.listing, self.chosen) {
            (Listing::Read(list), Some(at)) => list.get(at),
            _ => None,
        }
    }

    fn fit_agent(&mut self, cx: &mut Context<Self>) {
        let (hint, access) = self.agent().map(|a| (a.model_hint(), a.first_access())).unwrap_or_default();
        self.effort = None;
        self.access = access;
        self.error = None;
        self.model.update(cx, |model, cx| {
            model.clear(cx);
            model.set_placeholder(if hint.is_empty() { OWN.to_string() } else { hint }, cx);
        });
        Self::forget_refusal(cx);
    }

    fn forget_refusal(cx: &mut Context<Self>) {
        let dialog = cx.entity_id().as_u64();
        Store::get(cx).update(cx, |s, cx| s.forget_bot_refusal(dialog, cx));
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        Self::forget_refusal(cx);
        cx.notify();
    }

    fn picked(&mut self, cx: &mut Context<Self>) {
        self.changed(cx);
    }

    fn pick_agent(&mut self, at: usize, cx: &mut Context<Self>) {
        self.chosen = Some(at);
        self.fit_agent(cx);
        cx.notify();
    }

    fn pick_effort(&mut self, effort: Option<String>, cx: &mut Context<Self>) {
        self.effort = effort;
        self.changed(cx);
    }

    fn pick_access(&mut self, access: &str, cx: &mut Context<Self>) {
        self.access = Some(access.to_string());
        self.changed(cx);
    }

    fn fill_model(&mut self, model: &str, cx: &mut Context<Self>) {
        self.model.update(cx, |field, cx| field.fill(model.to_string(), cx));
    }

    fn typed(&self, agent: &AgentInfo, cx: &App) -> Typed {
        let account = self.accounts.picks(std::slice::from_ref(&agent.agent), cx).first().map(|p| p.folder.clone());
        Typed {
            name: self.name.read(cx).text(),
            model: self.model.read(cx).text(),
            effort: self.effort.clone(),
            account,
            access: self.access.clone(),
            about: self.about.read(cx).text(),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if dialog_starts(cx) {
            return;
        }
        let Some(agent) = self.agent() else { return };
        let typed = self.typed(agent, cx);
        match new_args(agent, &typed) {
            Ok(command) => {
                let dialog = cx.entity_id().as_u64();
                let name = typed.name.trim().to_string();
                Store::get(cx).update(cx, |s, cx| s.create_bot(command, &name, dialog, cx));
            }
            Err(NotSent::Access) => {
                self.error = Some(CHOOSE_ACCESS);
                cx.notify();
            }
            Err(NotSent::Blank | NotSent::NotABot(_)) => {}
        }
    }
}

impl BotForm {
    fn name_field(&self, window: &Window, cx: &App) -> Div {
        ui::field("Name", ui::boxed(&self.name, window, cx))
    }

    fn agent_field(&self, cx: &mut Context<Self>) -> Div {
        let body = match &self.listing {
            Listing::Reading => ui::loading(READING).into_any_element(),
            Listing::Failed(words) => div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(ui::error_line(read_failed(words)))
                .child(
                    div().flex().child(
                        ui::button("bot-agents-retry", "Try again")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.read_agents(cx))),
                    ),
                )
                .into_any_element(),
            Listing::Read(list) => self.agent_chips(list, cx),
        };
        ui::field("Agent", body)
    }

    fn agent_chips(&self, list: &[AgentInfo], cx: &mut Context<Self>) -> AnyElement {
        let items = list.iter().enumerate().map(|(at, agent)| {
            let is = self.chosen == Some(at);
            if agent.bot {
                ui::chip(("bot-agent", at), agent.label(), is)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_agent(at, cx)))
            } else {
                ui::resting_chip(("bot-agent", at), agent.label(), false)
            }
        });
        let none = (self.chosen.is_none()).then(|| ui::dim(NO_AGENT));
        div().flex().flex_col().gap(px(6.)).child(ui::chips(items.collect())).children(none).into_any_element()
    }

    fn model_field(&self, agent: &AgentInfo, window: &Window, cx: &mut Context<Self>) -> Div {
        let typed = self.model.read(cx).text();
        let items: Vec<Stateful<Div>> = agent
            .models
            .iter()
            .enumerate()
            .map(|(at, model)| {
                let value = model.clone();
                ui::chip(("bot-model", at), model.clone(), typed.trim() == model)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.fill_model(&value, cx)))
            })
            .collect();
        let column = div().flex().flex_col().gap(px(6.)).child(ui::boxed(&self.model, window, cx));
        ui::field("Model", column.when(!items.is_empty(), |d| d.child(ui::chips(items))))
    }

    fn effort_field(&self, agent: &AgentInfo, cx: &mut Context<Self>) -> Option<Div> {
        if !agent.takes_effort() {
            return None;
        }
        let own = ui::chip(("bot-effort", 0_usize), OWN.to_string(), self.effort.is_none())
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.pick_effort(None, cx)));
        let levels = agent.efforts.iter().enumerate().map(|(at, level)| {
            let value = level.clone();
            ui::chip(("bot-effort", at + 1), level.clone(), self.effort.as_deref() == Some(level))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_effort(Some(value.clone()), cx)))
        });
        Some(ui::field("Effort", ui::chips(std::iter::once(own).chain(levels).collect())))
    }

    fn access_field(&self, agent: &AgentInfo, cx: &mut Context<Self>) -> Div {
        let one = |id, label: &str, value: &'static str, cx: &mut Context<Self>| {
            let is = self.access.as_deref() == Some(value);
            if agent.offers(value) {
                ui::segment(id, label.to_string(), is)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_access(value, cx)))
            } else {
                ui::resting_segment(id, label.to_string(), is)
            }
        };
        let pair = ui::segments(vec![
            one("bot-access-cornercase", THROUGH, CORNERCASE, cx).into_any_element(),
            one("bot-access-full", FULL_ACCESS, FULL, cx).into_any_element(),
        ]);
        let full = div().text_size(px(11.)).text_color(colours::failed()).child(FULL_NOTE);
        let column = div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(div().flex().child(pair))
            .child(ui::dim(THROUGH_NOTE))
            .child(full)
            .children(agent.full_only_line().map(ui::dim));
        ui::field("Access", column)
    }

    fn about_field(&self, window: &Window, cx: &App) -> Div {
        ui::field("What it's for", ui::prompt_box(&self.about, "create", window, cx))
    }

    fn fields(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let column =
            div().flex().flex_col().gap(px(12.)).child(self.name_field(window, cx)).child(self.agent_field(cx));
        let Some(agent) = self.agent().cloned() else { return column };
        let accounts = self.accounts.fields(std::slice::from_ref(&agent.agent), cx);
        column
            .child(self.model_field(&agent, window, cx))
            .children(self.effort_field(&agent, cx))
            .children(accounts)
            .child(self.access_field(&agent, cx))
            .child(self.about_field(window, cx))
    }

    fn refusal(&self, dialog: u64, cx: &App) -> Option<Div> {
        let held = Store::get(cx).read(cx).bots.manage.refusal(dialog).map(str::to_string);
        let words: Option<SharedString> = self.error.map(SharedString::from).or(held.map(SharedString::from));
        words.map(|words| div().text_size(px(12.)).text_color(colours::failed()).child(words))
    }

    fn create_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        if dialog_starts(cx) {
            return ui::busy_button("create", "Creating…", Tone::Primary);
        }
        if self.agent().is_none() {
            return ui::resting_button("create", "Create", Tone::Primary);
        }
        ui::primary_button("create", "Create").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(cx)))
    }
}

impl Focusable for BotForm {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.name.read(cx).focus_handle(cx)
    }
}

impl Render for BotForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let choices = Store::get(cx).read(cx).account_choices();
        self.accounts.follow(choices, Self::picked, cx);
        let height = body_height(f32::from(window.viewport_size().height));
        let body = self.fields(window, cx);
        let scrolled = fitted("bot-form", window, body).min_h_0().track_scroll(&self.scroller);
        let column = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .max_h(px(height))
            .child(scrolled)
            .children(self.refusal(cx.entity_id().as_u64(), cx));
        frame(TITLE, column, cancel_and(self.create_button(cx))).w(px(520.))
    }
}
