use gpui::Context;

use super::held::{Held, Hold, Reached, forgets_after_close, live, unsent};
use super::{Busy, Store, ToastKind};
use crate::cli;
use crate::model::machines::hide_destination;
use crate::model::tunnels::Toward;

struct Closing {
    toward: Toward,
    port: u16,
    forget: bool,
    label: String,
    destination: String,
}

impl Closing {
    fn verb(&self) -> &'static str {
        if self.forget { "forget" } else { "close" }
    }
}

fn closing_key(toward: &Toward, port: u16) -> String {
    format!("tunnel-close:{}:{port}", toward.key())
}

impl Store {
    fn live_index(&self, toward: &Toward, port: u16) -> Option<usize> {
        self.tunnels.holds.iter().position(|h| h.port == port && h.toward == *toward && h.state != Held::Ending)
    }

    pub(crate) fn close_tunnel(&mut self, toward: &Toward, port: u16, forget: bool, cx: &mut Context<Self>) {
        let ours = self.live_index(toward, port);
        match (self.reached(toward), ours) {
            (Ok(reached), ours) => self.close_through(&reached, ours, (port, forget), cx),
            (Err(_), Some(at)) if *toward != Toward::Server => self.close_here_only(at, port, cx),
            (Err(why), None) if *toward != Toward::Server => self.not_sent(&why, cx),
            (Err(_), _) => {}
        }
    }

    fn close_through(
        &mut self,
        reached: &Reached,
        ours: Option<usize>,
        (port, forget): (u16, bool),
        cx: &mut Context<Self>,
    ) {
        let closing = Closing {
            toward: reached.toward.clone(),
            port,
            forget,
            label: reached.label.clone(),
            destination: reached.destination.clone(),
        };
        let quick = ours.is_some_and(|at| !matches!(self.tunnels.holds[at].state, Held::Up { .. }));
        if let Some(at) = ours {
            if quick {
                self.remember_tunnel(&closing.destination, port, false, cx);
                return self.drop_hold(at, cx);
            }
            self.tunnels.holds[at].state = Held::Ending;
        }
        let text = format!("Closing the tunnel of port {port} on {}…", closing.label);
        let args = vec!["tunnel".to_string(), closing.verb().to_string(), port.to_string()];
        let target = reached.target.clone();
        let busy = Busy::new(closing_key(&closing.toward, port), text);
        let work = move || cli::run_on(&target, &args, None);
        self.run_then(busy, work, move |this, result, cx| this.closed(&closing, result, cx), cx);
        cx.notify();
    }

    fn close_here_only(&mut self, at: usize, port: u16, cx: &mut Context<Self>) {
        let hold = self.tunnels.holds[at].clone();
        self.remember_tunnel(&hold.destination, port, false, cx);
        self.tunnels.ended_unsent.extend(unsent(&hold));
        self.drop_hold(at, cx);
        self.toast(format!("Closed the tunnel of port {port} on {}", hold.label()), ToastKind::Info, cx);
    }

    pub(super) fn drop_hold(&mut self, at: usize, cx: &mut Context<Self>) {
        let hold = self.tunnels.holds.remove(at);
        self.end_holds_now(vec![hold], cx);
    }

    fn closed(&mut self, closing: &Closing, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        let (port, label) = (closing.port, &closing.label);
        match result {
            Ok(_) => {
                let still = live(&self.tunnels.holds, &closing.toward, port).is_some_and(|h| h.state != Held::Ending);
                if forgets_after_close(still) {
                    self.remember_tunnel(&closing.destination, port, false, cx);
                }
                let past = if closing.forget { "Forgot" } else { "Closed" };
                self.toast(format!("{past} the tunnel of port {port} on {label}"), ToastKind::Info, cx);
            }
            Err(e) => {
                let why = hide_destination(&cli::reason(&e), label, &closing.destination);
                let text = format!("Could not {} the tunnel of port {port} on {label}: {why}", closing.verb());
                self.toast(text, ToastKind::Error, cx);
                self.restore_ending(&closing.toward, port);
                if let Toward::Machine(name) = &closing.toward {
                    self.there_failed(name, cx);
                }
            }
        }
        cx.notify();
    }

    fn restore_ending(&mut self, toward: &Toward, port: u16) {
        let ending = |h: &&mut Hold| h.port == port && h.toward == *toward && h.state == Held::Ending;
        for hold in self.tunnels.holds.iter_mut().filter(ending) {
            if let (Some(local), true) = (hold.last_local, hold.process != 0) {
                hold.state = Held::Up { local };
            }
        }
    }
}
