use gpui::{
    AnyElement, ClickEvent, Context, Div, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px, relative,
};

use super::arrange::divider;
use super::head_row;
use super::rail::count_disc;
use super::rows::{chevron, list_text};
use super::workspace::agent_text;
use crate::app::Root;
use crate::model::bots::{Bot, Offer};
use crate::theme::colours;
use crate::ui::{self, Icon, icon, orbits};

impl Root {
    pub(super) fn bots_section(&self, cx: &mut Context<Self>) -> Option<Div> {
        let store = self.store.read(cx);
        let offer = store.bots_offer();
        if offer == Offer::Hidden {
            return None;
        }
        let folded = self.side.bots_folded;
        let (thinking, unread) = (store.bots.thinking(), store.bots.unread() as u64);
        let body = match offer {
            Offer::Lacks(text) => Some(ui::note(text).into_any_element()),
            _ if folded => None,
            _ => Some(self.bot_rows(cx)),
        };
        Some(
            div()
                .flex_none()
                .max_h(relative(0.5))
                .flex()
                .flex_col()
                .child(divider())
                .child(Self::bots_head(folded, thinking, unread, cx))
                .children(body),
        )
    }

    fn bots_head(folded: bool, thinking: bool, unread: u64, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let still = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .when(folded && thinking, |d| d.child(ui::spinner("bots-thinking", 11., colours::working())))
            .children(count_disc(unread).filter(|_| folded));
        let chevron = chevron(SharedString::from("bots-chevron"), !folded);
        head_row(6., chevron, "Bots", still).id("sidebar-bots-head").cursor_pointer().on_click(cx.listener(
            |this, _: &ClickEvent, _, cx| {
                this.side.bots_folded = !this.side.bots_folded;
                cx.notify();
            },
        ))
    }

    fn bot_rows(&self, cx: &mut Context<Self>) -> AnyElement {
        let store = self.store.read(cx);
        let active = store.bots.active;
        let bots: Vec<Bot> = store.bots.list.clone();
        let rows: Vec<AnyElement> = bots
            .iter()
            .enumerate()
            .map(|(index, bot)| {
                let shown = self.side.bot.as_deref() == Some(bot.id.as_str());
                self.bot_row(index, bot, (shown, active), cx)
            })
            .collect();
        let list = list_text(div().flex().flex_col().gap(px(1.)).px(px(10.)).pb(px(4.))).children(rows);
        div().id("sidebar-bots").min_h_0().overflow_y_scroll().child(list).into_any_element()
    }

    fn bot_row(&self, index: usize, bot: &Bot, on_screen: (bool, bool), cx: &mut Context<Self>) -> AnyElement {
        let id = bot.id.clone();
        let lead = if bot.thinking() {
            orbits::badge(Some("working"), &self.parts.spots, 1.)
        } else {
            ui::badge_slot().child(icon(Icon::Message, 12.).text_color(colours::faint()))
        };
        let unread = u64::try_from(bot.unread).unwrap_or(u64::MAX);
        let (shown, active) = on_screen;
        let disc = count_disc(unread).filter(|_| !(shown && active));
        ui::row(("sidebar-bot", index), shown)
            .gap(px(4.))
            .px(px(6.))
            .py(px(3.))
            .child(lead)
            .child(agent_text(bot.name.clone(), bot.detail(), shown))
            .children(disc.map(gpui::Styled::flex_none))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.store.update(cx, |s, cx| s.show_bot(&id, cx));
            }))
            .into_any_element()
    }
}
