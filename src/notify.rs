//! What cornercase tells the user outside the terminal: agent notices (a toast with a `Go`
//! button, a macOS notification while the window is in the background, the sound the server
//! chose), what programs ask of the host terminal (clipboard writes, bells), and the app's state
//! in the status bar (an update to install, away on chat apps, keeping the machine awake).
//!
//! It follows `cornercase events --notices --host --json` for the server run the view shows
//! (again after a reconnect), and reads `Store.app` (`status --app --follow`) for the status bar.

mod feed;
mod update;

use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, AppContext as _, ClickEvent, ClipboardItem, Context, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Rgba, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, prelude::FluentBuilder as _, px,
};

use crate::cli::Stream;
use crate::model::Session;
use crate::store::{self, Store, ToastKind, args};
use crate::theme::colours;

pub use feed::{AppState, Event, Notice, Prefs};

/// How long a bell flashes the terminal.
const FLASH: Duration = Duration::from_millis(160);
/// A pane out of sight ringing again within this long gives no second toast.
const BELL_TOAST_EVERY: Duration = Duration::from_secs(30);
/// Two sounds closer than this would overlap; the second is dropped, as cornercase's own player does.
const SOUND_EVERY: Duration = Duration::from_secs(1);
/// After `events` ends (the server stopped or refused), wait this long before following again.
const RETRY: Duration = Duration::from_secs(3);

/// Installing an update with `cornercase update`, from this window.
#[derive(Clone, PartialEq)]
pub enum InstallState {
    Idle,
    Running,
    /// Installed this version; the server runs the old one until it restarts.
    Done(String),
    Failed(String),
}

pub struct Notifier {
    store: Entity<Store>,
    events: Option<Stream>,
    /// Which `events` stream is the current one, so a replaced one ending is ignored.
    generation: u64,
    /// The server run (`boot`) the stream follows.
    boot: Option<String>,
    told_failure: bool,
    /// `Store.app` read, and the boot and revision it was read at.
    pub app: AppState,
    app_key: Option<(Option<String>, u64)>,
    /// Whether the window is the active one (no desktop notification then).
    active: bool,
    last_sound: Option<Instant>,
    bell_toasts: HashMap<u64, Instant>,
    flash: u64,
    flashing: bool,
    pub install: InstallState,
    /// `cornercase restart --when-idle` waiting; dropping it cancels the restart.
    restart: Option<Stream>,
}

impl Notifier {
    pub fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
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

    /// Follows the server our view is on, again when it restarted, and rereads the app state.
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
                // A new server run: what this window installed is now running, or gone.
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
            move |this, err, cx| this.ended(generation, err, cx),
        );
    }

    /// `events` stopped: the server restarted or went away (the view reconnects, and a new boot
    /// follows again), or it refused. Try again in a moment while we are connected.
    fn ended(&mut self, generation: u64, err: String, cx: &mut Context<Self>) {
        if generation != self.generation {
            return;
        }
        self.events = None;
        let err = err.trim().lines().last().unwrap_or_default().to_string();
        if !err.is_empty() && !self.told_failure {
            self.told_failure = true;
            self.store.update(cx, |s, cx| s.toast(format!("Agent notices stopped: {err}"), ToastKind::Error, cx));
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(RETRY).await;
            let _ = this.update(cx, |this, cx| {
                let connected = this.store.read(cx).view_id.is_some();
                if this.generation == generation && this.events.is_none() && connected {
                    this.follow(cx);
                }
            });
        })
        .detach();
    }

    fn on_line(&mut self, line: &str, cx: &mut Context<Self>) {
        match feed::parse_event(line) {
            Some(Event::Notice(notice)) => self.notice(notice, cx),
            Some(Event::Clipboard { text, pane }) => self.clipboard(text, pane, cx),
            Some(Event::Bell { tab, pane, .. }) => self.bell(tab, pane, cx),
            Some(Event::Other) | None => {}
        }
    }

    // -----------------------------------------------------------------------------------------
    // Agent notices.

    fn notice(&mut self, notice: Notice, cx: &mut Context<Self>) {
        let go = ("Go".to_string(), args(&["go", &notice.token]));
        self.store.update(cx, |s, cx| s.toast_with(notice.message.clone(), notice.toast_kind(), Some(go), cx));
        let desktop = !self.active;
        if notice.sound.is_none() && !desktop {
            return;
        }
        // The settings say whether desktop notifications are off and which files to play; read
        // them now, so a change in Settings counts from the next notice.
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

    // -----------------------------------------------------------------------------------------
    // What programs ask of the terminal.

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

    /// A bell in the shown tab flashes the terminal; one elsewhere says where, once in a while.
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
            let _ = this.update(cx, |this, cx| {
                if this.flash == flash {
                    this.flashing = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Whether the terminal shows the bell's flash now.
    pub fn flashing(&self) -> bool {
        self.flashing
    }

    // -----------------------------------------------------------------------------------------
    // Updates.

    /// `cornercase update`: downloads and installs the release. Without a terminal and without
    /// `--yes` it never restarts the server; the restart is asked for separately.
    pub fn install(&mut self, cx: &mut Context<Self>) {
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

    /// The version installed and waiting for a restart: by this window, or by another client.
    pub fn installed_version(&self) -> Option<String> {
        match &self.install {
            InstallState::Done(v) => Some(v.clone()),
            _ => {
                let update = self.app.update.as_ref()?;
                update.installed.then(|| update.release.as_ref().map(|r| r.version.clone())).flatten()
            }
        }
    }

    /// `cornercase restart --yes`, or with `--when-idle` kept running until no agent works (it
    /// ends with the restart; dropping it cancels).
    pub fn restart(&mut self, when_idle: bool, cx: &mut Context<Self>) {
        if !when_idle {
            self.store.read(cx).cmd(args(&["restart", "--yes"]));
            return;
        }
        self.restart = store::follow_until(
            cx,
            args(&["restart", "--when-idle", "--yes"]),
            |_, _, _| {},
            |this, err, cx| {
                this.restart = None;
                let err = err.trim().lines().last().unwrap_or_default().to_string();
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

    // -----------------------------------------------------------------------------------------
    // The status bar.

    /// The status bar's items for the update, away and keeping awake, each shown only when it
    /// has something to say.
    pub fn status_items(&self, this: &Entity<Self>) -> Vec<AnyElement> {
        let mut items = Vec::new();
        let release = self.app.release();
        if self.restart.is_some() {
            let n = this.clone();
            items.push(
                chip("restart-pending", "↻ restart pending", Some(colours::working()), colours::working())
                    .tooltip(tip("Restarts once no agent is in the middle of a turn. Click to cancel."))
                    .on_click(move |_: &ClickEvent, _, cx| n.update(cx, |n, cx| n.cancel_restart(cx)))
                    .into_any_element(),
            );
        } else if let Some(version) = self.installed_version() {
            let n = this.clone();
            items.push(
                chip("restart", format!("↻ restart for {version}"), None, colours::accent())
                    .tooltip(tip(format!("cornercase {version} is installed; the server runs the old one until it restarts.")))
                    .on_click(move |_: &ClickEvent, window, cx| update::show_restart(n.clone(), window, cx))
                    .into_any_element(),
            );
        } else if let Some(release) = release {
            let n = this.clone();
            let label = match self.install {
                InstallState::Running => format!("↑ installing {}…", release.version),
                _ => format!("↑ {}", release.version),
            };
            items.push(
                chip("update", label, None, colours::accent())
                    .tooltip(tip(format!("cornercase {} is out: see what changed", release.version)))
                    .on_click(move |_: &ClickEvent, window, cx| update::show_update(n.clone(), window, cx))
                    .into_any_element(),
            );
        }

        if self.app.away_ready() {
            let on = self.app.away_on();
            let ready: Vec<String> =
                self.app.away.iter().filter(|a| a.ready).map(|a| feed::platform_name(&a.platform)).collect();
            let (dot, text, hint, command) = if on.is_empty() {
                (None, colours::faint(), format!("Click to have agents message you on {}", ready.join(" and ")), args(&["away"]))
            } else {
                let names: Vec<String> = on.iter().map(|p| feed::platform_name(p)).collect();
                (
                    Some(colours::waiting()),
                    colours::text(),
                    format!("Agents message you on {} while you are away. Click to stop.", names.join(" and ")),
                    args(&["away", "--off"]),
                )
            };
            items.push(
                chip("away", "away", dot, text)
                    .tooltip(tip(hint))
                    .on_click(move |_: &ClickEvent, _, cx| Store::get(cx).read(cx).cmd(command.clone()))
                    .into_any_element(),
            );
        }

        if let Some(awake) = &self.app.keep_awake {
            if awake.held {
                let by = awake.by.as_deref().map(|b| format!(" by {b}")).unwrap_or_default();
                items.push(
                    chip("awake", "awake", Some(colours::done()), colours::muted())
                        .tooltip(tip(format!("The machine is kept awake{by} while agents work")))
                        .into_any_element(),
                );
            } else if let Some(why) = &awake.unavailable {
                items.push(
                    chip("awake", "awake ✗", None, colours::faint())
                        .tooltip(tip(format!("Cannot keep the machine awake: {why}")))
                        .into_any_element(),
                );
            }
        }
        items
    }
}

/// The window's title: the shown pane's own title (what its program set), else its tab's name.
pub fn window_title(session: &Session) -> String {
    let shown = session.shown();
    let tab = session
        .projects
        .iter()
        .flat_map(|p| &p.workspaces)
        .flat_map(|w| &w.tabs)
        .find(|t| Some(t.id) == shown.tab);
    let Some(tab) = tab else { return "cornercase".into() };
    let pane = shown
        .pane
        .and_then(|id| tab.panes.iter().find(|p| p.id == id))
        .or_else(|| tab.panes.iter().find(|p| p.active));
    let title = pane.and_then(|p| p.title.as_deref()).map(str::trim).filter(|t| !t.is_empty());
    format!("{} — cornercase", title.unwrap_or(&tab.name))
}

/// `project › workspace › tab` for a tab.
fn tab_place(session: &Session, tab: u64) -> Option<String> {
    session.projects.iter().find_map(|p| {
        p.workspaces.iter().find_map(|w| {
            w.tabs.iter().find(|t| t.id == tab).map(|t| format!("{} › {} › {}", p.name, w.name, t.name))
        })
    })
}

/// A compact clickable item of the status bar, with an optional dot.
fn chip(id: &'static str, label: impl Into<SharedString>, dot: Option<Rgba>, text: Rgba) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .gap(px(5.))
        .px(px(6.))
        .py(px(1.))
        .rounded(px(4.))
        .text_color(text)
        .cursor_pointer()
        .hover(|s| s.bg(colours::hover()))
        .when_some(dot, |d, colour| d.child(div().size(px(6.)).rounded_full().bg(colour)))
        .child(label.into())
}

/// A tooltip builder for `.tooltip(..)`.
pub fn tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_, cx| cx.new(|_| Tip(text.clone())).into()
}

struct Tip(SharedString);

impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .max_w(px(320.))
            .px(px(8.))
            .py(px(5.))
            .rounded(px(5.))
            .bg(colours::panel())
            .border_1()
            .border_color(colours::border())
            .shadow_lg()
            .text_size(px(11.))
            .text_color(colours::text())
            .font_family(".SystemUIFont")
            .child(self.0.clone())
    }
}

/// A macOS notification. GPUI has none for an unbundled binary, so `osascript` posts it; the
/// texts go as arguments, never inside the script.
fn desktop_notification(title: &str, body: &str) {
    let script = ["on run argv", "display notification (item 2 of argv) with title (item 1 of argv)", "end run"];
    let mut a: Vec<String> = script.iter().flat_map(|line| ["-e".to_string(), (*line).to_string()]).collect();
    a.push(title.to_string());
    a.push(body.to_string());
    spawn_quietly("osascript", &a);
}

/// Runs a helper program with no input or output, reaped on a thread of its own.
fn spawn_quietly(program: &'static str, a: &[String]) {
    let a = a.to_vec();
    std::thread::spawn(move || {
        let _ = Command::new(program).args(&a).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        serde_json::from_value(serde_json::json!({
            "boot": "b", "revision": 1, "version": "0.12.189", "groups": [],
            "projects": [{"id": 1, "name": "shop", "path": "/p", "workspaces": [{
                "id": 2, "name": "main", "path": "/p", "tabs": [
                    {"id": 3, "name": "zsh", "panes": [
                        {"id": 4, "path": "/p", "active": true, "title": "vim notes.md"},
                        {"id": 5, "path": "/p", "active": false, "title": null}]},
                    {"id": 6, "name": "claude", "panes": [{"id": 7, "path": "/p", "active": true}]}
                ]}]}],
            "view": {"id": 9, "projection": 1, "active": true, "shown": {"project": 1, "workspace": 2, "tab": 3, "pane": 4}}
        }))
        .expect("a session")
    }

    #[test]
    fn the_title_is_the_panes_or_the_tabs() {
        let mut s = session();
        assert_eq!(window_title(&s), "vim notes.md — cornercase");
        s.view.as_mut().expect("view").shown.pane = Some(5);
        assert_eq!(window_title(&s), "zsh — cornercase");
        s.view.as_mut().expect("view").shown.tab = Some(6);
        s.view.as_mut().expect("view").shown.pane = None;
        assert_eq!(window_title(&s), "claude — cornercase");
        s.view = None;
        assert_eq!(window_title(&s), "cornercase");
    }

    #[test]
    fn a_tab_is_placed() {
        assert_eq!(tab_place(&session(), 6).as_deref(), Some("shop › main › claude"));
        assert_eq!(tab_place(&session(), 60), None);
    }
}
