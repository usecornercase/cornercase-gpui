mod agents;
mod data;
mod profiles;

use std::collections::{HashMap, HashSet};

use gpui::{
    AnyView, App, Context, Div, Entity, Focusable as _, IntoElement, Render, Stateful, Window, div, prelude::*, px,
};

use self::data::AccountsInfo;
use crate::panels::PanelKind;
use crate::panels::settings::widgets;
use crate::store::{self, Store, args};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};

#[derive(Clone, PartialEq, Eq)]
enum Rename {
    Account(String, String),
    Profile(String),
}

#[derive(Clone, PartialEq, Eq)]
enum Pending {
    Remove(String, String),
    Delete(String),
}

#[derive(Default)]
struct Asking {
    list: bool,
    check: bool,
}

pub(crate) struct AccountsPanel {
    info: Option<AccountsInfo>,
    load_error: Option<String>,
    error: Option<String>,
    asking: Asking,
    busy: bool,
    add_fields: HashMap<String, Entity<TextField>>,
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
        let mut this = Self {
            info: None,
            load_error: None,
            error: None,
            asking: Asking::default(),
            busy: false,
            add_fields: HashMap::new(),
            no_login: HashSet::new(),
            rename: None,
            rename_field: Self::rename_field(cx),
            profile_field: Self::text_field("a new profile, such as Work", Self::new_profile, cx),
            pending: None,
            was_open: true,
        };
        this.load(cx);
        this
    }

    fn rename_field(cx: &mut Context<Self>) -> Entity<TextField> {
        let field = cx.new(|cx| TextField::new("a name; empty shows the folder's", cx));
        cx.subscribe(&field, |this, _, event: &TextFieldEvent, cx| match event {
            TextFieldEvent::Submit => this.submit_rename(cx),
            TextFieldEvent::Cancel => this.cancel_rename(cx),
            TextFieldEvent::Changed => cx.notify(),
        })
        .detach();
        field
    }

    fn text_field(
        placeholder: &'static str,
        submit: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> Entity<TextField> {
        let field = cx.new(|cx| TextField::new(placeholder, cx));
        cx.subscribe(&field, move |this, field, event: &TextFieldEvent, cx| match event {
            TextFieldEvent::Submit => submit(this, cx),
            TextFieldEvent::Cancel => field.update(cx, TextField::clear),
            TextFieldEvent::Changed => cx.notify(),
        })
        .detach();
        field
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        if self.asking.list {
            return;
        }
        self.asking.list = true;
        store::run(cx, args(&["account", "list", "--json"]), |this, result, cx| {
            this.asking.list = false;
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
        if self.asking.check {
            return;
        }
        self.asking.check = true;
        store::run(cx, args(&["account", "list", "--check", "--json"]), |this, result, cx| {
            this.asking.check = false;
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
            if !self.add_fields.contains_key(&provider.id) {
                let agent = provider.id.clone();
                let field = Self::text_field("a new account, such as work", move |this, cx| this.add(&agent, cx), cx);
                self.add_fields.insert(provider.id.clone(), field);
            }
        }
        self.info = Some(info);
        cx.notify();
    }

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
                Ok(out) => this.changed(&out, then, cx),
                Err(e) => this.error = Some(clean(&e)),
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

    fn add(&mut self, agent: &str, cx: &mut Context<Self>) {
        let Some(field) = self.add_fields.get(agent).cloned() else { return };
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
        let workspace = Store::get(cx).read(cx).session.as_ref().and_then(|s| s.shown().workspace);
        let login = if self.no_login.contains(agent) { None } else { workspace };
        self.change(data::add_args(agent, &name, login), cx, move |_, answer, cx| {
            field.update(cx, TextField::clear);
            show_login_tab(answer, cx);
        });
    }

    fn toggle_login(&mut self, agent: &str, cx: &mut Context<Self>) {
        if !self.no_login.remove(agent) {
            self.no_login.insert(agent.to_string());
        }
        cx.notify();
    }

    fn start_rename(&mut self, target: Rename, current: String, window: &mut Window, cx: &mut Context<Self>) {
        self.rename = Some(target);
        self.pending = None;
        self.rename_field.update(cx, |f, cx| f.set_text(current, cx));
        window.focus(&self.rename_field.read(cx).focus_handle(cx));
        cx.notify();
    }

    fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        self.rename = None;
        cx.notify();
    }

    fn submit_rename(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.rename.take() else { return };
        let label = self.rename_field.read(cx).text().trim().to_string();
        let command = match &target {
            Rename::Account(provider, account) => data::rename_args(provider, account, &label),
            Rename::Profile(name) if label.is_empty() || label == *name => {
                cx.notify();
                return;
            }
            Rename::Profile(name) => data::profile_rename_args(name, &label),
        };
        self.act(command, cx);
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

    fn cancel_pending(&mut self, cx: &mut Context<Self>) {
        self.pending = None;
        cx.notify();
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
            .child(ui::button("rename-cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| this.cancel_rename(cx))))
            .child(
                ui::primary_button("rename-save", "Save")
                    .on_click(cx.listener(|this, _, _, cx| this.submit_rename(cx))),
            )
    }

    fn pending_cancel(cx: &mut Context<Self>) -> Stateful<Div> {
        ui::button("pending-cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| this.cancel_pending(cx)))
    }

    fn page(&self, info: &AccountsInfo, window: &Window, cx: &mut Context<Self>) -> Div {
        let mut page = div().flex().flex_col().gap(px(14.)).max_w(px(900.)).child(widgets::heading(
            "Accounts",
            "Each agent can keep several logins, each in a folder of its own. New workspaces start with the \
             default one (●); a pane keeps the account it started with.",
        ));
        page = page
            .children((!info.tokens.is_empty()).then(|| tokens_note(&info.tokens)))
            .children(self.error.clone().map(widgets::error))
            .when(self.asking.check, |d| d.child(ui::dim("asking each agent whether it is signed in…")));
        for provider in &info.providers {
            page = page.child(self.render_provider(provider, window, cx));
        }
        page.child(self.render_profiles(info, window, cx))
    }
}

fn show_login_tab(answer: Option<AccountsInfo>, cx: &mut App) {
    let Some(tab) = answer.and_then(|info| info.login).and_then(|ids| ids.tab) else { return };
    Store::get(cx).update(cx, |s, cx| {
        s.focus("tab", tab);
        s.close_sheet(cx);
    });
}

fn marked_name(name: &str, active: bool, tag: &'static str) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(
            div().w(px(12.)).text_color(if active { colours::accent() } else { colours::faint() }).child(if active {
                "●"
            } else {
                "○"
            }),
        )
        .child(name.to_string())
        .when(active, |d| d.child(div().text_size(px(11.)).text_color(colours::accent()).child(tag)))
}

fn tokens_note(tokens: &[String]) -> Div {
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
            tokens.join(", "),
            if tokens.len() == 1 { "is" } else { "are" }
        ))
}

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
        div().size_full().child(widgets::page("accounts-page").child(self.page(&info, window, cx)))
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| AccountsPanel::new(window, cx)).into()
}
