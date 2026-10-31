mod feed;
mod update;

use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, AppContext as _, ClickEvent, ClipboardItem, Context, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Rgba, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    div, px,
};

use crate::cli::Stream;
use crate::model::Session;
use crate::store::{self, Busy, Store, ToastKind, args};
use crate::theme::{colours, radius};
use crate::ui::{self, glass};

pub(crate) use feed::{AppState, Event, Notice, Prefs};

const FLASH: Duration = Duration::from_millis(160);
const BELL_TOAST_EVERY: Duration = Duration::from_secs(30);
const SOUND_EVERY: Duration = Duration::from_secs(1);
const RETRY: Duration = Duration::from_secs(3);

#[derive(Clone, PartialEq)]
pub(crate) enum InstallState {
    Idle,
    Running,
    Done(String),
    Failed(String),
}

pub(crate) struct Notifier {
    store: Entity<Store>,
    events: Option<Stream>,
    generation: u64,
    boot: Option<String>,
    told_failure: bool,
    pub app: AppState,
    app_key: Option<(Option<String>, u64)>,
    active: bool,
    last_sound: Option<Instant>,
    bell_toasts: HashMap<u64, Instant>,
    flash: u64,
    flashing: bool,
    pub install: InstallState,
    restart: Option<Stream>,
}

impl Notifier {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&store, |this, _, cx| this.store_changed(cx)).detach();
        cx.observe_window_activation(window, |this, window, _| this.active = window.is_window_active()).detach();
        Self {
            store,
            events: None,
            generation: 0,
            boot: None,
            told_failure: false,
            app: AppState::default(),
            app_key: None,
            active: window.is_window_active(),
            last_sound: None,
            bell_toasts: HashMap::new(),
            flash: 0,
            flashing: false,
            install: InstallState::Idle,
            restart: None,
        }
    }

    fn store_changed(&mut self, cx: &mut Context<Self>) {
        let s = self.store.read(cx);
        let boot = s.session.as_ref().filter(|_| s.view_id.is_some()).map(|s| s.boot.clone());
        let app_key = s.app.as_ref().map(|v| {
            let boot = v.get("boot").and_then(|b| b.as_str()).map(str::to_string);
            (boot, v.get("revision").and_then(serde_json::Value::as_u64).unwrap_or(0))
        });
        let app = (app_key != self.app_key).then(|| s.app.as_ref().map(AppState::from_value).unwrap_or_default());

        if let Some(app) = app {
            self.app = app;
            self.app_key = app_key;
            cx.notify();
        }
        if boot.is_some() && boot != self.boot {
            if self.boot.is_some() {
                self.install = InstallState::Idle;
                self.restart = None;
                self.bell_toasts.clear();
            }
            self.boot = boot;
            self.follow(cx);
        }
    }

    fn follow(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        self.events = None;
        self.events = store::follow_until(
            cx,
            args(&["events", "--notices", "--host", "--json"]),
            |this, line, cx| this.on_line(&line, cx),
            move |this, err, cx| this.ended(generation, &err, cx),
        );
    }

    fn ended(&mut self, generation: u64, err: &str, cx: &mut Context<Self>) {
        if generation != self.generation {
            return;
        }
        self.events = None;
        let err = last_line(err);
        if !err.is_empty() && !self.told_failure {
            self.told_failure = true;
            self.store.update(cx, |s, cx| s.toast(format!("Agent notices stopped: {err}"), ToastKind::Error, cx));
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(RETRY).await;
            let _ = this.update(cx, |this, cx| this.retry(generation, cx));
        })
        .detach();
    }

    fn retry(&mut self, generation: u64, cx: &mut Context<Self>) {
        let connected = self.store.read(cx).view_id.is_some();
        if self.generation == generation && self.events.is_none() && connected {
            self.follow(cx);
        }
    }

    fn on_line(&mut self, line: &str, cx: &mut Context<Self>) {
        match feed::parse_event(line) {
            Some(Event::Notice(notice)) => self.notice(notice, cx),
            Some(Event::Clipboard { text, pane }) => self.clipboard(text, pane, cx),
            Some(Event::Bell { tab, pane, .. }) => self.bell(tab, pane, cx),
            Some(Event::Other) | None => {}
        }
    }

    fn notice(&mut self, notice: Notice, cx: &mut Context<Self>) {
        let go = ("Go".to_string(), args(&["go", &notice.token]));
        self.store.update(cx, |s, cx| s.toast_with(notice.message.clone(), notice.toast_kind(), Some(go), cx));
        let desktop = !self.active;
        if notice.sound.is_none() && !desktop {
            return;
        }
        store::run_json(cx, args(&["config", "--json"]), move |this, config, _| {
            let prefs = config.map(|c| Prefs::from_config(&c)).unwrap_or_default();
            if let Some(cue) = &notice.sound
                && this.last_sound.is_none_or(|t| t.elapsed() >= SOUND_EVERY)
            {
                this.last_sound = Some(Instant::now());
                let home = std::env::var("HOME").unwrap_or_default();
                spawn_quietly("afplay", &[prefs.sound_file(cue, &home)]);
            }
            if desktop && prefs.desktop {
                desktop_notification(&notice.headline(), &notice.message);
            }
        });
    }

    fn clipboard(&mut self, text: String, pane: Option<u64>, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        let program = pane.and_then(|id| {
            let session = self.store.read(cx).session.as_ref()?;
            let p = session.pane(id)?;
            p.agent.clone().or_else(|| p.program.clone())
        });
        let text = match program {
            Some(program) => format!("{program} copied to the clipboard"),
            None => "A program copied to the clipboard".to_string(),
        };
        self.store.update(cx, |s, cx| s.toast(text, ToastKind::Info, cx));
    }

    fn bell(&mut self, tab: Option<u64>, pane: Option<u64>, cx: &mut Context<Self>) {
        let session = self.store.read(cx).session.as_ref();
        let shown = session.and_then(|s| s.shown().tab);
        if tab.is_some() && tab == shown {
            self.flash(cx);
            return;
        }
        let key = pane.or(tab).unwrap_or(0);
        if self.bell_toasts.get(&key).is_some_and(|t| t.elapsed() < BELL_TOAST_EVERY) {
            return;
        }
        self.bell_toasts.insert(key, Instant::now());
        let place = tab.and_then(|t| session.and_then(|s| tab_place(s, t)));
        let text = place.map_or_else(|| "Bell in another tab".to_string(), |p| format!("Bell in {p}"));
        let action = tab.map(|t| ("Show".to_string(), args(&["focus", "--tab", &t.to_string()])));
        self.store.update(cx, |s, cx| s.toast_with(text, ToastKind::Info, action, cx));
    }

    fn flash(&mut self, cx: &mut Context<Self>) {
        self.flash += 1;
        let flash = self.flash;
        self.flashing = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FLASH).await;
            let _ = this.update(cx, |this, cx| this.end_flash(flash, cx));
        })
        .detach();
    }

    fn end_flash(&mut self, flash: u64, cx: &mut Context<Self>) {
        if self.flash == flash {
            self.flashing = false;
            cx.notify();
        }
    }

    pub(crate) fn flashing(&self) -> Option<u64> {
        self.flashing.then_some(self.flash)
    }

    pub(crate) fn install(&mut self, cx: &mut Context<Self>) {
        if self.install == InstallState::Running {
            return;
        }
        let version = self.app.release().map(|r| r.version.clone()).unwrap_or_default();
        self.install = InstallState::Running;
        cx.notify();
        store::run(cx, args(&["update"]), move |this, result, cx| {
            this.install = match result {
                Ok(_) => InstallState::Done(version),
                Err(e) => InstallState::Failed(e.to_string()),
            };
            cx.notify();
        });
    }

    pub(crate) fn installed_version(&self) -> Option<String> {
        if let InstallState::Done(v) = &self.install {
            Some(v.clone())
        } else {
            let update = self.app.update.as_ref()?;
            update.installed.then(|| update.release.as_ref().map(|r| r.version.clone())).flatten()
        }
    }

    pub(crate) fn restart(&mut self, when_idle: bool, cx: &mut Context<Self>) {
        if !when_idle {
            let busy = Busy::new("restart", "Restarting the server…");
            self.store.update(cx, |s, cx| s.run_aside(busy, args(&["restart", "--yes"]), cx));
            return;
        }
        self.restart = store::follow_until(
            cx,
            args(&["restart", "--when-idle", "--yes"]),
            |_, _, _| {},
            |this, err, cx| {
                this.restart = None;
                let err = last_line(&err);
                if !err.is_empty() {
                    this.store.update(cx, |s, cx| s.toast(err, ToastKind::Error, cx));
                }
                cx.notify();
            },
        );
        cx.notify();
    }

    fn cancel_restart(&mut self, cx: &mut Context<Self>) {
        self.restart = None;
        self.store.update(cx, |s, cx| s.toast("Restart cancelled", ToastKind::Info, cx));
        cx.notify();
    }

    pub(crate) fn status_items(&self, this: &Entity<Self>) -> Vec<AnyElement> {
        [self.update_item(this), self.away_item(), self.awake_item()].into_iter().flatten().collect()
    }

    fn update_item(&self, this: &Entity<Self>) -> Option<AnyElement> {
        let n = this.clone();
        if self.restart.is_some() {
            return Some(
                chip("restart-pending", "↻ restart pending", dot(Some(colours::working())), colours::working())
                    .tooltip(tip("Restarts once no agent is in the middle of a turn. Click to cancel."))
                    .on_click(move |_: &ClickEvent, _, cx| n.update(cx, Notifier::cancel_restart))
                    .into_any_element(),
            );
        }
        if let Some(version) = self.installed_version() {
            return Some(
                chip("restart", format!("↻ restart for {version}"), None, colours::accent())
                    .tooltip(tip(format!(
                        "cornercase {version} is installed; the server runs the old one until it restarts."
                    )))
                    .on_click(move |_: &ClickEvent, window, cx| update::show_restart(n.clone(), window, cx))
                    .into_any_element(),
            );
        }
        let release = self.app.release()?;
        let (label, lead) = match self.install {
            InstallState::Running => (
                format!("installing {}…", release.version),
                Some(ui::spinner("installing", 10., colours::accent()).into_any_element()),
            ),
            _ => (format!("↑ {}", release.version), None),
        };
        Some(
            chip("update", label, lead, colours::accent())
                .tooltip(tip(format!("cornercase {} is out: see what changed", release.version)))
                .on_click(move |_: &ClickEvent, window, cx| update::show_update(n.clone(), window, cx))
                .into_any_element(),
        )
    }

    fn away_item(&self) -> Option<AnyElement> {
        if !self.app.away_ready() {
            return None;
        }
        let on = self.app.away_on();
        let (light, text, hint, command) = if on.is_empty() {
            let ready: Vec<String> =
                self.app.away.iter().filter(|a| a.ready).map(|a| feed::platform_name(&a.platform)).collect();
            (
                None,
                colours::faint(),
                format!("Click to have agents message you on {}", ready.join(" and ")),
                args(&["away"]),
            )
        } else {
            let names: Vec<String> = on.iter().map(|p| feed::platform_name(p)).collect();
            (
                Some(colours::waiting()),
                colours::text(),
                format!("Agents message you on {} while you are away. Click to stop.", names.join(" and ")),
                args(&["away", "--off"]),
            )
        };
        Some(
            chip("away", "away", dot(light), text)
                .tooltip(tip(hint))
                .on_click(move |_: &ClickEvent, _, cx| {
                    let busy = Busy::new("away", "Changing away…");
                    Store::get(cx).update(cx, |s, cx| s.run_aside(busy, command.clone(), cx));
                })
                .into_any_element(),
        )
    }

    fn awake_item(&self) -> Option<AnyElement> {
        let awake = self.app.keep_awake.as_ref()?;
        if awake.held {
            let by = awake.by.as_deref().map(|b| format!(" by {b}")).unwrap_or_default();
            return Some(
                chip("awake", "awake", dot(Some(colours::done())), colours::muted())
                    .tooltip(tip(format!("The machine is kept awake{by} while agents work")))
                    .into_any_element(),
            );
        }
        let why = awake.unavailable.as_ref()?;
        Some(
            chip("awake", "awake ✗", None, colours::faint())
                .tooltip(tip(format!("Cannot keep the machine awake: {why}")))
                .into_any_element(),
        )
    }
}

fn last_line(err: &str) -> String {
    err.trim().lines().last().unwrap_or_default().to_string()
}

pub(crate) fn window_title(session: &Session) -> String {
    let shown = session.shown();
    let tab =
        session.projects.iter().flat_map(|p| &p.workspaces).flat_map(|w| &w.tabs).find(|t| Some(t.id) == shown.tab);
    let Some(tab) = tab else { return "cornercase".into() };
    let pane =
        shown.pane.and_then(|id| tab.panes.iter().find(|p| p.id == id)).or_else(|| tab.panes.iter().find(|p| p.active));
    let title = pane.and_then(|p| p.title.as_deref()).map(str::trim).filter(|t| !t.is_empty());
    format!("{} — cornercase", title.unwrap_or(&tab.name))
}

fn tab_place(session: &Session, tab: u64) -> Option<String> {
    session.projects.iter().find_map(|p| {
        p.workspaces
            .iter()
            .find_map(|w| w.tabs.iter().find(|t| t.id == tab).map(|t| format!("{} › {} › {}", p.name, w.name, t.name)))
    })
}

fn dot(colour: Option<Rgba>) -> Option<AnyElement> {
    let colour = colour?;
    Some(div().size(px(6.)).rounded_full().bg(colour).shadow(glass::glow(colour, 0.8, 5.)).into_any_element())
}

fn chip(
    id: &'static str,
    label: impl Into<SharedString>,
    lead: Option<AnyElement>,
    text: Rgba,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .gap(px(5.))
        .px(px(7.))
        .py(px(1.))
        .rounded(px(radius::CHIP))
        .text_color(text)
        .cursor_pointer()
        .hover(|s| s.bg(colours::hover()))
        .children(lead)
        .child(label.into())
}

pub(crate) fn tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_, cx| cx.new(|_| Tip(text.clone())).into()
}

struct Tip(SharedString);

impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        glass::popover()
            .max_w(px(320.))
            .px(px(9.))
            .py(px(6.))
            .text_size(px(11.))
            .text_color(colours::text())
            .font_family(".SystemUIFont")
            .child(self.0.clone())
    }
}

fn desktop_notification(title: &str, body: &str) {
    let script = ["on run argv", "display notification (item 2 of argv) with title (item 1 of argv)", "end run"];
    let mut a: Vec<String> = script.iter().flat_map(|line| ["-e".to_string(), (*line).to_string()]).collect();
    a.push(title.to_string());
    a.push(body.to_string());
    spawn_quietly("osascript", &a);
}

fn spawn_quietly(program: &'static str, a: &[String]) {
    let a = a.to_vec();
    std::thread::spawn(move || {
        let _ =
            Command::new(program).args(&a).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
    });
}

#[cfg(test)]
mod tests;
