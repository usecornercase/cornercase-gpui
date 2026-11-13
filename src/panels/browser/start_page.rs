use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, div, px,
};

use super::Browser;
use super::tunnels::{State, Tunnel};
use crate::cli;
use crate::model::Session;
use crate::model::link::{loopback_port, rewrite};
use crate::model::tunnels::{Toward, Way, older_note};
use crate::panels::ports::{self, data::Port};
use crate::store::Held;
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, icon};

const HINT: &str = "Type an address above, or just a port such as 5173.";
const NOTHING: &str = "Nothing in this workspace listens on a port yet. Start its dev server in a pane \
                       (npm run dev, cargo run…) and it shows here, and opens on its own.";
const NO_WORKSPACE: &str = "No workspace on screen: its dev servers show here once one is.";

pub(crate) enum Reach {
    Direct(String),
    Tunnelled { url: String, state: State },
    Opening,
    NotTunnelled,
    Closing,
}

pub(crate) fn held_reach(port: &Port, held: Option<&Held>) -> Reach {
    let url = port.url.clone().unwrap_or_default();
    match held {
        Some(Held::Up { local }) => Reach::Tunnelled { url: rewrite(&url, *local), state: State::Ready },
        Some(Held::Down { .. }) => Reach::Tunnelled { url, state: State::Retrying },
        Some(Held::Opening) => Reach::Opening,
        Some(Held::Ending) => Reach::Closing,
        None => Reach::NotTunnelled,
    }
}

pub(crate) fn reach(port: &Port, tunnel: Option<&Tunnel>, remote: bool) -> Reach {
    let url = port.url.clone().unwrap_or_default();
    match (remote, tunnel) {
        (false, _) => Reach::Direct(url),
        (true, Some(t)) => Reach::Tunnelled { url: rewrite(&url, t.local), state: t.state },
        (true, None) => Reach::Opening,
    }
}

pub(crate) fn shown_url(url: &str) -> &str {
    url.split_once("://").map_or(url, |(_, rest)| rest)
}

pub(crate) fn how_reached(port: &Port, reach: &Reach, host: &str) -> String {
    match reach {
        Reach::Direct(_) => String::new(),
        Reach::Tunnelled { state: State::Retrying, .. } => format!("via {host}: the tunnel stopped, trying again"),
        Reach::Tunnelled { url, .. } if loopback_port(url) == Some(port.number) => format!("via {host}"),
        Reach::Tunnelled { .. } => format!("via {host}, port {} there", port.number),
        Reach::Opening | Reach::NotTunnelled => format!("port {} on {host}", port.number),
        Reach::Closing => format!("closing the tunnel of port {} on {host}", port.number),
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
        let list = list.children(Self::older_line(cx));
        match (&self.dev.ports, &self.dev.error) {
            _ if self.workspace.is_none() => list.child(ui::dim(NO_WORKSPACE)),
            (_, Some(e)) => list.child(ui::dim(e.clone()).text_color(colours::failed())),
            (None, None) => list.child(ui::busy("browser-ports", "Looking for the workspace's dev servers…")),
            (Some(ports), None) if ports.is_empty() => list.child(ui::dim(NOTHING)),
            (Some(ports), None) => list.children(self.port_rows(ports, cx)),
        }
    }

    fn older_line(cx: &Context<Self>) -> Option<Div> {
        let host = cli::host()?;
        match Store::get(cx).read(cx).tunnel_way() {
            Way::Own(older) => Some(ui::dim(older_note(&older, &host))),
            _ => None,
        }
    }

    fn port_rows(&self, ports: &[Port], cx: &Context<Self>) -> Vec<AnyElement> {
        let host = cli::host();
        let store = Store::get(cx);
        let held = store.read(cx).tunnel_way() == Way::Cornercase;
        let session = store.read(cx).session.as_ref();
        let rows = ports.iter().map(|port| {
            let reach = if held {
                held_reach(port, store.read(cx).held_tunnel(&Toward::Server, port.number))
            } else {
                reach(port, self.dev.tunnels.get(port.number), host.is_some())
            };
            port_row(port, &reach, (session, host.as_deref().unwrap_or_default()), cx)
        });
        rows.collect()
    }
}

fn port_row(
    port: &Port,
    reach: &Reach,
    (session, host): (Option<&Session>, &str),
    cx: &Context<Browser>,
) -> AnyElement {
    let url = match reach {
        Reach::Direct(url) | Reach::Tunnelled { url, .. } => url.clone(),
        Reach::Opening | Reach::NotTunnelled | Reach::Closing => port.url.clone().unwrap_or_default(),
    };
    let opened = port.url.clone().unwrap_or_default();
    let closing = matches!(reach, Reach::Closing);
    let place = [port.place(session, false), how_reached(port, reach, host)];
    ui::row(SharedString::from(format!("browser-port-{}", port.number)), false)
        .p(px(12.))
        .gap(px(12.))
        .rounded(px(radius::card()))
        .bg(colours::panel())
        .border_1()
        .border_color(colours::border())
        .child(icon(Icon::Ports, 16.).flex_none().text_color(colours::accent()))
        .child(port_text(shown_url(&url), port, &place))
        .child(open_hint(port.number, reach))
        .children(close_button(port.number, reach, cx))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            if !closing {
                this.browse(opened.clone(), this.workspace, cx);
            }
        }))
        .into_any_element()
}

fn port_text(url: &str, port: &Port, place: &[String]) -> Div {
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
        .child(ports::command_line(port))
        .child(ui::dim(ui::one_line(place)))
}

fn close_button(number: u16, reach: &Reach, cx: &Context<Browser>) -> Option<AnyElement> {
    let held = Store::get(cx).read(cx).tunnel_way() == Way::Cornercase;
    let tunnelled = matches!(reach, Reach::Tunnelled { .. });
    (held && tunnelled).then(|| {
        let store = Store::get(cx);
        ui::button(SharedString::from(format!("browser-close-tunnel-{number}")), "Close tunnel")
            .on_click(move |_: &ClickEvent, _, cx| {
                cx.stop_propagation();
                store.update(cx, |s, cx| s.close_tunnel(&Toward::Server, number, false, cx));
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
