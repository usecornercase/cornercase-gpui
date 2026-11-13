use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, div, px,
};

use super::Browser;
use super::tunnels::{State, Tunnel};
use crate::cli;
use crate::model::link::{loopback_port, rewrite};
use crate::model::tunnels::MachineOpen;
use crate::model::tunnels::{Gate, Toward, Way, gate_note, older_note};
use crate::model::{Aim, Owner};
use crate::panels::on_machine;
use crate::panels::ports::{self, Listed, PortView, views};
use crate::store::Held;
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, icon};

const HINT: &str = "Type an address above, or just a port such as 5173.";
const NOTHING: &str = "Nothing in this workspace listens on a port yet. Start its dev server in a pane \
                       (npm run dev, cargo run…) and it shows here, and opens on its own.";
const LOOKING: &str = "Looking for the workspace's dev servers…";
const NO_WORKSPACE: &str = "No workspace on screen: its dev servers show here once one is.";

pub(crate) enum Reach {
    Direct(String),
    Tunnelled { url: String, state: State },
    Opening,
    NotTunnelled,
    Closing,
}

pub(crate) fn held_reach(url: Option<&str>, held: Option<&Held>) -> Reach {
    let url = url.unwrap_or_default().to_string();
    match held {
        Some(Held::Up { local }) => Reach::Tunnelled { url: rewrite(&url, *local), state: State::Ready },
        Some(Held::Down { .. }) => Reach::Tunnelled { url, state: State::Retrying },
        Some(Held::Opening) => Reach::Opening,
        Some(Held::Ending) => Reach::Closing,
        None => Reach::NotTunnelled,
    }
}

pub(crate) fn reach(url: Option<&str>, tunnel: Option<&Tunnel>, remote: bool) -> Reach {
    let url = url.unwrap_or_default().to_string();
    match (remote, tunnel) {
        (false, _) => Reach::Direct(url),
        (true, Some(t)) => Reach::Tunnelled { url: rewrite(&url, t.local), state: t.state },
        (true, None) => Reach::Opening,
    }
}

pub(crate) fn card_acts(open: &MachineOpen) -> bool {
    *open == MachineOpen::Open
}

pub(crate) fn shown_url(url: &str) -> &str {
    url.split_once("://").map_or(url, |(_, rest)| rest)
}

pub(crate) fn how_reached(number: u16, reach: &Reach, host: &str) -> String {
    match reach {
        Reach::Direct(_) => String::new(),
        Reach::Tunnelled { state: State::Retrying, .. } => format!("via {host}: the tunnel stopped, trying again"),
        Reach::Tunnelled { url, .. } if loopback_port(url) == Some(number) => format!("via {host}"),
        Reach::Tunnelled { .. } => format!("via {host}, port {number} there"),
        Reach::Opening | Reach::NotTunnelled => format!("port {number} on {host}"),
        Reach::Closing => format!("closing the tunnel of port {number} on {host}"),
    }
}

impl Browser {
    pub(super) fn start_page(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div().id("browser-start").size_full().overflow_y_scroll().child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(10.))
                .px(px(20.))
                .py(px(28.))
                .child(ui::emblem(Icon::Globe))
                .child(div().text_color(colours::soft()).child("Nothing open in this workspace"))
                .child(ui::dim(HINT))
                .child(self.servers(cx)),
        )
    }

    fn servers(&self, cx: &mut Context<Self>) -> Div {
        let list = div().w_full().max_w(px(560.)).flex().flex_col().gap(px(8.)).child(ui::section("Dev servers"));
        match &self.place {
            Owner::Nobody => list.child(ui::dim(NO_WORKSPACE)),
            Owner::Here(_) => self.local_servers(list, cx),
            Owner::There(ws) => self.machine_servers(list, ws.machine(), cx),
        }
    }

    fn local_servers(&self, list: Div, cx: &mut Context<Self>) -> Div {
        let list = list.children(Self::older_line(cx));
        match (&self.dev.listed, &self.dev.error) {
            (_, Some(e)) => list.child(ui::dim(e.clone()).text_color(colours::failed())),
            (Listed::None, None) => list.child(ui::busy("browser-ports", LOOKING)),
            (_, None) => self.cards(list, None, cx),
        }
    }

    fn machine_servers(&self, list: Div, name: &str, cx: &mut Context<Self>) -> Div {
        let store = Store::get(cx).read(cx);
        let list = list.children(machine_older_line(store, name));
        let read = store.machine_read();
        match (&self.dev.listed, read) {
            (Listed::None, Some(read)) if on_machine::may_ask(&read, &self.dev.retry).is_err() => {
                list.child(ui::dim(on_machine::nothing_read(&read)))
            }
            (Listed::None, _) => list.child(ui::busy("browser-ports", LOOKING)),
            (_, read) => self.cards(list, read.as_ref().map(|read| &read.aim), cx),
        }
    }

    fn cards(&self, list: Div, aim: Option<&Aim>, cx: &mut Context<Self>) -> Div {
        let store = Store::get(cx).read(cx);
        let views = views(&self.dev.listed, store.session.as_ref(), aim, false);
        if views.is_empty() {
            return list.child(ui::dim(NOTHING));
        }
        list.children(views.iter().map(|view| self.card(view, cx)))
    }

    fn older_line(cx: &Context<Self>) -> Option<Div> {
        let host = cli::host()?;
        match Store::get(cx).read(cx).tunnel_way() {
            Way::Own(older) => Some(ui::dim(older_note(&older, &host))),
            _ => None,
        }
    }

    fn card(&self, view: &PortView, cx: &Context<Self>) -> AnyElement {
        let host = cli::host();
        let store = Store::get(cx).read(cx);
        let machine = self.place.machine();
        let held = match (machine, store.tunnel_way()) {
            (Some(name), _) => Some(Toward::Machine(name.to_string())),
            (None, Way::Cornercase) => Some(Toward::Server),
            (None, _) => None,
        };
        let reach = match &held {
            Some(toward) => held_reach(view.url.as_deref(), store.held_tunnel(toward, view.number)),
            None => reach(view.url.as_deref(), self.dev.tunnels.get(view.number), host.is_some()),
        };
        let label = machine.map(str::to_string).or(host).unwrap_or_default();
        let acts = machine.is_none_or(|name| card_acts(&store.machine_open(name)));
        let close = held.filter(|toward| store.can_tunnel(toward));
        port_row(view, (&reach, &label), (acts, close), self.place.clone(), cx)
    }
}

fn machine_older_line(store: &Store, name: &str) -> Option<Div> {
    match store.machine_tunnel_gate(name) {
        Gate::Older(older) => Some(ui::dim(gate_note(&older, name))),
        Gate::Open | Gate::Waiting => None,
    }
}

fn port_row(
    view: &PortView,
    (reach, label): (&Reach, &str),
    (acts, close): (bool, Option<Toward>),
    owner: Owner,
    cx: &Context<Browser>,
) -> AnyElement {
    let url = match reach {
        Reach::Direct(url) | Reach::Tunnelled { url, .. } => url.clone(),
        Reach::Opening | Reach::NotTunnelled | Reach::Closing => view.url.clone().unwrap_or_default(),
    };
    let opened = view.url.clone().unwrap_or_default();
    let closing = matches!(reach, Reach::Closing);
    let place = [view.place.clone(), how_reached(view.number, reach, label)];
    let row = card_row(SharedString::from(format!("browser-port-{}", view.number)), acts)
        .child(icon(Icon::Ports, 16.).flex_none().text_color(colours::accent()))
        .child(port_text(shown_url(&url), view, &place))
        .children(acts.then(|| open_hint(view.number, reach)))
        .children(close.and_then(|toward| close_button(view.number, reach, toward, cx)));
    if !acts {
        return row.into_any_element();
    }
    row.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
        if !closing {
            this.browse(opened.clone(), owner.clone(), cx);
        }
    }))
    .into_any_element()
}

pub(crate) fn card_row(id: SharedString, acts: bool) -> gpui::Stateful<Div> {
    let base = if acts {
        ui::row(id, false)
    } else {
        div().id(id).relative().flex().items_center().gap(px(8.)).cursor_default()
    };
    base.p(px(12.))
        .gap(px(12.))
        .rounded(px(radius::card()))
        .bg(colours::panel())
        .border_1()
        .border_color(colours::border())
}

fn port_text(url: &str, view: &PortView, place: &[String]) -> Div {
    let place = place.iter().filter(|p| !p.is_empty()).cloned().collect::<Vec<_>>().join(" · ");
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div().font_weight(FontWeight::SEMIBOLD).text_color(colours::accent()).child(ui::one_line(url.to_string())),
        )
        .child(ports::command_text(view.command_line.clone()))
        .child(ui::dim(ui::one_line(place)))
}

fn close_button(number: u16, reach: &Reach, toward: Toward, cx: &Context<Browser>) -> Option<AnyElement> {
    let tunnelled = matches!(reach, Reach::Tunnelled { .. });
    tunnelled.then(|| {
        let store = Store::get(cx);
        ui::button(SharedString::from(format!("browser-close-tunnel-{number}")), "Close tunnel")
            .on_click(move |_: &ClickEvent, _, cx| {
                cx.stop_propagation();
                store.update(cx, |s, cx| s.close_tunnel(&toward, number, false, cx));
            })
            .into_any_element()
    })
}

fn open_hint(number: u16, reach: &Reach) -> AnyElement {
    match reach {
        Reach::Closing => {
            ui::busy(SharedString::from(format!("browser-closing-{number}")), "Closing…").into_any_element()
        }
        Reach::NotTunnelled => div()
            .flex_none()
            .text_size(px(12.))
            .text_color(colours::accent())
            .child("Tunnel and open")
            .into_any_element(),
        Reach::Opening | Reach::Tunnelled { state: State::Starting, .. } => {
            ui::busy(SharedString::from(format!("browser-tunnel-{number}")), "Tunnelling…").into_any_element()
        }
        _ => div().flex_none().text_size(px(12.)).text_color(colours::accent()).child("Open").into_any_element(),
    }
}

#[cfg(test)]
mod tests;
