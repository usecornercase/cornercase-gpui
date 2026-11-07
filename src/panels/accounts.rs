mod agents;
mod data;
mod profiles;
pub(crate) mod widgets;

use std::collections::HashSet;

use gpui::{
    AnyElement, AnyView, App, ClickEvent, Context, Div, Entity, Focusable as _, IntoElement, Render, SharedString,
    Stateful, Window, div, prelude::*, px,
};

use self::data::{AccountsInfo, Ask, Asking, Target};
use crate::cli;
use crate::dialogs::{self, Question};
use crate::notify::tip;
use crate::panels::PanelKind;
use crate::store::{self, Busy, Store, args};
use crate::theme::colours;
use crate::ui::{self, Icon, TextField, TextFieldEvent, Tone};

const INTRO: &str = "Each agent can keep several logins, each in a folder of its own. New workspaces start with the \
                     default one; open panes keep the account they started with until you switch them on the pane.";
const TOKENS_NOTE: &str = "A token variable wins over every account: an agent that reads it uses it, whichever \
                           account it starts with.";

struct Running {
    target: Option<Target>,
    doing: &'static str,
}

struct Failure {
    at: Option<Target>,
    text: String,
}

struct OpenField {
    target: Target,
    field: Entity<TextField>,
}

pub(crate) struct AccountsPanel {
    info: Option<AccountsInfo>,
    load_error: Option<String>,
    check_error: Option<String>,
    error: Option<Failure>,
    asking: Asking,
    busy: Option<Running>,
    no_login: HashSet<String>,
    form: Option<OpenField>,
    rename: Option<OpenField>,
    was_open: bool,
    store_changes: u64,
}

impl AccountsPanel {
    pub(crate) fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::get(cx);
        cx.observe(&store, |this, store, cx| {
            let store = store.read(cx);
            let open = store.sheet == Some(PanelKind::Accounts);
            let changed = store.accounts_changes != this.store_changes;
            this.store_changes = store.accounts_changes;
            if open && (changed || !this.was_open) {
                this.load(cx);
            }
            this.was_open = open;
        })
        .detach();
        let mut this = Self {
            info: None,
            load_error: None,
            check_error: None,
            error: None,
            asking: Asking::default(),
            busy: None,
            no_login: HashSet::new(),
            form: None,
            rename: None,
            was_open: true,
            store_changes: store.read(cx).accounts_changes,
        };
        this.load(cx);
        this
    }

    fn rename_field(current: String, cx: &mut Context<Self>) -> Entity<TextField> {
        let field = cx.new(|cx| TextField::new("a name; empty clears it", cx));
        field.update(cx, |f, cx| f.set_text(current, cx));
        cx.subscribe(&field, |this, _, event: &TextFieldEvent, cx| match event {
            TextFieldEvent::Submit => this.submit_rename(cx),
            TextFieldEvent::Cancel => this.cancel_rename(cx),
            TextFieldEvent::Changed => cx.notify(),
        })
        .detach();
        field
    }

    fn form_field(form: &Target, cx: &mut Context<Self>) -> Entity<TextField> {
        let placeholder = match form {
            Target::Adding(_) => "a name, such as work",
            _ => "a name, such as Work",
        };
        let field = cx.new(|cx| TextField::new(placeholder, cx));
        cx.subscribe(&field, |this, _, event: &TextFieldEvent, cx| match event {
            TextFieldEvent::Submit => this.submit_open_form(cx),
            TextFieldEvent::Cancel => this.close_form(cx),
            TextFieldEvent::Changed => cx.notify(),
        })
        .detach();
        field
    }

    fn form_is(&self, form: &Target) -> bool {
        self.form.as_ref().is_some_and(|open| open.target == *form)
    }

    fn form_text(&self, cx: &App) -> String {
        self.form.as_ref().map(|open| open.field.read(cx).text().trim().to_string()).unwrap_or_default()
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let asked = self.asking.list();
        cx.notify();
        store::run(cx, args(&["account", "list", "--json"]), move |this, result, cx| {
            if !this.asking.list_answered(asked) {
                return;
            }
            match result.and_then(|out| data::parse(&out)) {
                Ok(info) => {
                    this.load_error = None;
                    this.show(info, cx);
                    this.check(cx);
                }
                Err(e) => this.load_error = Some(cli::reason(&e)),
            }
            cx.notify();
        });
    }

    fn check(&mut self, cx: &mut Context<Self>) {
        let asked = self.asking.check();
        store::run(cx, args(&["account", "list", "--check", "--json"]), move |this, result, cx| {
            if !this.asking.check_answered(asked) {
                return;
            }
            match result.and_then(|out| data::parse(&out)) {
                Ok(info) if this.busy.is_none() => {
                    this.check_error = None;
                    this.show(info, cx);
                }
                Ok(_) => {}
                Err(e) => this.check_error = Some(cli::reason(&e)),
            }
            cx.notify();
        });
    }

    fn show(&mut self, info: AccountsInfo, cx: &mut Context<Self>) {
        self.info = Some(info);
        cx.notify();
    }

    fn change(
        &mut self,
        command: Vec<String>,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, Option<AccountsInfo>, &mut Context<Self>) + 'static,
    ) {
        let doing = data::doing(&command);
        let target = data::target(&command);
        self.busy = Some(Running { target: target.clone(), doing });
        self.error = None;
        cx.notify();
        store::run_busy(cx, Busy::new("accounts", doing), command, move |this, result, cx| {
            this.busy = None;
            match result {
                Ok(out) => this.changed(&out, then, cx),
                Err(e) => this.error = Some(Failure { at: target, text: cli::reason(&e) }),
            }
            cx.notify();
        });
    }

    fn changed(
        &mut self,
        out: &str,
        then: impl FnOnce(&mut Self, Option<AccountsInfo>, &mut Context<Self>),
        cx: &mut Context<Self>,
    ) {
        let answer = data::parse(out).ok();
        if let Some(info) = &answer {
            self.show(info.clone(), cx);
        }
        then(self, answer, cx);
        self.load(cx);
    }

    fn act(&mut self, command: Vec<String>, cx: &mut Context<Self>) {
        self.change(command, cx, |_, _, _| {});
    }

    fn idle(&self) -> bool {
        self.busy.is_none()
    }

    fn running_on(&self, target: &Target) -> Option<&'static str> {
        self.busy.as_ref().filter(|r| r.target.as_ref() == Some(target)).map(|r| r.doing)
    }

    fn error_at(&self, target: Option<&Target>) -> Option<String> {
        self.error.as_ref().filter(|e| e.at.as_ref() == target).map(|e| e.text.clone())
    }

    fn add(&mut self, agent: &str, cx: &mut Context<Self>) {
        let name = self.form_text(cx);
        if name.is_empty() || !self.idle() {
            return;
        }
        if !data::valid_name(&name) {
            let text = format!("`{name}`: an account's name takes letters, digits, `.`, `_` and `-`, at most 32");
            self.error = Some(Failure { at: Some(Target::Adding(agent.to_string())), text });
            cx.notify();
            return;
        }
        let workspace = Store::get(cx).read(cx).session.as_ref().and_then(|s| s.shown().workspace);
        let login = if self.no_login.contains(agent) { None } else { workspace };
        self.change(data::add_args(agent, &name, login), cx, |this, answer, cx| {
            this.form = None;
            show_login_tab(answer, cx);
        });
    }

    fn toggle_login(&mut self, agent: &str, cx: &mut Context<Self>) {
        if !self.no_login.remove(agent) {
            self.no_login.insert(agent.to_string());
        }
        cx.notify();
    }

    fn new_profile(&mut self, cx: &mut Context<Self>) {
        let name = self.form_text(cx);
        if name.is_empty() || !self.idle() {
            return;
        }
        self.change(data::profile_new_args(&name), cx, |this, _, _| this.form = None);
    }

    fn open_form(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        let field = Self::form_field(&target, cx);
        window.focus(&field.read(cx).focus_handle(cx));
        self.form = Some(OpenField { target, field });
        self.rename = None;
        self.error = None;
        cx.notify();
    }

    fn close_form(&mut self, cx: &mut Context<Self>) {
        let target = self.form.take().map(|open| open.target);
        if self.error.as_ref().is_some_and(|e| e.at == target) {
            self.error = None;
        }
        cx.notify();
    }

    fn submit_open_form(&mut self, cx: &mut Context<Self>) {
        match self.form.as_ref().map(|open| open.target.clone()) {
            Some(Target::Adding(agent)) => self.add(&agent, cx),
            Some(Target::NewProfile) => self.new_profile(cx),
            _ => {}
        }
    }

    fn start_rename(&mut self, target: Target, current: String, window: &mut Window, cx: &mut Context<Self>) {
        let field = Self::rename_field(current, cx);
        window.focus(&field.read(cx).focus_handle(cx));
        self.rename = Some(OpenField { target, field });
        self.form = None;
        self.error = None;
        cx.notify();
    }

    fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        self.rename = None;
        cx.notify();
    }

    fn submit_rename(&mut self, cx: &mut Context<Self>) {
        let Some(OpenField { target, field }) = self.rename.take() else { return };
        let label = field.read(cx).text().trim().to_string();
        let command = match &target {
            Target::Account(provider, account) => data::rename_args(provider, account, &label),
            Target::Profile(name) if !label.is_empty() && label != *name => data::profile_rename_args(name, &label),
            _ => {
                cx.notify();
                return;
            }
        };
        self.act(command, cx);
    }

    fn ask(&mut self, ask: Ask, window: &mut Window, cx: &mut Context<Self>) {
        self.rename = None;
        self.form = None;
        let panel = cx.entity();
        let command = ask.command;
        let question = Question::new(ask.title, ask.message, ask.button, Vec::new()).on_yes(move |cx| {
            let command = command.clone();
            panel.update(cx, |this, cx| this.act(command, cx));
        });
        dialogs::confirm(question, window, cx);
    }
}

impl AccountsPanel {
    fn refresh_button(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.asking.running() {
            return div()
                .size(px(28.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .child(ui::spinner("accounts-checking", 14., colours::muted()))
                .into_any_element();
        }
        ui::tool_button("accounts-refresh", Icon::Refresh, 28., false)
            .tooltip(tip("List the accounts and check their logins again"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.load(cx)))
            .into_any_element()
    }

    fn top_bar(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .child(div().flex_1().min_w_0().child(widgets::note(INTRO)))
            .child(self.refresh_button(cx))
    }

    fn renames(&self, target: &Target) -> bool {
        self.rename.as_ref().is_some_and(|open| open.target == *target)
    }

    fn rename_row(&self, id: String, lead: impl IntoElement, window: &Window, cx: &mut Context<Self>) -> Stateful<Div> {
        let field = self.rename.as_ref().map(|open| {
            let focused = open.field.read(cx).focus_handle(cx).is_focused(window);
            ui::input_box(open.field.clone(), focused).max_w(px(360.)).overflow_hidden()
        });
        widgets::row(SharedString::from(id), false)
            .child(widgets::lead(lead))
            .child(div().flex_1().min_w_0().children(field))
            .child(ui::button("rename-cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| this.cancel_rename(cx))))
            .child(
                ui::primary_button("rename-save", "Save")
                    .on_click(cx.listener(|this, _, _, cx| this.submit_rename(cx))),
            )
    }

    fn form_buttons(&self, form: &Target, label: &'static str, doing: &'static str, cx: &mut Context<Self>) -> Div {
        let buttons = div().flex().items_center().gap(px(8.));
        if self.running_on(form).is_some() {
            return buttons.child(ui::busy_button("form-busy", doing, Tone::Primary));
        }
        buttons
            .child(ui::button("form-cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| this.close_form(cx))))
            .child(
                ui::primary_button("form-go", label)
                    .when(!self.idle(), |d| d.opacity(0.5))
                    .on_click(cx.listener(|this, _, _, cx| this.submit_open_form(cx))),
            )
    }

    fn form_input(&self, width: f32, window: &Window, cx: &App) -> Option<Div> {
        let open = self.form.as_ref()?;
        let focused = open.field.read(cx).focus_handle(cx).is_focused(window);
        Some(ui::input_box(open.field.clone(), focused).w(px(width)).overflow_hidden())
    }

    fn settled(&self, line: Stateful<Div>, note: Option<Div>, target: &Target) -> AnyElement {
        let line = line.when(self.running_on(target).is_some(), |d| d.opacity(0.6));
        let error = self.error_at(Some(target)).map(ui::error_line);
        let notes = note.into_iter().chain(error).map(widgets::under_row);
        div().flex().flex_col().child(line).children(notes).into_any_element()
    }

    fn form_note(&self, form: &Target, hint: &'static str) -> Div {
        match self.error_at(Some(form)) {
            Some(error) => ui::error_line(error),
            None => widgets::hint(hint),
        }
    }

    fn page(&self, info: &AccountsInfo, window: &Window, cx: &mut Context<Self>) -> Div {
        let cards: Vec<AnyElement> = info.providers.iter().map(|p| self.agent_card(p, window, cx)).collect();
        let tokens = (!info.tokens.is_empty())
            .then(|| ui::callout(colours::waiting(), data::tokens_title(&info.tokens), TOKENS_NOTE));
        let failed = self.error_at(None).map(|e| ui::callout(colours::failed(), "The change failed", e));
        let empty = cards.is_empty().then(|| widgets::note("No agent here takes accounts."));
        widgets::page()
            .child(self.top_bar(cx))
            .children(tokens)
            .children(failed)
            .children(empty)
            .children(cards)
            .child(self.profiles_card(info, window, cx))
    }

    fn waiting(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(error) = self.load_error.clone() else {
            return ui::loading("Listing the accounts…").into_any_element();
        };
        let try_again = ui::button("accounts-retry", "Try again")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.load(cx)));
        widgets::could_not_read(format!("Could not list the accounts: {error}"), try_again).into_any_element()
    }
}

fn show_login_tab(answer: Option<AccountsInfo>, cx: &mut App) {
    let Some(tab) = answer.and_then(|info| info.login).and_then(|ids| ids.tab) else { return };
    Store::get(cx).update(cx, |s, cx| {
        s.focus("tab", tab);
        s.close_sheet(cx);
    });
}

impl Render for AccountsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.info.clone() {
            Some(info) => self.page(&info, window, cx).into_any_element(),
            None => self.waiting(cx),
        };
        div().id("accounts").size_full().overflow_y_scroll().child(body)
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| AccountsPanel::new(window, cx)).into()
}
