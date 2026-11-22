use gpui::{
    App, AppContext as _, ClickEvent, Context, Div, Entity, FocusHandle, Focusable, IntoElement, ParentElement as _,
    Render, ScrollHandle, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};

use self::chat_apps::ChatRow;
use self::follow::FollowBox;
use self::more::More;
use self::secrets::SecretsRow;
use super::{
    body_height, bot_refusal, cancel_and, close, dialog_starts, fitted, forget_refusal, frame, revealed, show, within,
};
use crate::model::bots::{
    AgentInfo, BotKey, Editing, NotSent, Save, Typed, agents_of, chosen_at_open, kept_accounts, new_args,
};
use crate::store::{Store, args};
use crate::theme::colours;
use crate::ui::account_picker::AccountPickers;
use crate::ui::{self, TextField, TextFieldEvent, Tone};

mod chat_apps;
mod follow;
mod look;
mod more;
mod secrets;

const TITLE: &str = "New bot";
const SAVE: &str = "Save";
const SAVING: &str = "Saving…";
const KEEPS_AGENT: &str = "A bot keeps its agent: another one cannot continue its conversation.";
const ABOUT_WAITS: &str = "Stays as it is: it is bot.md, in the bot's folder. Showing and changing it here needs usecornercase/cornercase-terminal#876.";
const FROM_NEXT: &str =
    "Only what you change is sent, from the bot's next message on: an answer it is writing keeps what it had.";
const READING: &str = "Reading the agents…";
const NO_AGENT: &str = "No agent here can be a bot yet.";
const CHOOSE_ACCESS: &str = "Choose its access.";
const WIDTH: f32 = 540.;
const INSIDE: f32 = WIDTH - 46.;
const ABOUT_HINT: &str = "what it is for (optional)";
const OWN: &str = "the agent's own";

fn edit_title(name: &str) -> String {
    format!("Edit the bot {name}")
}

pub(crate) fn read_failed(words: &str) -> String {
    format!("Could not read the agents: {words}")
}

enum Listing {
    Reading,
    Read(Vec<AgentInfo>),
    Failed(String),
}

enum Mode {
    New,
    Edit(Box<Editing>),
}

pub(crate) struct BotForm {
    mode: Mode,
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
    chat: ChatRow,
    follow: FollowBox,
    secrets: SecretsRow,
    face: Option<u8>,
    more: More,
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

fn open_form(mode: Mode, window: &mut Window, cx: &mut App) -> Option<Entity<BotForm>> {
    Store::get(cx).read(cx).session.as_ref()?;
    let view = cx.new(|cx| BotForm::new(mode, window, cx));
    let focus = view.read(cx).name.read(cx).focus_handle(cx);
    show(view.clone().into(), &focus, window, cx);
    Some(view)
}

pub(crate) fn new_bot(window: &mut Window, cx: &mut App) {
    open_form(Mode::New, window, cx);
}

pub(crate) fn new_bot_for(name: &'static str, about: &'static str, window: &mut Window, cx: &mut App) {
    let Some(view) = open_form(Mode::New, window, cx) else { return };
    let (name_field, about_field) = {
        let form = view.read(cx);
        (form.name.clone(), form.about.clone())
    };
    name_field.update(cx, |field, cx| field.set_text(name, cx));
    about_field.update(cx, |field, cx| field.set_text(about, cx));
}

pub(crate) fn edit_bot(id: &str, window: &mut Window, cx: &mut App) {
    let Some(bot) = Store::get(cx).read(cx).bots.find(id).cloned() else { return };
    if open_form(Mode::Edit(Box::new(Editing::of(&bot))), window, cx).is_some() {
        Store::get(cx).update(cx, |s, _| s.names_bot(&BotKey::here(id)));
    }
}

impl BotForm {
    fn new(mode: Mode, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let scroller = ScrollHandle::new();
        let plain = || within(&scroller, |f| f);
        let (placeholder, own) = match &mode {
            Mode::New => ("PRs".to_string(), String::new()),
            Mode::Edit(editing) => (editing.before.name.clone(), editing.before.model.clone()),
        };
        let name = field(&placeholder, plain(), cx, window);
        let model = field(OWN, plain(), cx, window);
        let about = field(ABOUT_HINT, within(&scroller, TextField::prompt), cx, window);
        model.update(cx, |last, _| last.set_ends_form(true));
        if let Mode::Edit(editing) = &mode {
            name.update(cx, |name, cx| name.set_text(editing.before.name.clone(), cx));
            model.update(cx, |model, cx| model.set_text(own, cx));
        }
        Self::follow_store(cx);
        let mut form = Self {
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
            chat: ChatRow::new(&mode, cx),
            follow: FollowBox::new(&mode, cx),
            secrets: SecretsRow::new(cx),
            face: None,
            more: More::new(&mode, cx),
            mode,
        };
        form.read_agents(cx);
        form.follow_secrets(cx);
        form
    }

    fn follow_store(cx: &mut Context<Self>) {
        cx.observe(&Store::get(cx), |this, _, cx| {
            this.follow_secrets(cx);
            cx.notify();
        })
        .detach();
    }

    fn editing(&self) -> Option<&Editing> {
        match &self.mode {
            Mode::Edit(editing) => Some(editing),
            Mode::New => None,
        }
    }

    fn read_agents(&mut self, cx: &mut Context<Self>) {
        self.listing = Listing::Reading;
        crate::store::run(cx, args(&["bot", "agents", "--json"]), Self::agents_read);
        cx.notify();
    }

    fn agents_read(&mut self, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        match result.and_then(|out| agents_of(&out)) {
            Ok(list) => {
                self.chosen = self.first_chosen(&list, cx);
                let kept = kept_accounts(&list, self.editing());
                self.accounts = AccountPickers::keeping(kept).inside(self.scroller.clone());
                self.listing = Listing::Read(list);
                self.fit(cx);
            }
            Err(error) => self.listing = Listing::Failed(crate::cli::reason(&error)),
        }
        cx.notify();
    }

    fn first_chosen(&self, list: &[AgentInfo], cx: &App) -> Option<usize> {
        if let Some(editing) = self.editing() {
            return list.iter().position(|a| a.agent == editing.agent);
        }
        let default = Store::get(cx).read(cx).account_settings.default_agent.clone();
        chosen_at_open(list, default.as_deref())
    }

    fn fit(&mut self, cx: &mut Context<Self>) {
        match self.editing().map(|editing| editing.before.clone()) {
            Some(before) => self.fit_edit(&before, cx),
            None => self.fit_agent(cx),
        }
    }

    fn fit_edit(&mut self, before: &Typed, cx: &mut Context<Self>) {
        let hint = self.agent().map(AgentInfo::model_hint).unwrap_or_default();
        self.effort.clone_from(&before.effort);
        self.access.clone_from(&before.access);
        self.reset_model(hint, false, cx);
    }

    fn reset_model(&mut self, hint: String, clear: bool, cx: &mut Context<Self>) {
        self.error = None;
        self.model.update(cx, |model, cx| {
            if clear {
                model.clear(cx);
            }
            model.set_placeholder(if hint.is_empty() { OWN.to_string() } else { hint }, cx);
        });
        forget_refusal(cx);
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
        self.reset_model(hint, true, cx);
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        forget_refusal(cx);
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
            chat_apps: self.chat_sent(cx),
            follow: self.follow_sent(cx),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let Some(agent) = self.agent().cloned() else { return };
        let typed = self.typed(&agent, cx);
        let dialog = cx.entity_id().as_u64();
        let face = self.shown_face(cx);
        if let Some(editing) = self.editing() {
            let picked = self.face.map(|face| (BotKey::here(&editing.id), face));
            match editing.save(dialog_starts(cx), &agent, &typed) {
                Save::Waits => {}
                Save::Closes => {
                    Store::get(cx).update(cx, |s, cx| s.keep_picked_face(picked, cx));
                    close(cx);
                }
                Save::Sends(command) => {
                    let name = editing.before.name.clone();
                    Store::get(cx).update(cx, |s, cx| s.save_bot(command, (&name, picked), dialog, cx));
                }
            }
            return;
        }
        if dialog_starts(cx) {
            return;
        }
        match new_args(&agent, &typed) {
            Ok(command) => {
                let name = typed.name.trim().to_string();
                Store::get(cx).update(cx, |s, cx| s.create_bot(command, (&name, face), dialog, cx));
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

    fn fields(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let column = div().w(px(INSIDE)).flex().flex_col().gap(px(14.)).child(self.hero(cx));
        let column = column.child(self.name_field(window, cx));
        let column = column.when(self.editing().is_none(), |d| d.child(self.about_field(window, cx)));
        let column = column.child(self.agent_field(cx));
        let Some(agent) = self.agent().cloned() else { return column };
        let column = column.child(self.access_field(&agent, cx)).child(self.more_head(&agent, cx));
        let column = column.when(self.more.open, |d| d.child(self.more_fields(&agent, window, cx)));
        column.children(self.editing().map(|_| ui::dim(FROM_NEXT).w_full()))
    }

    fn more_fields(&self, agent: &AgentInfo, window: &Window, cx: &mut Context<Self>) -> Div {
        let accounts = self.accounts.fields(std::slice::from_ref(&agent.agent), cx);
        let about = self.editing().map(|_| ui::field("What it's for", ui::dim(ABOUT_WAITS)));
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .pl(px(12.))
            .border_l_2()
            .border_color(colours::border())
            .child(self.model_field(agent, window, cx))
            .children(self.effort_field(agent, cx))
            .children(accounts)
            .children(self.follow_field(cx))
            .children(self.chat_field(cx))
            .children(self.secrets_field(cx))
            .children(about)
    }

    fn submit_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let (ready, doing) = if self.editing().is_some() { (SAVE, SAVING) } else { ("Create", "Creating…") };
        if dialog_starts(cx) {
            return ui::busy_button("create", doing, Tone::Primary);
        }
        if self.agent().is_none() {
            return ui::resting_button("create", ready, Tone::Primary);
        }
        ui::primary_button("create", ready).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(cx)))
    }

    fn title(&self) -> String {
        self.editing().map_or_else(|| TITLE.to_string(), |editing| edit_title(&editing.before.name))
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
        self.chat_reveal(window);
        self.follow_reveal(window);
        self.secrets_keys(window);
        let body = self.fields(window, cx);
        let scrolled = fitted("bot-form", window, body).min_h_0().track_scroll(&self.scroller);
        let column = div().flex().flex_col().gap(px(12.)).max_h(px(height)).child(scrolled).children(bot_refusal(
            self.error,
            cx.entity_id().as_u64(),
            cx,
        ));
        frame(self.title(), column, cancel_and(self.submit_button(cx))).w(px(WIDTH))
    }
}
