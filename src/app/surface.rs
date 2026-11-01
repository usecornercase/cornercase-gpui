use std::rc::Rc;
use std::time::Duration;

use gpui::Context;

use super::{Msg, Root, first_line};
use crate::cli::{self, Stream};
use crate::model::{Applied, Frame, Session};

pub(super) enum Feed {
    Off,
    Waiting,
    Open { _stream: Stream },
}

impl Root {
    fn open_surface(&mut self, view: u64, cx: &mut Context<Self>) {
        let generation = self.generation;
        let args = ["surface".to_string(), "--view".into(), view.to_string(), "--patches".into()];
        let (tx, end) = (self.tx.clone(), self.tx.clone());
        let stream = cli::stream(
            &args,
            move |line| match serde_json::from_str::<Frame>(&line) {
                Ok(f) => {
                    let _ = tx.unbounded_send(Msg::Frame(generation, Box::new(f)));
                }
                Err(e) => {
                    let _ = tx.unbounded_send(Msg::Error(format!("surface line: {e}")));
                }
            },
            move |err| {
                let _ = end.unbounded_send(Msg::SurfaceEnded(generation, err));
            },
        );
        match stream {
            Ok(s) => self.surface_stream = Feed::Open { _stream: s },
            Err(e) => {
                self.surface_stream = Feed::Off;
                self.say(&e.to_string(), cx);
            }
        }
    }

    pub(super) fn follow_surface(&mut self, session: &Session, cx: &mut Context<Self>) {
        if let Some(id) = session.view.as_ref().map(|v| v.id)
            && Some(id) != self.view_id
        {
            self.view_id = Some(id);
            self.surface_stream = Feed::Waiting;
            self.match_view_size(session);
        }
        let waiting = matches!(self.surface_stream, Feed::Waiting);
        if let Some(id) = self.view_id.filter(|_| waiting && session.shown().tab.is_some()) {
            self.open_surface(id, cx);
        }
    }

    fn match_view_size(&mut self, session: &Session) {
        if let Some((cols, rows)) = self.size
            && session.view.as_ref().is_some_and(|v| v.cols != Some(cols) || v.rows != Some(rows))
        {
            self.send_size(cols, rows);
        }
    }

    pub(super) fn frame_arrived(&mut self, frame: Frame) {
        if let Applied::NeedRedraw = Rc::make_mut(&mut self.grid).apply(frame) {
            let surface = self.grid.surface;
            self.queue.coalesced("redraw", vec!["redraw".into(), "--surface".into(), surface.to_string()]);
        } else {
            self.has_frame = true;
            self.surface_error = None;
        }
    }

    pub(super) fn surface_ended(&mut self, generation: u64, err: &str, cx: &mut Context<Self>) {
        self.has_frame = false;
        if self.session.as_ref().is_some_and(|s| s.shown().tab.is_none()) {
            self.surface_stream = Feed::Waiting;
            return;
        }
        let err = first_line(err.trim());
        let repeated = !err.is_empty() && self.surface_error.as_deref() == Some(err);
        if !err.is_empty() && !repeated {
            self.say(err, cx);
        }
        self.surface_error = (!err.is_empty()).then(|| err.to_string());
        let wait = if repeated { 5000 } else { 400 };
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(wait)).await;
            let _ = this.update(cx, |this, cx| this.reopen_surface(generation, cx));
        })
        .detach();
    }

    fn reopen_surface(&mut self, generation: u64, cx: &mut Context<Self>) {
        if self.generation == generation
            && matches!(self.surface_stream, Feed::Open { .. })
            && let Some(id) = self.view_id
        {
            self.open_surface(id, cx);
            cx.notify();
        }
    }
}
