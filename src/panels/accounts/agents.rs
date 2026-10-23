use gpui::{
    AnyElement, Context, Div, Focusable as _, IntoElement, SharedString, Stateful, Window, div, prelude::*, px,
};

use super::data::{self, Account, Login, LoginState, Provider};
use super::{AccountsPanel, Pending, Rename, marked_name};
use crate::panels::settings::widgets;
use crate::theme::colours;
use crate::ui;

#[derive(Clone, Copy)]
struct AccountRow<'a> {
    provider: &'a Provider,
    account: Option<&'a Account>,
}

impl<'a> AccountRow<'a> {
    fn agent(self) -> &'a str {
        &self.provider.id
    }

    fn folder(self) -> &'a str {
        self.account.map_or("", |a| a.name.as_str())
    }

    fn active(self) -> bool {
        self.provider.active == self.folder()
    }

    fn name(self) -> &'a str {
        self.provider.name_of(self.folder())
    }

    fn path(self) -> &'a str {
        match self.account {
            Some(a) => &a.path,
            None => self.provider.system_path.as_deref().unwrap_or_default(),
        }
    }

    fn login(self) -> Option<&'a Login> {
        match self.account {
            Some(a) => a.login.as_ref(),
            None => self.provider.system_login.as_ref(),
        }
    }

    fn label(self) -> &'a str {
        match self.account {
            Some(a) => &a.label,
            None => &self.provider.system_label,
        }
    }

    fn id(self) -> String {
        format!("{}-{}", self.agent(), self.folder())
    }

    fn rename(self) -> Rename {
        Rename::Account(self.agent().to_string(), self.folder().to_string())
    }

    fn remove(self) -> Pending {
        Pending::Remove(self.agent().to_string(), self.folder().to_string())
    }
}

impl AccountsPanel {
    pub(super) fn render_provider(&self, provider: &Provider, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let mut card = widgets::card().child(self.render_account(AccountRow { provider, account: None }, window, cx));
        for account in &provider.accounts {
            card = card.child(self.render_account(AccountRow { provider, account: Some(account) }, window, cx));
        }
        card = card.child(self.render_add(provider, window, cx));
        div()
            .flex()
            .flex_col()
            .child(ui::section(format!("{}  ·  {}", provider.title, provider.env)))
            .child(card)
            .into_any_element()
    }

    fn render_account(&self, row: AccountRow, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let renaming = self.rename.as_ref() == Some(&row.rename());
        let confirm = (self.pending.as_ref() == Some(&row.remove())).then(|| Self::confirm_remove(row, cx));
        div()
            .flex()
            .flex_col()
            .child(widgets::row(row.account.is_none(), account_label(row), self.account_buttons(row, cx)))
            .when(renaming, |d| d.child(self.render_rename(window, cx)))
            .children(confirm)
            .into_any_element()
    }

    fn account_buttons(&self, row: AccountRow, cx: &mut Context<Self>) -> Div {
        let buttons = div().flex().gap(px(6.));
        if self.busy {
            return buttons;
        }
        let id = row.id();
        let (agent, folder) = (row.agent().to_string(), row.folder().to_string());
        let (target, current) = (row.rename(), row.label().to_string());
        let pending = row.remove();
        buttons
            .when(!row.active(), |d| {
                d.child(
                    ui::button(SharedString::from(format!("use-{id}")), "Make default")
                        .on_click(cx.listener(move |this, _, _, cx| this.act(data::use_args(&agent, &folder), cx))),
                )
            })
            .child(ui::button(SharedString::from(format!("rename-{id}")), "Rename").on_click(
                cx.listener(move |this, _, window, cx| this.start_rename(target.clone(), current.clone(), window, cx)),
            ))
            .when(row.account.is_some(), |d| {
                d.child(
                    ui::button(SharedString::from(format!("remove-{id}")), "Remove")
                        .on_click(cx.listener(move |this, _, _, cx| this.ask(pending.clone(), cx))),
                )
            })
    }

    fn confirm_remove(row: AccountRow, cx: &mut Context<Self>) -> Div {
        let (agent, folder) = (row.agent().to_string(), row.folder().to_string());
        widgets::confirm(
            format!(
                "Remove {} and delete its folder? Panes and profiles that use it go back to the own login. \
                 The keychain item stays: to log out first, run the agent's logout in a tab of this account.",
                row.name()
            ),
            Self::pending_cancel(cx),
            ui::danger_button("pending-go", "Remove")
                .on_click(cx.listener(move |this, _, _, cx| this.act(data::remove_args(&agent, &folder), cx))),
        )
    }

    fn render_add(&self, provider: &Provider, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(field) = self.add_fields.get(&provider.id) else { return div().into_any_element() };
        let focused = field.read(cx).focus_handle(cx).is_focused(window);
        widgets::row(
            false,
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(ui::input_box(field.clone(), focused).w(px(240.)).overflow_hidden())
                .child(self.login_checkbox(&provider.id, cx)),
            self.add_button(provider.id.clone(), cx),
        )
        .into_any_element()
    }

    fn login_checkbox(&self, agent: &str, cx: &mut Context<Self>) -> Stateful<Div> {
        let login = !self.no_login.contains(agent);
        let target = agent.to_string();
        ui::checkbox(SharedString::from(format!("login-{agent}")), login, "log in now, in a new tab")
            .text_size(px(12.))
            .text_color(colours::muted())
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_login(&target, cx)))
    }

    fn add_button(&self, agent: String, cx: &mut Context<Self>) -> Stateful<Div> {
        ui::primary_button(SharedString::from(format!("add-{agent}")), "Add account")
            .when(self.busy, |d| d.opacity(0.5))
            .on_click(cx.listener(move |this, _, _, cx| {
                if !this.busy {
                    this.add(&agent, cx);
                }
            }))
    }
}

fn account_label(row: AccountRow) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(marked_name(row.name(), row.active(), "default").children(row.login().map(login_text)))
        .child(div().pl(px(20.)).flex().gap(px(8.)).children(account_details(row)))
        .children(row.account.filter(|a| !a.unshared.is_empty()).map(unshared_note))
}

fn login_text(login: &Login) -> Div {
    let colour = match login.state {
        LoginState::SignedIn => colours::done(),
        LoginState::SignedOut | LoginState::Failed => colours::failed(),
        LoginState::Unknown => colours::faint(),
    };
    div().text_size(px(12.)).text_color(colour).child(login.text())
}

fn account_details(row: AccountRow) -> Vec<Div> {
    let first = match row.account {
        Some(a) if !a.label.is_empty() => Some(ui::dim(format!("folder {}", a.name))),
        Some(_) => None,
        None => Some(ui::dim("its own login")),
    };
    first.into_iter().chain([ui::dim(row.path().to_string())]).collect()
}

fn unshared_note(account: &Account) -> Div {
    div()
        .pl(px(20.))
        .text_size(px(11.5))
        .text_color(colours::waiting())
        .child(format!("not shared any more (now a file of its own): {}", account.unshared.join(", ")))
}
