use gpui::{
    AnyElement, App, ClickEvent, Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    Stateful, StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
    transparent_black,
};

use super::{BotForm, KEEPS_AGENT, Listing, Mode, NO_AGENT, READING, read_failed};
use crate::cli::Feature;
use crate::model::bots::{
    AgentInfo, BotKey, CORNERCASE, FACE_COLOURS, FULL, IDEAS, about_note, face_of_name, unavailable_line,
};
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui;

const HERO: f32 = 68.;
const PICK: f32 = 22.;
const IDEA_FACE: f32 = 14.;
const THROUGH: &str = "Through cornercase";
const FULL_ACCESS: &str = "Full access";
const FULL_NOTE: &str = "Changes any file and runs anything, unattended.";
const TRY: &str = "Or start from an idea";

impl BotForm {
    pub(super) fn shown_face(&self, cx: &App) -> u8 {
        if let Some(face) = self.face {
            return face;
        }
        match &self.mode {
            Mode::Edit(editing) => Store::get(cx).read(cx).face_of(&BotKey::here(&editing.id)),
            Mode::New => face_of_name(&self.name.read(cx).text()),
        }
    }

    fn pick_face(&mut self, colour: u8, cx: &mut Context<Self>) {
        self.face = Some(colour);
        cx.notify();
    }

    pub(super) fn hero(&self, cx: &mut Context<Self>) -> Div {
        let shown = self.shown_face(cx);
        let picks = FACE_COLOURS.iter().enumerate().map(|(at, &colour)| {
            let ring = if colour == shown { colours::accent() } else { transparent_black().into() };
            div()
                .id(("bot-face", at))
                .p(px(2.))
                .rounded_full()
                .border_2()
                .border_color(ring)
                .cursor_pointer()
                .hover(|s| s.bg(colours::hover()))
                .child(ui::face::face(colour, PICK))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_face(colour, cx)))
        });
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(12.))
            .pb(px(4.))
            .child(ui::face::face(shown, HERO))
            .child(div().flex().flex_wrap().justify_center().gap(px(2.)).children(picks))
    }

    pub(super) fn agent_field(&self, cx: &mut Context<Self>) -> Div {
        let body = match &self.listing {
            Listing::Reading => ui::loading(READING).into_any_element(),
            Listing::Failed(words) => Self::agents_failed(words, cx),
            Listing::Read(list) => self.agent_body(list, cx),
        };
        ui::field("Agent", body)
    }

    fn agents_failed(words: &str, cx: &mut Context<Self>) -> AnyElement {
        let retry = ui::button("bot-agents-retry", "Try again")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.read_agents(cx)));
        let column = div().flex().flex_col().gap(px(6.)).child(ui::error_line(read_failed(words)));
        column.child(div().flex().child(retry)).into_any_element()
    }

    fn agent_body(&self, list: &[AgentInfo], cx: &mut Context<Self>) -> AnyElement {
        let Some(editing) = self.editing() else { return self.agent_chips(list, cx) };
        let label =
            list.iter().find(|a| a.agent == editing.agent).map_or_else(|| editing.agent.clone(), AgentInfo::label);
        let chips = ui::chips(vec![ui::resting_chip(("bot-agent", 0_usize), label, true)]);
        div().flex().flex_col().gap(px(6.)).child(chips).child(ui::dim(KEEPS_AGENT)).into_any_element()
    }

    fn agent_chips(&self, list: &[AgentInfo], cx: &mut Context<Self>) -> AnyElement {
        let items: Vec<Stateful<Div>> = list
            .iter()
            .enumerate()
            .filter(|(_, agent)| agent.bot)
            .map(|(at, agent)| {
                ui::chip(("bot-agent", at), agent.label(), self.chosen == Some(at))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_agent(at, cx)))
            })
            .collect();
        let none = self.chosen.is_none().then(|| ui::dim(NO_AGENT));
        let others = unavailable_line(list).map(|line| ui::dim(line).w_full());
        let column = div().flex().flex_col().gap(px(6.));
        column.when(!items.is_empty(), |d| d.child(ui::chips(items))).children(none).children(others).into_any_element()
    }

    pub(super) fn access_field(&self, agent: &AgentInfo, cx: &mut Context<Self>) -> Div {
        let through = self.access_card(agent, (THROUGH, agent.through_note(), CORNERCASE), cx);
        let full = self.access_card(agent, (FULL_ACCESS, FULL_NOTE, FULL), cx);
        let column = div().flex().flex_col().gap(px(6.)).child(div().flex().gap(px(8.)).child(through).child(full));
        ui::field("Access", column.children(agent.full_only_line().map(ui::dim)))
    }

    fn access_card(
        &self,
        agent: &AgentInfo,
        (title, line, value): (&'static str, &'static str, &'static str),
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let (is, offered, danger) = (self.access.as_deref() == Some(value), agent.offers(value), value == FULL);
        let edge = match (is, danger) {
            (true, true) => colours::failed(),
            (true, false) => colours::accent(),
            _ => colours::border(),
        };
        let card = div()
            .id(("bot-access", usize::from(danger)))
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(4.))
            .px(px(12.))
            .py(px(10.))
            .rounded(px(radius::card()))
            .border_1()
            .border_color(edge)
            .bg(if is { colours::selected() } else { colours::panel() })
            .child(access_title(title, is, danger))
            .child(access_line(line, danger));
        if !offered {
            return card.opacity(0.45);
        }
        let card = card.cursor_pointer().hover(|s| s.bg(colours::hover()));
        card.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_access(value, cx)))
    }

    pub(super) fn about_field(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let setup = Store::get(cx).read(cx).server_has(Feature::BotSetup);
        let note = about_note(setup);
        let empty = self.about.read(cx).text().trim().is_empty();
        let ideas = (note.is_some() && empty).then(|| Self::ideas(cx));
        let column = div().flex().flex_col().gap(px(6.)).child(ui::prompt_box(&self.about, "create", window, cx));
        ui::field("What it's for", column.children(ideas).children(note.map(|note| ui::dim(note).w_full())))
    }

    fn ideas(cx: &mut Context<Self>) -> Div {
        let chips = IDEAS.iter().enumerate().map(|(at, &(name, about))| {
            div()
                .id(("bot-form-idea", at))
                .flex()
                .items_center()
                .gap(px(6.))
                .pl(px(6.))
                .pr(px(10.))
                .py(px(3.))
                .rounded_full()
                .border_1()
                .border_color(colours::border())
                .text_size(px(12.))
                .cursor_pointer()
                .hover(|s| s.bg(colours::hover()))
                .child(ui::face::face(face_of_name(name), IDEA_FACE))
                .child(name)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.take_idea(name, about, cx)))
        });
        let label = div().text_size(px(11.)).text_color(colours::faint()).child(TRY);
        div().flex().flex_wrap().items_center().gap(px(6.)).child(label).children(chips)
    }

    fn take_idea(&mut self, name: &str, about: &str, cx: &mut Context<Self>) {
        if self.name.read(cx).text().trim().is_empty() {
            self.name.update(cx, |field, cx| field.set_text(name.to_string(), cx));
        }
        self.about.update(cx, |field, cx| field.set_text(about.to_string(), cx));
        self.changed(cx);
    }
}

fn access_title(title: &'static str, is: bool, danger: bool) -> Div {
    let tint = match (is, danger) {
        (true, true) => colours::failed(),
        (true, false) => colours::accent(),
        _ => colours::faint(),
    };
    let radio = div()
        .size(px(13.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .border_1()
        .border_color(tint)
        .when(is, |d| d.child(div().size(px(7.)).rounded_full().bg(tint)));
    div()
        .flex()
        .items_center()
        .gap(px(7.))
        .text_size(px(12.5))
        .font_weight(FontWeight::SEMIBOLD)
        .child(radio)
        .child(title)
}

fn access_line(line: &'static str, danger: bool) -> Div {
    let colour = if danger { colours::failed() } else { colours::muted() };
    div().w_full().text_size(px(11.5)).text_color(colour).child(line)
}
