use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, Div, InteractiveElement as _, IntoElement, ParentElement as _, Stateful,
    StatefulInteractiveElement as _, Styled as _, div, px,
};

use super::agents::{AgentLine, Link, State, Stop, folds, link};
use super::rows::{Meta, blocks_of, line};
use super::words::{CLOSED, FOLD, GO_TO_AGENT, SHOW, SHOW_WHOLE, there};
use crate::model::bots::outline_text;
use crate::notify;
use crate::store::Store;
use crate::theme::{FONT_FAMILY, colours, radius};
use crate::ui::markdown::text as md;
use crate::ui::{self, Icon, press};

const INDENT: f32 = ui::BADGE_SLOT + 8.;
const FOLDED_LINES: u8 = 3;
const LINE_HEIGHT: f32 = 19.;

pub(super) fn agent(agent: &AgentLine, meta: &Meta) -> Div {
    let column = div().flex().flex_col().gap(px(4.)).child(header(agent, meta)).children(body(agent, meta));
    line(column)
}

fn icon(state: State) -> Div {
    match state {
        State::Started => ui::badge_slot().child(ui::icon(Icon::Play, 12.).text_color(colours::faint())),
        State::Finished => ui::still_badge(Some("done")),
        State::Failed => ui::still_badge(Some("failed")),
        State::Waiting => ui::still_badge(Some("waiting")),
    }
}

fn header(agent: &AgentLine, meta: &Meta) -> Div {
    let colour = if agent.state == State::Started { colours::muted() } else { colours::soft() };
    let words = div().flex_1().min_w_0().text_size(px(12.5)).text_color(colour).child(agent.words.clone());
    let tail = (agent.state != State::Waiting).then(|| link_or(agent, meta, show_press));
    div().flex().items_start().gap(px(8.)).child(icon(agent.state)).child(words).children(tail)
}

fn link_or(agent: &AgentLine, meta: &Meta, control: fn(u64, u64, &Meta) -> Stateful<Div>) -> AnyElement {
    match link(agent.pane, &meta.bot.home) {
        Link::Show(pane) => control(agent.id, pane, meta).into_any_element(),
        Link::Closed => faint(CLOSED.to_string()).into_any_element(),
        Link::There(machine) => faint(there(&machine)).into_any_element(),
    }
}

fn faint(words: String) -> Div {
    div().flex_none().text_size(px(11.5)).text_color(colours::faint()).child(words)
}

fn stopped(el: Stateful<Div>, stop: Stop, name: &str, meta: &Meta) -> Stateful<Div> {
    match meta.agent_stops.get(&stop) {
        Some(handle) => press::stop_at(el, handle, name.to_string(), meta.stops),
        None => el,
    }
}

fn showing(el: Stateful<Div>, id: u64, pane: u64, meta: &Meta) -> Stateful<Div> {
    let name = format!("bot-agent-show-{id}");
    stopped(el, Stop::Show(id), &name, meta).on_click(move |event: &ClickEvent, _, cx| {
        if press::pressed(event, &name) {
            Store::get(cx).update(cx, |store, cx| store.show_in_terminal("pane", pane, cx));
        }
    })
}

fn show_press(id: u64, pane: u64, meta: &Meta) -> Stateful<Div> {
    let press = div()
        .id(("bot-agent-show", id))
        .flex_none()
        .px(px(4.))
        .rounded(px(radius::chip()))
        .cursor_pointer()
        .text_size(px(11.5))
        .text_color(colours::accent())
        .hover(|s| s.bg(colours::hover()))
        .child(SHOW);
    showing(press, id, pane, meta)
}

fn body(agent: &AgentLine, meta: &Meta) -> Option<Div> {
    match agent.state {
        State::Waiting => Some(excerpt(agent, meta)),
        State::Finished | State::Failed if folds(agent) => Some(indented(fold(agent, meta).into_any_element())),
        _ => None,
    }
}

fn indented(inner: AnyElement) -> Div {
    div().pl(px(INDENT)).child(inner)
}

fn fold_stop(id: u64) -> String {
    format!("bot-agent-fold-{id}")
}

fn closing_links(meta: &Meta) -> md::OnLink {
    let open = meta.on_link.clone();
    Rc::new(move |url: &str, cx: &mut App| {
        open(url, cx);
        cx.stop_propagation();
    })
}

fn fold(agent: &AgentLine, meta: &Meta) -> Stateful<Div> {
    let (id, open) = (agent.id, meta.opened.contains(&agent.id));
    let shown = if open {
        let blocks = blocks_of(id, &agent.body, &meta.blocks);
        let parts = md::linked_blocks(&format!("bot-agent-{id}"), &blocks, &closing_links(meta));
        div().flex().flex_col().gap(px(6.)).children(parts)
    } else {
        ui::clamped(FOLDED_LINES, LINE_HEIGHT).child(outline_text(&agent.body))
    };
    let name = fold_stop(id);
    let chat = meta.chat.clone();
    let pressed = div()
        .id(("bot-agent-fold", id))
        .px(px(4.))
        .ml(px(-4.))
        .rounded(px(radius::chip()))
        .cursor_pointer()
        .text_size(px(12.))
        .text_color(colours::muted())
        .hover(|s| s.bg(colours::hover()))
        .tooltip(notify::tip(if open { FOLD } else { SHOW_WHOLE }))
        .child(shown);
    stopped(pressed, Stop::Fold(id), &name, meta).on_click(move |event: &ClickEvent, _, cx| {
        if press::pressed(event, &name) {
            let _ = chat.update(cx, |chat, cx| chat.open_steps(id, cx));
        }
    })
}

fn excerpt(agent: &AgentLine, meta: &Meta) -> Div {
    let rows = agent.body.lines().map(|row| div().child(ui::one_line(row.to_string())));
    let screen = div()
        .flex()
        .flex_col()
        .font_family(FONT_FAMILY)
        .text_size(px(11.5))
        .text_color(colours::faint())
        .children(rows);
    let after = div().flex().child(link_or(agent, meta, go_button));
    indented(div().flex().flex_col().gap(px(6.)).child(screen).child(after).into_any_element())
}

fn go_button(id: u64, pane: u64, meta: &Meta) -> Stateful<Div> {
    showing(ui::button(("bot-agent-go", id), GO_TO_AGENT), id, pane, meta)
}
