use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use futures::channel::mpsc::UnboundedSender;
use serde::de::DeserializeOwned;

use super::{Heard, Msg};
use crate::cli::{self, Queue, Stream, Target};
use crate::model::{Applied, Frame, Grid, Session};

static NEXT_SERIAL: AtomicU64 = AtomicU64::new(1);

enum Feed {
    Off,
    Waiting,
    Open { _stream: Stream },
}

pub(crate) struct Connection {
    pub(crate) queue: Queue,
    pub(crate) view_id: Option<u64>,
    pub(crate) grid: Rc<Grid>,
    pub(crate) has_frame: bool,
    pub(crate) surface_error: Option<String>,
    pub(crate) size: Option<(u16, u16)>,
    target: Target,
    view_stream: Option<Stream>,
    feed: Feed,
    view_serial: u64,
    surface_serial: u64,
    queue_serial: u64,
}

fn next_serial() -> u64 {
    NEXT_SERIAL.fetch_add(1, Ordering::Relaxed)
}

fn heard_lines<T: DeserializeOwned + 'static>(
    target: &Target,
    args: &[String],
    tx: &UnboundedSender<Msg>,
    what: &'static str,
    (line, end): (impl Fn(Box<T>) -> Heard + Send + 'static, impl FnOnce(cli::StreamEnd) -> Heard + Send + 'static),
) -> Result<(Stream, u64)> {
    let serial = next_serial();
    let (lines, ended) = (tx.clone(), tx.clone());
    let stream = cli::stream_on(
        target,
        args,
        move |text| {
            let msg = match serde_json::from_str::<T>(&text) {
                Ok(item) => Msg::Stream(serial, line(Box::new(item))),
                Err(e) => Msg::Error(format!("{what} line: {e}")),
            };
            let _ = lines.unbounded_send(msg);
        },
        move |err| {
            let _ = ended.unbounded_send(Msg::Stream(serial, end(err)));
        },
    )?;
    Ok((stream, serial))
}

pub(super) fn surface_args(view: u64, boot: Option<&str>) -> Vec<String> {
    let args = ["surface".to_string(), "--view".into(), view.to_string(), "--patches".into()].to_vec();
    match boot {
        Some(boot) => cli::with_boot(args, boot),
        None => args,
    }
}

impl Connection {
    pub(super) fn new(target: Target, queue: Queue) -> Self {
        Self {
            queue,
            view_id: None,
            grid: Rc::new(Grid::default()),
            has_frame: false,
            surface_error: None,
            size: None,
            target,
            view_stream: None,
            feed: Feed::Off,
            view_serial: 0,
            surface_serial: 0,
            queue_serial: 0,
        }
    }

    pub(super) fn unopened(size: Option<(u16, u16)>) -> Self {
        Self { size, ..Self::new(Target::Local, Queue::nowhere()) }
    }

    pub(super) fn owns(&self, serial: u64) -> bool {
        serial != 0 && [self.view_serial, self.surface_serial, self.queue_serial].contains(&serial)
    }

    pub(super) fn queue_serial(&self) -> u64 {
        self.queue_serial
    }

    pub(super) fn target(&self) -> &Target {
        &self.target
    }

    pub(super) fn open_queue(&mut self, target: Target, run: (String, Option<String>), tx: &UnboundedSender<Msg>) {
        let serial = next_serial();
        let errors = tx.clone();
        self.queue = Queue::on(target.clone(), run, move |e| {
            let _ = errors.unbounded_send(Msg::Stream(serial, Heard::QueueError(e)));
        });
        self.queue_serial = serial;
        self.target = target;
    }

    pub(super) fn open_view(&mut self, args: &[String], tx: &UnboundedSender<Msg>) -> Result<()> {
        self.close();
        let (stream, serial) = heard_lines(&self.target, args, tx, "view", (Heard::Line, Heard::ViewEnded))?;
        self.view_stream = Some(stream);
        self.view_serial = serial;
        Ok(())
    }

    pub(super) fn open_surface(&mut self, view: u64, boot: Option<&str>, tx: &UnboundedSender<Msg>) -> Result<()> {
        let args = surface_args(view, boot);
        self.feed = Feed::Off;
        self.surface_serial = 0;
        let (stream, serial) = heard_lines(&self.target, &args, tx, "surface", (Heard::Frame, Heard::SurfaceEnded))?;
        self.feed = Feed::Open { _stream: stream };
        self.surface_serial = serial;
        Ok(())
    }

    pub(super) fn close(&mut self) {
        self.feed = Feed::Off;
        self.view_stream = None;
        self.view_id = None;
        self.view_serial = 0;
        self.surface_serial = 0;
    }

    pub(super) fn go_nowhere(&mut self) {
        self.close();
        self.queue = Queue::nowhere();
        self.queue_serial = 0;
    }

    pub(super) fn pause_surface(&mut self) {
        self.feed = Feed::Waiting;
        self.surface_serial = 0;
    }

    pub(super) fn surface_open(&self) -> bool {
        matches!(self.feed, Feed::Open { .. })
    }

    pub(super) fn saw_view(&mut self, session: &Session) -> bool {
        let Some(id) = session.view.as_ref().map(|v| v.id).filter(|id| Some(*id) != self.view_id) else {
            return false;
        };
        self.view_id = Some(id);
        self.feed = Feed::Waiting;
        self.match_view_size(session);
        true
    }

    pub(super) fn surface_due(&self, shows_tab: bool) -> Option<u64> {
        self.view_id.filter(|_| shows_tab && matches!(self.feed, Feed::Waiting))
    }

    fn match_view_size(&self, session: &Session) {
        if let Some(size) = self.size
            && session.view.as_ref().is_some_and(|v| (v.cols, v.rows) != (Some(size.0), Some(size.1)))
        {
            self.send_size(size);
        }
    }

    pub(super) fn send_size(&self, (cols, rows): (u16, u16)) {
        if let Some(id) = self.view_id {
            let args = vec![
                "view-set".into(),
                "--view".into(),
                id.to_string(),
                "--cols".into(),
                cols.to_string(),
                "--rows".into(),
                rows.to_string(),
            ];
            self.queue.coalesced("size", args);
        }
    }

    pub(super) fn tell_visible(&self, visible: bool) {
        if let Some(id) = self.view_id {
            let flag = if visible { "--visible" } else { "--hidden" };
            self.queue.cmd(vec!["view-set".into(), "--view".into(), id.to_string(), flag.into()]);
        }
    }

    pub(crate) fn act(&self, mut args: Vec<String>) {
        if let Some(id) = self.view_id {
            args.extend(["--view".to_string(), id.to_string()]);
            self.queue.cmd(args);
        }
    }

    pub(super) fn frame_arrived(&mut self, frame: Frame) -> bool {
        if let Applied::NeedRedraw = Rc::make_mut(&mut self.grid).apply(frame) {
            let surface = self.grid.surface;
            self.queue.coalesced("redraw", vec!["redraw".into(), "--surface".into(), surface.to_string()]);
            return false;
        }
        self.has_frame = true;
        self.surface_error = None;
        true
    }

    #[cfg(test)]
    pub(super) fn with_serials(view: u64, surface: u64) -> Self {
        Self { view_serial: view, surface_serial: surface, ..Self::unopened(None) }
    }
}

#[cfg(test)]
mod tests;
