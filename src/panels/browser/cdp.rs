use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Result, anyhow, bail};
use futures::channel::oneshot;
use parking_lot::Mutex;
use serde_json::{Map, Value};

use super::socket::{self, Message, Reader, Writer};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Event {
    pub(crate) session: Option<String>,
    pub(crate) method: String,
    pub(crate) params: Value,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Parsed {
    Reply(u64, Result<Value, String>),
    Event(Event),
}

type Pending = HashMap<u64, oneshot::Sender<Result<Value>>>;

struct Inner {
    writer: Mutex<Option<Writer>>,
    next: AtomicU64,
    pending: Mutex<Pending>,
}

#[derive(Clone)]
pub(crate) struct Cdp {
    inner: Arc<Inner>,
}

pub(crate) struct Listener {
    reader: Reader,
}

pub(crate) fn connect(url: &str) -> Result<(Cdp, Listener)> {
    let (writer, reader) = socket::connect(url)?;
    let inner = Inner { writer: Mutex::new(Some(writer)), next: AtomicU64::new(1), pending: Mutex::default() };
    Ok((Cdp { inner: Arc::new(inner) }, Listener { reader }))
}

impl Cdp {
    pub(crate) fn send(&self, session: Option<&str>, method: &str, params: Value) {
        let id = self.inner.next.fetch_add(1, Ordering::Relaxed);
        let _ = self.write(&request(id, session, method, params));
    }

    pub(crate) fn call(&self, session: Option<&str>, method: &str, params: Value) -> oneshot::Receiver<Result<Value>> {
        let id = self.inner.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.inner.pending.lock().insert(id, tx);
        if let Err(e) = self.write(&request(id, session, method, params))
            && let Some(tx) = self.inner.pending.lock().remove(&id)
        {
            let _ = tx.send(Err(e));
        }
        rx
    }

    pub(crate) fn close(&self) {
        self.inner.writer.lock().take();
    }

    fn write(&self, text: &str) -> Result<()> {
        let mut writer = self.inner.writer.lock();
        let Some(socket) = writer.as_mut() else { bail!("the browser is not connected") };
        socket.text(text)
    }

    fn pong(&self, payload: &[u8]) {
        if let Some(socket) = self.inner.writer.lock().as_mut() {
            let _ = socket.pong(payload);
        }
    }

    fn dispatch(&self, text: &str, on_event: &mut impl FnMut(Event)) {
        match parse(text) {
            Some(Parsed::Event(event)) => on_event(event),
            Some(Parsed::Reply(id, result)) => {
                if let Some(tx) = self.inner.pending.lock().remove(&id) {
                    let _ = tx.send(result.map_err(|e| anyhow!(e)));
                }
            }
            None => {}
        }
    }
}

impl Listener {
    pub(crate) fn run(mut self, cdp: &Cdp, mut on_event: impl FnMut(Event)) -> String {
        let why = loop {
            match self.reader.message() {
                Ok(Message::Text(text)) => cdp.dispatch(&text, &mut on_event),
                Ok(Message::Ping(payload)) => cdp.pong(&payload),
                Ok(Message::Close) => break "the browser closed the connection".to_string(),
                Err(e) => break e.to_string(),
            }
        };
        cdp.inner.pending.lock().clear();
        cdp.close();
        why
    }
}

pub(crate) fn request(id: u64, session: Option<&str>, method: &str, params: Value) -> String {
    let mut message = Map::new();
    message.insert("id".into(), id.into());
    message.insert("method".into(), method.into());
    message.insert("params".into(), if params.is_null() { Value::Object(Map::new()) } else { params });
    if let Some(session) = session {
        message.insert("sessionId".into(), session.into());
    }
    Value::Object(message).to_string()
}

pub(crate) fn parse(text: &str) -> Option<Parsed> {
    let mut value: Value = serde_json::from_str(text).ok()?;
    let session = value.get("sessionId").and_then(Value::as_str).map(str::to_string);
    if let Some(id) = value.get("id").and_then(Value::as_u64) {
        let result = match value.get_mut("error") {
            Some(error) => Err(error.get("message").and_then(Value::as_str).unwrap_or("failed").to_string()),
            None => Ok(value.get_mut("result").map(Value::take).unwrap_or_default()),
        };
        return Some(Parsed::Reply(id, result));
    }
    let method = value.get("method")?.as_str()?.to_string();
    let params = value.get_mut("params").map(Value::take).unwrap_or_default();
    Some(Parsed::Event(Event { session, method, params }))
}

#[cfg(test)]
mod tests;
