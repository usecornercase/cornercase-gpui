use std::thread;

use anyhow::Result;
use futures::channel::mpsc::UnboundedSender;
use serde_json::{Value, json};

use super::cdp::{self, Cdp, Event};
use super::chrome;
use super::frame::{self, Shot};

pub(crate) enum Incoming {
    Ready(Result<Engine, String>),
    Event(Event),
    Frame(String, Shot),
    Ended(String),
}

pub(crate) struct Engine {
    pub(crate) cdp: Cdp,
    pub(crate) port: u16,
}

pub(crate) fn start(tx: UnboundedSender<Incoming>) {
    thread::spawn(move || {
        let ready = open(tx.clone()).map_err(|e| format!("{e:#}"));
        let _ = tx.unbounded_send(Incoming::Ready(ready));
    });
}

fn open(tx: UnboundedSender<Incoming>) -> Result<Engine> {
    let endpoint = chrome::attach_or_launch()?;
    let (cdp, listener) = cdp::connect(&endpoint.url)?;
    let reader = cdp.clone();
    thread::spawn(move || {
        let why = listener.run(&reader, |event| route(&reader, &tx, event));
        let _ = tx.unbounded_send(Incoming::Ended(why));
    });
    Ok(Engine { cdp, port: endpoint.port })
}

fn route(cdp: &Cdp, tx: &UnboundedSender<Incoming>, event: Event) {
    if event.method != "Page.screencastFrame" {
        let _ = tx.unbounded_send(Incoming::Event(event));
        return;
    }
    let shot = frame::decode(&event.params);
    if let Some(id) = event.params.get("sessionId").and_then(Value::as_u64) {
        cdp.send(event.session.as_deref(), "Page.screencastFrameAck", json!({ "sessionId": id }));
    }
    if let (Some(session), Ok(shot)) = (event.session, shot) {
        let _ = tx.unbounded_send(Incoming::Frame(session, shot));
    }
}
