use gpui::{AnyElement, Context, Div, Focusable as _, IntoElement, SharedString, Window, div, prelude::*, px};

use super::data::{self, AccountsInfo, Profile, Provider};
use super::{AccountsPanel, Pending, Rename, marked_name};
use crate::panels::settings::widgets;
use crate::theme::colours;
use crate::ui;

impl AccountsPanel {
    pub(super) fn render_profiles(
        &self,
        info: &AccountsInfo,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let mut card = widgets::card().child(self.no_profile_row(info.profile.is_empty(), cx));
        for profile in &info.profiles {
            card = card.child(self.render_profile(profile, &info.providers, window, cx));
        }
        card = card.child(self.new_profile_row(window, cx));
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

    fn no_profile_row(&self, none_active: bool, cx: &mut Context<Self>) -> Div {
        widgets::row(
            true,
            profile_label("No profile", none_active, "each agent starts with its default account, above"),
            div().when(!none_active && !self.busy, |d| {
                d.child(
                    ui::button("profile-use-none", "Use")
                        .on_click(cx.listener(|this, _, _, cx| this.act(data::profile_use_args(""), cx))),
                )
            }),
        )
    }

    fn new_profile_row(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let focused = self.profile_field.read(cx).focus_handle(cx).is_focused(window);
        widgets::row(
            false,
            ui::input_box(self.profile_field.clone(), focused).w(px(240.)).overflow_hidden(),
            ui::primary_button("profile-new", "Make profile").on_click(cx.listener(|this, _, _, cx| {
                if !this.busy {
                    this.new_profile(cx);
                }
            })),
        )
    }

    fn render_profile(
        &self,
        profile: &Profile,
        providers: &[Provider],
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let name = profile.name.clone();
        let rename = (self.rename == Some(Rename::Profile(name.clone()))).then(|| self.render_rename(window, cx));
        let choices: Vec<Div> = data::profile_accounts(profile, providers)
            .into_iter()
            .map(|(provider, chosen)| agent_choices(&name, provider, chosen, cx))
            .collect();
        let confirm = (self.pending == Some(Pending::Delete(name.clone()))).then(|| confirm_delete(&name, cx));
        div()
            .flex()
            .flex_col()
            .child(widgets::row(false, profile_label(&name, profile.active, ""), self.profile_buttons(profile, cx)))
            .children(rename)
            .child(div().flex().flex_col().gap(px(6.)).pl(px(20.)).pb(px(10.)).children(choices))
            .children(confirm)
            .into_any_element()
    }

    fn profile_buttons(&self, profile: &Profile, cx: &mut Context<Self>) -> Div {
        let buttons = div().flex().gap(px(6.));
        if self.busy {
            return buttons;
        }
        let name = profile.name.clone();
        let (used, renamed, deleted) = (name.clone(), name.clone(), name.clone());
        buttons
            .when(!profile.active, |d| {
                d.child(
                    ui::button(SharedString::from(format!("profile-use-{name}")), "Use")
                        .on_click(cx.listener(move |this, _, _, cx| this.act(data::profile_use_args(&used), cx))),
                )
            })
            .child(ui::button(SharedString::from(format!("profile-rename-{name}")), "Rename").on_click(cx.listener(
                move |this, _, window, cx| {
                    this.start_rename(Rename::Profile(renamed.clone()), renamed.clone(), window, cx);
                },
            )))
            .child(
                ui::button(SharedString::from(format!("profile-delete-{name}")), "Delete")
                    .on_click(cx.listener(move |this, _, _, cx| this.ask(Pending::Delete(deleted.clone()), cx))),
            )
    }
}

fn agent_choices(profile: &str, provider: &Provider, chosen: &str, cx: &mut Context<AccountsPanel>) -> Div {
    let folders = std::iter::once("").chain(provider.accounts.iter().map(|a| a.name.as_str()));
    let items = folders.map(|folder| choice(profile, provider, folder, folder == chosen, cx)).collect();
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .child(div().w(px(110.)).text_size(px(12.)).text_color(colours::muted()).child(provider.title.clone()))
        .child(ui::segments(items))
}

fn choice(
    profile: &str,
    provider: &Provider,
    folder: &str,
    selected: bool,
    cx: &mut Context<AccountsPanel>,
) -> AnyElement {
    let (name, agent, account) = (profile.to_string(), provider.id.clone(), folder.to_string());
    ui::segment(
        SharedString::from(format!("profile-{profile}-{agent}-{folder}")),
        provider.name_of(folder).to_string(),
        selected,
    )
    .on_click(cx.listener(move |this, _, _, cx| {
        if !this.busy {
            this.act(data::profile_set_args(&name, &agent, &account), cx);
        }
    }))
    .into_any_element()
}

fn confirm_delete(name: &str, cx: &mut Context<AccountsPanel>) -> Div {
    let deleted = name.to_string();
    widgets::confirm(
        format!("Delete the profile {name}? Its accounts stay."),
        AccountsPanel::pending_cancel(cx),
        ui::danger_button("pending-go", "Delete")
            .on_click(cx.listener(move |this, _, _, cx| this.act(data::profile_delete_args(&deleted), cx))),
    )
}

fn profile_label(name: &str, active: bool, note: &str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(marked_name(name, active, "in use"))
        .when(!note.is_empty(), |d| d.child(ui::dim(note.to_string()).pl(px(20.))))
}
