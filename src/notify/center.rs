use std::collections::VecDeque;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures::StreamExt as _;
use futures::channel::mpsc;
use gpui::App;

use crate::platform::Os;
use crate::store::{Store, ToastGo};

#[cfg(target_os = "macos")]
mod cocoa;
#[cfg(not(target_os = "macos"))]
#[path = "center/no_cocoa.rs"]
mod cocoa;

const KEEP: usize = 64;
const ANSWER_WITHIN: Duration = Duration::from_secs(30);
const TURN_ON: &str = "turn on Allow notifications in System Settings › Notifications › cornercase, or set desktop notifications to off in Settings › UI";
const NO_BUNDLE: &str = "the app's own notifications need its bundle";
static NEXT: AtomicU64 = AtomicU64::new(1);
static AVAILABLE: OnceLock<bool> = OnceLock::new();

fn uses_center(os: Os, bundle: Option<&str>) -> bool {
    os == Os::Mac && bundle.is_some_and(|id| !id.is_empty())
}

pub(super) fn available() -> bool {
    *AVAILABLE.get_or_init(|| uses_center(Os::CURRENT, cocoa::bundle_id().as_deref()))
}

#[derive(Default)]
pub(crate) struct Posted(VecDeque<(String, Option<ToastGo>)>);

impl Posted {
    pub(super) fn remember(&mut self, go: Option<ToastGo>) -> String {
        let id = format!("{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed));
        self.0.push_back((id.clone(), go));
        while self.0.len() > KEEP {
            self.0.pop_front();
        }
        id
    }

    fn take(&mut self, id: &str) -> Option<ToastGo> {
        let at = self.0.iter().position(|(known, _)| known == id)?;
        self.0.remove(at).and_then(|(_, go)| go)
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Refusal {
    allowed: bool,
    error: Option<String>,
}

impl Refusal {
    fn why(&self) -> String {
        let error = self.error.as_deref().map(str::trim).filter(|error| !error.is_empty());
        match (self.allowed, error) {
            (true, Some(error)) => error.to_string(),
            (true, None) => "macOS did not post it".to_string(),
            (false, Some(error)) => format!("{error}; {TURN_ON}"),
            (false, None) => format!("macOS does not let cornercase notify you; {TURN_ON}"),
        }
    }
}

pub(super) fn post((id, title, body): (&str, &str, &str)) -> Result<(), String> {
    if !available() {
        return Err(NO_BUNDLE.to_string());
    }
    cocoa::post((id, title, body), ANSWER_WITHIN).map_err(|refusal| refusal.why())
}

pub(crate) fn listen(cx: &mut App) {
    if !available() {
        return;
    }
    let (clicks, mut opened) = mpsc::unbounded::<String>();
    cocoa::install(&clicks);
    cx.spawn(async move |cx| {
        while let Some(id) = opened.next().await
            && cx.update(|cx| clicked(&id, cx)).is_ok()
        {}
    })
    .detach();
}

fn clicked(id: &str, cx: &mut App) {
    cx.activate(true);
    if let Some(window) = cx.windows().into_iter().next() {
        let _ = window.update(cx, |_, window, _| window.activate_window());
    }
    Store::get(cx).update(cx, |s, cx| {
        if let Some(action) = s.posted.take(id) {
            s.go(&action, cx);
        }
    });
}

#[cfg(test)]
mod tests;
