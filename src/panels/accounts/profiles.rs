use gpui::{
    AnyElement, ClickEvent, Context, Div, Focusable as _, IntoElement, SharedString, Stateful, Window, div, prelude::*,
    px,
};

use super::AccountsPanel;
use super::data::{self, AccountsInfo, Profile, Provider, Target};
use super::widgets::{self, ROW_X};
use crate::theme::colours;
use crate::ui::{self, Icon, Tone};

const ABOUT: &str = "For scripts: a profile names an account per agent. While one is in use it is the default; \
                     making an account the default above leaves it.";
const NAME_NOTE: &str = "Scripts start with it through --profile, as in `cornercase start claude --profile Work`.";
const NO_PROFILE: &str = "Each agent starts with its default account, above";

fn id(what: &str, profile: &str) -> SharedString {
    SharedString::from(format!("profile-{what}-{profile}"))
}

fn agent_pick(profile: &str, provider: &Provider, chosen: &str, cx: &mut Context<AccountsPanel>) -> Div {
    let folders = std::iter::once("").chain(provider.accounts.iter().map(|a| a.name.as_str()));
    let items = folders.map(|folder| choice(profile, provider, folder, folder == chosen, cx)).collect();
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .child(
            div().w(px(110.)).flex_none().text_size(px(12.)).text_color(colours::muted()).child(provider.title.clone()),
        )
        .child(ui::segments(items).flex_wrap())
}

fn choice(
    profile: &str,
    provider: &Provider,
    folder: &str,
    selected: bool,
    cx: &mut Context<AccountsPanel>,
) -> AnyElement {
    let command = data::profile_set_args(profile, &provider.id, folder);
    let segment_id = format!("profile-{profile}-{}-{folder}", provider.id);
    ui::segment(SharedString::from(segment_id), provider.name_of(folder).to_string(), selected)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            if this.idle() {
                this.act(command.clone(), cx);
            }
        }))
        .into_any_element()
}

fn profile_body(profile: &Profile, providers: &[Provider], cx: &mut Context<AccountsPanel>) -> Div {
    let picks: Vec<Div> = data::profile_accounts(profile, providers)
        .into_iter()
        .map(|(provider, chosen)| agent_pick(&profile.name, provider, chosen, cx))
        .collect();
    let title = widgets::title(profile.name.clone()).children(profile.active.then(|| ui::accent_chip("in use")));
    div().flex_1().min_w_0().flex().flex_col().gap(px(8.)).child(title).children(picks)
}

impl AccountsPanel {
    pub(super) fn profiles_card(&self, info: &AccountsInfo, window: &Window, cx: &mut Context<Self>) -> Div {
        let mut rows: Vec<AnyElement> = Vec::new();
        if !info.profiles.is_empty() {
            rows.push(self.no_profile_row(info.profile.is_empty(), cx));
            rows.extend(info.profiles.iter().map(|p| self.profile_row(p, &info.providers, window, cx)));
        }
        widgets::card()
            .child(self.profiles_header(cx))
            .child(div().px(px(ROW_X)).pb(px(8.)).child(widgets::note(ABOUT)))
            .when(!rows.is_empty(), |d| d.child(widgets::rule()).children(rows))
            .children(self.new_profile_form(window, cx))
    }

    fn profiles_header(&self, cx: &mut Context<Self>) -> Div {
        let header = widgets::header(Icon::Accounts, "Profiles").child(div().flex_1());
        if self.form == Some(Target::NewProfile) {
            return header;
        }
        let button = ui::icon_text_button("profile-new", Icon::Plus, "New profile", Tone::Plain);
        if !self.idle() {
            return header.child(button.opacity(0.5).cursor_default());
        }
        header.child(
            button.on_click(
                cx.listener(|this, _: &ClickEvent, window, cx| this.open_form(Target::NewProfile, window, cx)),
            ),
        )
    }

    fn use_radio(&self, profile: &str, active: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let hint = match (profile.is_empty(), active) {
            (_, true) => "In use: new workspaces and agents start with these accounts",
            (true, false) => "Use no profile: each agent starts with its default account",
            (false, false) => "Use this profile: new workspaces and agents start with its accounts",
        };
        let radio = widgets::radio(id("use", profile), active, hint, self.idle());
        if active || !self.idle() {
            return radio;
        }
        let command = data::profile_use_args(profile);
        radio.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.act(command.clone(), cx)))
    }

    fn no_profile_row(&self, active: bool, cx: &mut Context<Self>) -> AnyElement {
        let target = Target::Profile(String::new());
        let doing = self.running_on(&target);
        let title = widgets::title("No profile").children(active.then(|| ui::accent_chip("in use")));
        let line = widgets::row("profile-none", doing.is_none())
            .child(widgets::lead(self.use_radio("", active, cx)))
            .child(widgets::name_column(title, NO_PROFILE))
            .children(doing.map(|d| ui::busy("profile-none-busy", d)))
            .child(widgets::actions());
        self.settled(line, None, &target)
    }

    fn profile_row(
        &self,
        profile: &Profile,
        providers: &[Provider],
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let name = profile.name.clone();
        let target = Target::Profile(name.clone());
        let radio = self.use_radio(&name, profile.active, cx);
        if self.rename.as_ref() == Some(&target) {
            return self.rename_row(format!("profile-renaming-{name}"), radio, window, cx).into_any_element();
        }
        let doing = self.running_on(&target);
        let line = widgets::row(id("row", &name), doing.is_none())
            .items_start()
            .child(widgets::lead(radio).pt(px(1.)))
            .child(profile_body(profile, providers, cx))
            .children(doing.map(|d| ui::busy(id("busy", &name), d)))
            .child(self.profile_actions(&name, cx));
        self.settled(line, None, &target)
    }

    fn profile_actions(&self, name: &str, cx: &mut Context<Self>) -> Div {
        if !self.idle() {
            return widgets::actions().child(ui::resting_action(Icon::Pencil)).child(ui::resting_action(Icon::Trash));
        }
        let (renamed, ask) = (name.to_string(), data::delete_ask(name));
        let rename = ui::row_action(id("rename", name), Icon::Pencil, "Rename", false).on_click(cx.listener(
            move |this, _: &ClickEvent, window, cx| {
                this.start_rename(Target::Profile(renamed.clone()), renamed.clone(), window, cx);
            },
        ));
        let delete = ui::row_action(id("delete", name), Icon::Trash, "Delete…", true)
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.ask(ask.clone(), window, cx)));
        widgets::actions().child(rename).child(delete)
    }

    fn new_profile_form(&self, window: &Window, cx: &mut Context<Self>) -> Option<Div> {
        if self.form != Some(Target::NewProfile) {
            return None;
        }
        let focused = self.profile_field.read(cx).focus_handle(cx).is_focused(window);
        let line = widgets::form_line()
            .child(ui::input_box(self.profile_field.clone(), focused).w(px(260.)).overflow_hidden())
            .child(div().flex_1())
            .child(self.form_buttons(Target::NewProfile, "Make profile", "Making…", cx));
        Some(widgets::form().child(line).child(self.form_note(&Target::NewProfile, NAME_NOTE)))
    }
}
