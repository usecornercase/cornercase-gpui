use gpui::{
    AnyElement, ClickEvent, Context, Div, Focusable as _, IntoElement, Rgba, SharedString, Stateful, Window, div,
    prelude::*, px,
};

use super::AccountsPanel;
use super::data::{self, LoginState, Provider, Row, Status, Target};
use super::widgets::{self, STATUS};
use crate::notify::tip;
use crate::panels::settings::widgets::capitalised;
use crate::theme::{FONT_FAMILY, colours};
use crate::ui::{self, Icon, Tone};

const NAME_RULE: &str = "Letters, digits, `.`, `_` and `-`, at most 32: it names the account's folder.";

fn home() -> String {
    std::env::var("HOME").unwrap_or_default()
}

fn env_label(provider: &Provider) -> impl IntoElement + use<> {
    div()
        .id(SharedString::from(format!("env-{}", provider.id)))
        .min_w_0()
        .font_family(FONT_FAMILY)
        .text_size(px(11.))
        .text_color(colours::faint())
        .tooltip(tip("The variable that points the agent at the account's folder"))
        .child(ui::one_line(provider.env.clone()))
}

fn account_name(row: Row) -> Div {
    let chips =
        [row.account.is_none().then(|| ui::quiet_chip("own login")), row.active().then(|| ui::accent_chip("default"))];
    widgets::name_column(
        widgets::title(row.name().to_string()).children(chips.into_iter().flatten()),
        row.detail(&home()),
    )
}

fn unshared_note(row: Row) -> Option<Div> {
    let account = row.account.filter(|a| !a.unshared.is_empty())?;
    Some(ui::warning_line(data::unshared_text(&account.unshared)))
}

fn login_colour(state: LoginState) -> Option<Rgba> {
    match state {
        LoginState::SignedIn => Some(colours::done()),
        LoginState::SignedOut => Some(colours::failed()),
        LoginState::Failed => Some(colours::waiting()),
        LoginState::Unknown => None,
    }
}

fn login_line(row: Row, state: LoginState, headline: String, plan: Option<String>) -> Stateful<Div> {
    let colour = login_colour(state);
    let ink = match state {
        LoginState::SignedIn => colours::soft(),
        _ => colour.unwrap_or_else(colours::muted),
    };
    let line = div()
        .id(SharedString::from(row.id("login")))
        .flex()
        .items_center()
        .gap(px(7.))
        .text_size(px(12.))
        .child(ui::dot(colour))
        .child(div().min_w_0().text_color(ink).child(ui::one_line(headline)))
        .children(plan.map(|plan| ui::quiet_chip(capitalised(&plan))));
    match row.login_hint(state) {
        Some(hint) => line.tooltip(tip(hint)),
        None => line,
    }
}

impl AccountsPanel {
    pub(super) fn agent_card(&self, provider: &Provider, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let rows: Vec<AnyElement> = Row::of(provider).map(|row| self.account_row(row, window, cx)).collect();
        widgets::card()
            .child(self.agent_header(provider, cx))
            .child(widgets::rule())
            .children(rows)
            .children(self.add_form(provider, window, cx))
            .into_any_element()
    }

    fn agent_header(&self, provider: &Provider, cx: &mut Context<Self>) -> Div {
        let form = Target::Adding(provider.id.clone());
        let open = self.form.as_ref() == Some(&form);
        widgets::header(Icon::Agent, provider.title.clone())
            .child(env_label(provider))
            .child(div().flex_1())
            .when(!open, |d| d.child(self.add_button(form, cx)))
    }

    fn add_button(&self, form: Target, cx: &mut Context<Self>) -> Stateful<Div> {
        let id = match &form {
            Target::Adding(agent) => format!("add-{agent}"),
            _ => "add".to_string(),
        };
        let button = ui::icon_text_button(SharedString::from(id), Icon::Plus, "Add account", Tone::Plain);
        if !self.idle() {
            return button.opacity(0.5).cursor_default();
        }
        button.on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.open_form(form.clone(), window, cx)))
    }

    fn account_row(&self, row: Row, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let target = row.target();
        if self.rename.as_ref() == Some(&target) {
            return self.rename_row(row.id("renaming"), self.default_radio(row, cx), window, cx).into_any_element();
        }
        let doing = self.running_on(&target);
        let line = widgets::row(SharedString::from(row.id("row")), doing.is_none())
            .child(widgets::lead(self.default_radio(row, cx)))
            .child(account_name(row))
            .child(self.status_cell(row, doing))
            .child(self.account_actions(row, cx));
        self.settled(line, unshared_note(row), &target)
    }

    fn default_radio(&self, row: Row, cx: &mut Context<Self>) -> Stateful<Div> {
        let hint = if row.active() {
            "The default: new workspaces start with it"
        } else {
            "Make it the default: new workspaces start with it"
        };
        let radio = widgets::radio(SharedString::from(row.id("default")), row.active(), hint, self.idle());
        if row.active() || !self.idle() {
            return radio;
        }
        let command = data::use_args(row.agent(), row.folder());
        radio.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(command.clone(), cx)))
    }

    fn status_cell(&self, row: Row, doing: Option<&'static str>) -> Div {
        let cell = div().w(px(STATUS)).flex_none().min_w_0();
        if let Some(doing) = doing {
            return cell.child(ui::busy(SharedString::from(row.id("busy")), doing));
        }
        cell.child(self.login_status(row))
    }

    fn login_status(&self, row: Row) -> AnyElement {
        match row.status(self.asking.list || self.asking.check) {
            Status::Login { state, headline, plan } => login_line(row, state, headline, plan).into_any_element(),
            Status::Checking => {
                ui::busy(SharedString::from(row.id("checking")), "Checking the login…").into_any_element()
            }
            Status::NotChecked => self.not_checked(row).into_any_element(),
            Status::Unasked => div().into_any_element(),
        }
    }

    fn not_checked(&self, row: Row) -> Stateful<Div> {
        let why = self.check_error.clone().unwrap_or_else(|| "The logins were not checked yet".into());
        div()
            .id(SharedString::from(row.id("unchecked")))
            .flex()
            .items_center()
            .gap(px(7.))
            .text_size(px(12.))
            .text_color(colours::faint())
            .tooltip(tip(why))
            .child(ui::dot(None))
            .child(div().min_w_0().child(ui::one_line("Login not checked")))
    }

    fn account_actions(&self, row: Row, cx: &mut Context<Self>) -> Div {
        let remove = match row.account {
            Some(_) => self.remove_action(row, cx).into_any_element(),
            None => widgets::slot().into_any_element(),
        };
        widgets::actions().child(self.rename_action(row, cx)).child(remove)
    }

    fn rename_action(&self, row: Row, cx: &mut Context<Self>) -> AnyElement {
        if !self.idle() {
            return ui::resting_action(Icon::Pencil).into_any_element();
        }
        let (target, current) = (row.target(), row.label().to_string());
        ui::row_action(SharedString::from(row.id("rename")), Icon::Pencil, "Rename", false)
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.start_rename(target.clone(), current.clone(), window, cx);
            }))
            .into_any_element()
    }

    fn remove_action(&self, row: Row, cx: &mut Context<Self>) -> AnyElement {
        if !self.idle() {
            return ui::resting_action(Icon::Trash).into_any_element();
        }
        let ask = row.remove_ask();
        ui::row_action(SharedString::from(row.id("remove")), Icon::Trash, "Remove…", true)
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.ask(ask.clone(), window, cx)))
            .into_any_element()
    }

    fn add_form(&self, provider: &Provider, window: &Window, cx: &mut Context<Self>) -> Option<Div> {
        let form = Target::Adding(provider.id.clone());
        if self.form.as_ref() != Some(&form) {
            return None;
        }
        let field = self.add_fields.get(&provider.id)?;
        let focused = field.read(cx).focus_handle(cx).is_focused(window);
        let note = self.form_note(&form, NAME_RULE);
        let line = widgets::form_line()
            .child(ui::input_box(field.clone(), focused).w(px(260.)).overflow_hidden())
            .child(self.login_checkbox(&provider.id, cx))
            .child(div().flex_1())
            .child(self.form_buttons(form, "Add account", "Adding…", cx));
        Some(widgets::form().child(line).child(note))
    }

    fn login_checkbox(&self, agent: &str, cx: &mut Context<Self>) -> Stateful<Div> {
        let login = !self.no_login.contains(agent);
        let target = agent.to_string();
        ui::checkbox(SharedString::from(format!("login-{agent}")), login, "Log in now, in a new tab")
            .text_size(px(12.))
            .text_color(colours::soft())
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_login(&target, cx)))
    }
}
