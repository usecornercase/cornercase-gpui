use std::time::Instant;

use gpui::Context;

use super::held::{Asked, Held};
use super::taker::{
    Answer, Cause, Next, Taker, after_end, answer, keeps, may_take, next, orders_stay, refusal_words, split_closed,
    tells, unheard, untold, wait_after,
};
use super::{Store, ToastKind, later, rows};
use crate::cli;
use crate::model::tunnels::{Gate, Order, OrdersLine, Toward, Way, failed_args, orders_args, orders_gate, orders_line};
use crate::store;

impl Store {
    pub(crate) fn orders_gate_of(&self, toward: &Toward) -> Gate {
        self.release_gate(toward, orders_gate)
    }

    pub(crate) fn orders_notes(&self, towards: &[Toward]) -> Vec<String> {
        let gates: Vec<(String, Gate)> = towards
            .iter()
            .filter_map(|toward| {
                let label = match toward {
                    Toward::Server => cli::host()?,
                    Toward::Machine(name) => name.clone(),
                };
                Some((label, self.orders_gate_of(toward)))
            })
            .collect();
        rows::orders_notes(&gates)
    }

    fn taker_facts(&self, toward: &Toward) -> (bool, bool) {
        let gate = self.orders_gate_of(toward);
        let taker = self.tunnels.takers.get(toward);
        match toward {
            Toward::Server => {
                let cornercase = self.tunnel_way() == Way::Cornercase;
                (cornercase && self.tunnels.link.is_some() && gate == Gate::Open, orders_stay(cornercase, gate))
            }
            Toward::Machine(name) => {
                let listed = self.machines.iter().find(|machine| machine.name == *name);
                let shared = listed.is_some_and(|machine| {
                    let others = self.tunnels.takers.iter().filter(|(other, _)| *other != toward);
                    others.into_iter().any(|(_, other)| other.destination == machine.destination)
                });
                let linked = self.tunnels.machines.contains_key(name);
                let link = self.tunnel_link(name);
                let stays = taker.is_none_or(|taker| orders_stay(link.keeps_taker(&taker.destination), gate));
                (may_take(gate, linked, shared), stays)
            }
        }
    }

    pub(super) fn step_taker(&mut self, toward: &Toward, cause: Cause, cx: &mut Context<Self>) {
        let phase = self.tunnels.takers.get(toward).map(Taker::phase);
        match next(phase, cause, self.taker_facts(toward)) {
            Next::Start => self.start_taker(toward, cx),
            Next::Stop => drop(self.tunnels.takers.remove(toward)),
            Next::Nothing => {}
        }
    }

    pub(super) fn follow_takers(&mut self, cx: &mut Context<Self>) {
        for name in self.machines_followed() {
            self.step_taker(&Toward::Machine(name), Cause::Follow, cx);
        }
    }

    pub(super) fn follow_server_taker(&mut self, cx: &mut Context<Self>) {
        if cli::host().is_some() || self.tunnels.takers.contains_key(&Toward::Server) {
            self.step_taker(&Toward::Server, Cause::Follow, cx);
        }
    }

    pub(super) fn orders_linked(&mut self, toward: &Toward, cx: &mut Context<Self>) {
        self.step_taker(toward, Cause::Linked, cx);
    }

    fn start_taker(&mut self, toward: &Toward, cx: &mut Context<Self>) {
        let Ok(reached) = self.reached(toward) else { return };
        self.tunnels.next_taker += 1;
        let generation = self.tunnels.next_taker;
        let (heard, ended) = (toward.clone(), toward.clone());
        let stream = store::follow_on(
            cx,
            &reached.target,
            orders_args(),
            move |this, line, cx| this.orders_heard(&heard, generation, &line, cx),
            move |this, said, cx| this.orders_ended(&ended, generation, &said, cx),
        );
        let attempts = self.tunnels.takers.get(toward).map_or(0, |taker| taker.attempts);
        let told = self.tunnels.takers.get(toward).is_some_and(|taker| taker.told);
        let (destination, label, target) = (reached.destination, reached.label, reached.target);
        let taker = Taker { destination, label, target, stream, attempts, up_since: None, told, generation };
        self.tunnels.takers.insert(toward.clone(), taker);
    }

    fn current_taker(&mut self, toward: &Toward, generation: u64) -> Option<&mut Taker> {
        self.tunnels.takers.get_mut(toward).filter(|taker| taker.generation == generation)
    }

    fn orders_heard(&mut self, toward: &Toward, generation: u64, line: &str, cx: &mut Context<Self>) {
        let Some(taker) = self.current_taker(toward, generation) else { return };
        match orders_line(line) {
            Some(OrdersLine::Taking(_)) => taker.up_since = Some(Instant::now()),
            Some(OrdersLine::Order(Order::Open(port))) => self.ordered_open(toward, port, cx),
            Some(OrdersLine::Order(order @ (Order::Close(port) | Order::Forget(port)))) => {
                self.ordered_closed(toward, (order, port), cx);
            }
            None => {}
        }
    }

    fn orders_ended(&mut self, toward: &Toward, generation: u64, said: &str, cx: &mut Context<Self>) {
        let Some(taker) = self.current_taker(toward, generation) else { return };
        taker.stream = None;
        (taker.attempts, taker.told) =
            after_end(taker.attempts, taker.told, taker.up_since.take().map(|at| at.elapsed()));
        let wait = wait_after(taker.attempts);
        let tell = tells(taker.attempts, taker.told).then(|| unheard(toward, (&taker.label, &taker.destination), said));
        taker.told |= tell.is_some();
        if let Some(text) = tell {
            self.toast(text, ToastKind::Error, cx);
        }
        let toward = toward.clone();
        if let Toward::Machine(name) = &toward {
            self.there_failed(name, cx);
        }
        later(wait, move |this, cx| this.step_taker(&toward, Cause::Due { generation }, cx), cx);
    }

    fn cannot_hold(&self, toward: &Toward, destination: &str) -> Option<String> {
        if !self.can_tunnel(toward) {
            return Some(match toward {
                Toward::Server => "it is not connected to this server".to_string(),
                Toward::Machine(_) => self.tunnel_refusal(toward),
            });
        }
        let reached = self.reached(toward).ok()?;
        (reached.destination != destination).then(|| format!("{} now points at another address", reached.label))
    }

    fn ordered_open(&mut self, toward: &Toward, port: u16, cx: &mut Context<Self>) {
        let Some(destination) = self.tunnels.takers.get(toward).map(|taker| taker.destination.clone()) else { return };
        let cannot = self.cannot_hold(toward, &destination);
        let live = self.tunnels.holds.iter().filter(|h| h.toward == *toward && h.state != Held::Ending).count();
        let label = self.tunnels.takers.get(toward).map(|taker| taker.label.clone()).unwrap_or_default();
        match answer(self.held_tunnel(toward, port), live, cannot, (port, &label)) {
            Answer::Ignore => {}
            Answer::Hold => self.hold_asked(toward, port, cx),
            Answer::Again => {
                self.drop_down_tunnel(toward, port);
                self.hold_asked(toward, port, cx);
            }
            Answer::Refuse(why) => self.tell_failed(toward, port, &why, cx),
        }
    }

    fn hold_asked(&mut self, toward: &Toward, port: u16, cx: &mut Context<Self>) {
        if let Err(why) = self.start_hold(toward, port, (Asked::Program, None), cx) {
            let here = crate::desktop::here();
            let words = format!("the window on this {here} could not start cornercase tunnel hold: {why}");
            self.tell_failed(toward, port, &words, cx);
        }
    }

    fn tell_failed(&mut self, toward: &Toward, port: u16, why: &str, cx: &mut Context<Self>) {
        let Some(taker) = self.tunnels.takers.get(toward) else { return };
        let (target, destination, label) = (taker.target.clone(), taker.destination.clone(), taker.label.clone());
        let toward = toward.clone();
        let args = failed_args(port, &refusal_words(why)).to_vec();
        store::run_on(cx, target, args, move |this: &mut Store, result, cx| {
            if let Err(e) = result {
                let text = untold(&toward, (&label, &destination), port, &cli::reason(&e));
                this.toast(text, ToastKind::Error, cx);
            }
        });
    }

    fn ordered_closed(&mut self, toward: &Toward, (order, port): (Order, u16), cx: &mut Context<Self>) {
        let Some(destination) = self.tunnels.takers.get(toward).map(|taker| taker.destination.clone()) else { return };
        self.tunnels.settling.retain(|s| !(s.destination == destination && s.port == port));
        let (ended, kept) = split_closed(std::mem::take(&mut self.tunnels.holds), &destination, port);
        self.tunnels.holds = kept;
        self.end_holds_now(ended, cx);
        self.remember_tunnel(&destination, port, keeps(order), cx);
        cx.notify();
    }
}
