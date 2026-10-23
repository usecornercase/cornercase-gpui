//! The accounts agents log in with, and profiles (`cornercase account`, `cornercase profile`).
//! One section per agent that takes accounts: its own login and each account, with whether it is
//! signed in (`account list --check`, which asks each agent and can take a while, so the list shows
//! first and the logins follow), the default one marked `●`, names, adding (and logging in, in a
//! tab of that account) and removing. Profiles, a tool for scripts, come after.

mod data;

use std::collections::{HashMap, HashSet};

use gpui::{
    AnyElement, AnyView, Context, Entity, Focusable as _, IntoElement, Render, SharedString, Window, div, prelude::*,
    px,
};

use self::data::{Account, AccountsInfo, LoginState, Profile, Provider};
use crate::panels::PanelKind;
use crate::panels::settings::widgets;
use crate::store::{self, Store, args};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};

/// What the one rename field names.
#[derive(Clone, PartialEq, Eq)]
enum Rename {
    /// An agent's account (`""` its own login).
    Account(String, String),
    Profile(String),
}

/// A question asked under a row before something that cannot be undone.
#[derive(Clone, PartialEq, Eq)]
enum Pending {
    Remove(String, String),
    Delete(String),
}

pub(crate) struct AccountsPanel {
    info: Option<AccountsInfo>,
    load_error: Option<String>,
    /// Why the last change was refused.
    error: Option<String>,
    loading: bool,
    checking: bool,
    /// A change is running; the buttons wait for it.
    busy: bool,
    /// The new account's name field, per agent.
    add_fields: HashMap<String, Entity<TextField>>,
    /// Agents whose new account is not logged in at once.
    no_login: HashSet<String>,
    rename: Option<Rename>,
    rename_field: Entity<TextField>,
    profile_field: Entity<TextField>,
    pending: Option<Pending>,
    was_open: bool,
}

impl AccountsPanel {
    pub(crate) fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::get(cx);
        cx.observe(&store, |this, store, cx| {
            let open = store.read(cx).sheet == Some(PanelKind::Accounts);
            if open && !this.was_open {
                this.load(cx);
            }
            this.was_open = open;
        })
        .detach();
        let rename_field = cx.new(|cx| TextField::new("a name; empty shows the folder's", cx));
        cx.subscribe(&rename_field, |this, _, event: &TextFieldEvent, cx| match event {
            TextFieldEvent::Submit => this.submit_rename(cx),
            TextFieldEvent::Cancel => {
                this.rename = None;
                cx.notify();
            }
            TextFieldEvent::Changed => cx.notify(),
        })
        .detach();
        let profile_field = cx.new(|cx| TextField::new("a new profile, such as Work", cx));
        cx.subscribe(&profile_field, |this, field, event: &TextFieldEvent, cx| match event {
            TextFieldEvent::Submit => this.new_profile(cx),
            TextFieldEvent::Cancel => field.update(cx, TextField::clear),
            TextFieldEvent::Changed => cx.notify(),
        })
        .detach();
        let mut this = Self {
            info: None,
            load_error: None,
            error: None,
            loading: false,
            checking: false,
            busy: false,
            add_fields: HashMap::new(),
            no_login: HashSet::new(),
            rename: None,
            rename_field,
            profile_field,
            pending: None,
            was_open: true,
        };
        this.load(cx);
        this
    }

    /// Lists the accounts, then asks each agent whether they are signed in.
    fn load(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        store::run(cx, args(&["account", "list", "--json"]), |this, result, cx| {
            this.loading = false;
            match result.and_then(|out| data::parse(&out)) {
                Ok(info) => {
                    this.load_error = None;
                    this.show(info, cx);
                    this.check(cx);
                }
                Err(e) => this.load_error = Some(clean(&e)),
            }
            cx.notify();
        });
    }

    fn check(&mut self, cx: &mut Context<Self>) {
        if self.checking {
            return;
        }
        self.checking = true;
        store::run(cx, args(&["account", "list", "--check", "--json"]), |this, result, cx| {
            this.checking = false;
            // A failed check leaves the list as it is: the logins only stay unknown. A change made
            // meanwhile is newer than this answer, and lists again itself.
            if let Ok(info) = result.and_then(|out| data::parse(&out))
                && !this.busy
            {
                this.show(info, cx);
            }
            cx.notify();
        });
    }

    fn show(&mut self, info: AccountsInfo, cx: &mut Context<Self>) {
        for provider in &info.providers {
            if !self.add_fields.contains_key(&provider.provider) {
                let field = cx.new(|cx| TextField::new("a new account, such as work", cx));
                let agent = provider.provider.clone();
                cx.subscribe(&field, move |this, field, event: &TextFieldEvent, cx| match event {
                    TextFieldEvent::Submit => this.add(agent.clone(), cx),
                    TextFieldEvent::Cancel => field.update(cx, TextField::clear),
                    TextFieldEvent::Changed => cx.notify(),
                })
                .detach();
                self.add_fields.insert(provider.provider.clone(), field);
            }
        }
        self.info = Some(info);
        cx.notify();
    }

    /// Runs a change, then lists the accounts again (with their logins); `then` gets the change's
    /// own answer when it printed one (`--json`).
    fn change(
        &mut self,
        command: Vec<String>,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, Option<AccountsInfo>, &mut Context<Self>) + 'static,
    ) {
        self.busy = true;
        self.error = None;
        self.pending = None;
        cx.notify();
        store::run(cx, command, move |this, result, cx| {
            this.busy = false;
            match result {
                Ok(out) => {
                    let answer = data::parse(&out).ok();
                    if let Some(info) = &answer {
                        this.show(info.clone(), cx);
                    }
                    then(this, answer, cx);
                    this.load(cx);
                }
                Err(e) => this.error = Some(clean(&e)),
            }
            cx.notify();
        });
    }

    fn add(&mut self, provider: String, cx: &mut Context<Self>) {
        let Some(field) = self.add_fields.get(&provider).cloned() else { return };
        let name = field.read(cx).text().trim().to_string();
        if name.is_empty() {
            return;
        }
        if !data::valid_name(&name) {
            self.error =
                Some(format!("`{name}`: an account's name takes letters, digits, `.`, `_` and `-`, at most 32"));
            cx.notify();
            return;
        }
        // The login tab opens in the workspace this window shows, and is shown there.
        let workspace = Store::get(cx).read(cx).session.as_ref().and_then(|s| s.shown().workspace);
        let login = if self.no_login.contains(&provider) { None } else { workspace };
        self.change(data::add_args(&provider, &name, login), cx, move |_, answer, cx| {
            field.update(cx, TextField::clear);
            if let Some(tab) = answer.and_then(|info| info.login).and_then(|ids| ids.tab) {
                Store::get(cx).update(cx, |s, cx| {
                    s.focus("tab", tab);
                    s.close_sheet(cx);
                });
            }
        });
    }

    fn start_rename(&mut self, target: Rename, current: String, window: &mut Window, cx: &mut Context<Self>) {
        self.rename = Some(target);
        self.pending = None;
        self.rename_field.update(cx, |f, cx| f.set_text(current, cx));
        window.focus(&self.rename_field.read(cx).focus_handle(cx));
        cx.notify();
    }

    fn submit_rename(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.rename.take() else { return };
        let label = self.rename_field.read(cx).text().trim().to_string();
        let command = match &target {
            Rename::Account(provider, account) => data::rename_args(provider, account, &label),
            Rename::Profile(name) => {
                if label.is_empty() || label == *name {
                    cx.notify();
                    return;
                }
                data::profile_rename_args(name, &label)
            }
        };
        self.change(command, cx, |_, _, _| {});
    }

    fn new_profile(&mut self, cx: &mut Context<Self>) {
        let name = self.profile_field.read(cx).text().trim().to_string();
        if name.is_empty() {
            return;
        }
        let field = self.profile_field.clone();
        self.change(data::profile_new_args(&name), cx, move |_, _, cx| field.update(cx, TextField::clear));
    }

    fn ask(&mut self, pending: Pending, cx: &mut Context<Self>) {
        self.pending = Some(pending);
        self.rename = None;
        cx.notify();
    }

    // -----------------------------------------------------------------------------------------
    // Drawing.

    fn render_provider(&self, provider: &Provider, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let mut card = widgets::card();
        card = card.child(self.render_account(provider, None, true, window, cx));
        for account in &provider.accounts {
            card = card.child(self.render_account(provider, Some(account), false, window, cx));
        }
        card = card.child(self.render_add(provider, window, cx));
        div()
            .flex()
            .flex_col()
            .child(ui::section(format!("{}  ·  {}", provider.title, provider.env)))
            .child(card)
            .into_any_element()
    }

    /// The agent's own login (`account` None) or one of its accounts.
    fn render_account(
        &self,
        provider: &Provider,
        account: Option<&Account>,
        first: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let agent = provider.provider.clone();
        let folder = account.map(|a| a.name.clone()).unwrap_or_default();
        let active = provider.active == folder;
        let name = provider.name_of(&folder).to_string();
        let path = match account {
            Some(a) => a.path.clone(),
            None => provider.system_path.clone().unwrap_or_default(),
        };
        let login = match account {
            Some(a) => a.login.clone(),
            None => provider.system_login.clone(),
        };
        let mut details: Vec<AnyElement> = Vec::new();
        if account.is_some_and(|a| !a.label.is_empty()) {
            details.push(ui::dim(format!("folder {folder}")).into_any_element());
        } else if account.is_none() {
            details.push(ui::dim("its own login").into_any_element());
        }
        details.push(ui::dim(path).into_any_element());
        let id = format!("{agent}-{folder}");

        let label = div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .w(px(12.))
                            .text_color(if active { colours::accent() } else { colours::faint() })
                            .child(if active { "●" } else { "○" }),
                    )
                    .child(name.clone())
                    .when(active, |d| d.child(div().text_size(px(11.)).text_color(colours::accent()).child("default")))
                    .children(login.map(|l| {
                        let colour = match l.state {
                            LoginState::SignedIn => colours::done(),
                            LoginState::SignedOut | LoginState::Failed => colours::failed(),
                            LoginState::Unknown => colours::faint(),
                        };
                        div().text_size(px(12.)).text_color(colour).child(l.text())
                    })),
            )
            .child(div().pl(px(20.)).flex().gap(px(8.)).children(details))
            .children(account.filter(|a| !a.unshared.is_empty()).map(|a| {
                div()
                    .pl(px(20.))
                    .text_size(px(11.5))
                    .text_color(colours::waiting())
                    .child(format!("not shared any more (now a file of its own): {}", a.unshared.join(", ")))
            }));

        let mut buttons = div().flex().gap(px(6.));
        if !self.busy {
            if !active {
                let (agent, folder) = (agent.clone(), folder.clone());
                buttons = buttons.child(ui::button(SharedString::from(format!("use-{id}")), "Make default").on_click(
                    cx.listener(move |this, _, _, cx| this.change(data::use_args(&agent, &folder), cx, |_, _, _| {})),
                ));
            }
            let target = Rename::Account(agent.clone(), folder.clone());
            let current = match account {
                Some(a) => a.label.clone(),
                None => provider.system_label.clone(),
            };
            buttons = buttons.child(ui::button(SharedString::from(format!("rename-{id}")), "Rename").on_click(
                cx.listener(move |this, _, window, cx| this.start_rename(target.clone(), current.clone(), window, cx)),
            ));
            if account.is_some() {
                let pending = Pending::Remove(agent.clone(), folder.clone());
                buttons = buttons.child(
                    ui::button(SharedString::from(format!("remove-{id}")), "Remove")
                        .on_click(cx.listener(move |this, _, _, cx| this.ask(pending.clone(), cx))),
                );
            }
        }

        let renaming = self.rename == Some(Rename::Account(agent.clone(), folder.clone()));
        let confirm =
            (self.pending == Some(Pending::Remove(agent.clone(), folder.clone()))).then(|| {
                let (agent, folder) = (agent.clone(), folder.clone());
                widgets::confirm(
                    format!(
                        "Remove {name} and delete its folder? Panes and profiles that use it go back to the own login. \
                     The keychain item stays: to log out first, run the agent's logout in a tab of this account."
                    ),
                    ui::button("pending-cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| {
                        this.pending = None;
                        cx.notify();
                    })),
                    ui::danger_button("pending-go", "Remove").on_click(cx.listener(move |this, _, _, cx| {
                        this.change(data::remove_args(&agent, &folder), cx, |_, _, _| {})
                    })),
                )
            });
        div()
            .flex()
            .flex_col()
            .child(widgets::row(first, label, buttons))
            .when(renaming, |d| d.child(self.render_rename(window, cx)))
            .children(confirm)
            .into_any_element()
    }

    fn render_rename(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let focused = self.rename_field.read(cx).focus_handle(cx).is_focused(window);
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .pl(px(20.))
            .pb(px(9.))
            .child(ui::input_box(self.rename_field.clone(), focused).w(px(260.)).overflow_hidden())
            .child(ui::button("rename-cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| {
                this.rename = None;
                cx.notify();
            })))
            .child(
                ui::primary_button("rename-save", "Save")
                    .on_click(cx.listener(|this, _, _, cx| this.submit_rename(cx))),
            )
    }

    fn render_add(&self, provider: &Provider, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(field) = self.add_fields.get(&provider.provider) else { return div().into_any_element() };
        let focused = field.read(cx).focus_handle(cx).is_focused(window);
        let agent = provider.provider.clone();
        let login = !self.no_login.contains(&agent);
        let (a1, a2) = (agent.clone(), agent.clone());
        widgets::row(
            false,
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(ui::input_box(field.clone(), focused).w(px(240.)).overflow_hidden())
                .child(
                    ui::checkbox(SharedString::from(format!("login-{agent}")), login, "log in now, in a new tab")
                        .text_size(px(12.))
                        .text_color(colours::muted())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !this.no_login.remove(&a1) {
                                this.no_login.insert(a1.clone());
                            }
                            cx.notify();
                        })),
                ),
            ui::primary_button(SharedString::from(format!("add-{agent}")), "Add account")
                .when(self.busy, |d| d.opacity(0.5))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !this.busy {
                        this.add(a2.clone(), cx);
                    }
                })),
        )
        .into_any_element()
    }

    fn render_profiles(
        &self,
        info: &AccountsInfo,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let mut card = widgets::card();
        // No profile: each agent uses its default account.
        let none_active = info.profile.is_empty();
        card =
            card.child(widgets::row(
                true,
                profile_label("No profile", none_active, "each agent starts with its default account, above"),
                div().when(!none_active && !self.busy, |d| {
                    d.child(ui::button("profile-use-none", "Use").on_click(
                        cx.listener(|this, _, _, cx| this.change(data::profile_use_args(""), cx, |_, _, _| {})),
                    ))
                }),
            ));
        for profile in &info.profiles {
            card = card.child(self.render_profile(profile, &info.providers, window, cx));
        }
        let focused = self.profile_field.read(cx).focus_handle(cx).is_focused(window);
        card = card.child(widgets::row(
            false,
            ui::input_box(self.profile_field.clone(), focused).w(px(240.)).overflow_hidden(),
            ui::primary_button("profile-new", "Make profile").on_click(cx.listener(|this, _, _, cx| {
                if !this.busy {
                    this.new_profile(cx);
                }
            })),
        ));
        div()
            .flex()
            .flex_col()
            .child(ui::section("Profiles"))
            .child(
                div()
                    .pb(px(6.))
                    .text_size(px(12.))
                    .text_color(colours::muted())
                    .child("For scripts: an account per agent under one name. While one is in use it is the default; making an account the default above leaves it."),
            )
            .child(card)
    }

    fn render_profile(
        &self,
        profile: &Profile,
        providers: &[Provider],
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let name = profile.name.clone();
        let mut buttons = div().flex().gap(px(6.));
        if !self.busy {
            if !profile.active {
                let n = name.clone();
                buttons = buttons.child(ui::button(SharedString::from(format!("profile-use-{name}")), "Use").on_click(
                    cx.listener(move |this, _, _, cx| this.change(data::profile_use_args(&n), cx, |_, _, _| {})),
                ));
            }
            let n = name.clone();
            buttons = buttons.child(
                ui::button(SharedString::from(format!("profile-rename-{name}")), "Rename").on_click(cx.listener(
                    move |this, _, window, cx| this.start_rename(Rename::Profile(n.clone()), n.clone(), window, cx),
                )),
            );
            let n = name.clone();
            buttons = buttons.child(
                ui::button(SharedString::from(format!("profile-delete-{name}")), "Delete")
                    .on_click(cx.listener(move |this, _, _, cx| this.ask(Pending::Delete(n.clone()), cx))),
            );
        }
        // One row of choices per agent: its own login or one of its accounts.
        let rename = (self.rename == Some(Rename::Profile(name.clone()))).then(|| self.render_rename(window, cx));
        let choices: Vec<_> = data::profile_accounts(profile, providers)
            .into_iter()
            .map(|(provider, chosen)| {
                let options = std::iter::once(String::new()).chain(provider.accounts.iter().map(|a| a.name.clone()));
                let items = options
                    .map(|folder| {
                        let (n, agent) = (name.clone(), provider.provider.clone());
                        ui::segment(
                            SharedString::from(format!("profile-{name}-{agent}-{folder}")),
                            provider.name_of(&folder).to_string(),
                            folder == chosen,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !this.busy {
                                this.change(data::profile_set_args(&n, &agent, &folder), cx, |_, _, _| {});
                            }
                        }))
                        .into_any_element()
                    })
                    .collect();
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div().w(px(110.)).text_size(px(12.)).text_color(colours::muted()).child(provider.title.clone()),
                    )
                    .child(ui::segments(items))
            })
            .collect();
        let confirm = (self.pending == Some(Pending::Delete(name.clone()))).then(|| {
            let n = name.clone();
            widgets::confirm(
                format!("Delete the profile {name}? Its accounts stay."),
                ui::button("pending-cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| {
                    this.pending = None;
                    cx.notify();
                })),
                ui::danger_button("pending-go", "Delete").on_click(
                    cx.listener(move |this, _, _, cx| this.change(data::profile_delete_args(&n), cx, |_, _, _| {})),
                ),
            )
        });
        div()
            .flex()
            .flex_col()
            .child(widgets::row(false, profile_label(&name, profile.active, ""), buttons))
            .children(rename)
            .child(div().flex().flex_col().gap(px(6.)).pl(px(20.)).pb(px(10.)).children(choices))
            .children(confirm)
            .into_any_element()
    }
}

fn profile_label(name: &str, active: bool, note: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .w(px(12.))
                        .text_color(if active { colours::accent() } else { colours::faint() })
                        .child(if active { "●" } else { "○" }),
                )
                .child(name.to_string())
                .when(active, |d| d.child(div().text_size(px(11.)).text_color(colours::accent()).child("in use"))),
        )
        .when(!note.is_empty(), |d| d.child(ui::dim(note.to_string()).pl(px(20.))))
}

/// A failed command's message without `cornercase account: error:` around it.
fn clean(error: &anyhow::Error) -> String {
    let text = error.to_string();
    let text = match text.split_once(": ") {
        Some((head, rest)) if head.starts_with("cornercase ") => rest.to_string(),
        _ => text,
    };
    let text = text.strip_prefix("Error: ").or_else(|| text.strip_prefix("error: ")).unwrap_or(&text);
    text.trim().to_string()
}

impl Render for AccountsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(info) = self.info.clone() else {
            return div().size_full().child(match &self.load_error {
                Some(e) => ui::note(format!("Could not list the accounts: {e}")),
                None => ui::note("Listing the accounts…"),
            });
        };
        let mut page = div().flex().flex_col().gap(px(14.)).max_w(px(900.)).child(widgets::heading(
            "Accounts",
            "Each agent can keep several logins, each in a folder of its own. New workspaces start with the \
             default one (●); a pane keeps the account it started with.",
        ));
        if !info.tokens.is_empty() {
            page = page.child(
                div()
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(colours::waiting())
                    .text_size(px(12.))
                    .text_color(colours::waiting())
                    .child(format!(
                        "{} {} set where cornercase runs, and wins over every account.",
                        info.tokens.join(", "),
                        if info.tokens.len() == 1 { "is" } else { "are" }
                    )),
            );
        }
        if let Some(error) = &self.error {
            page = page.child(widgets::error(error.clone()));
        }
        if self.checking {
            page = page.child(ui::dim("asking each agent whether it is signed in…"));
        }
        for provider in &info.providers {
            page = page.child(self.render_provider(provider, window, cx));
        }
        page = page.child(self.render_profiles(&info, window, cx));
        div().size_full().child(widgets::page("accounts-page").child(page))
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut gpui::App) -> AnyView {
    cx.new(|cx| AccountsPanel::new(window, cx)).into()
}
