mod dialog;
mod fetch;
mod install;
mod relaunch;
mod release;

use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Entity, Global, IntoElement as _,
    StatefulInteractiveElement as _,
};
use serde_json::Value;

use crate::cli;
use crate::notify::{self, InstallState, Notes};
use crate::platform::Os;
use crate::store::{self, Busy, Store, args};
use crate::theme::colours;
use crate::ui;
use install::Way;
use release::{Found, Latest};

pub(crate) use relaunch::Relaunch;
pub(crate) use release::version_line;

const FIRST_CHECK: Duration = Duration::from_secs(10);
const CHECK_EVERY: Duration = Duration::from_hours(1);
const RELEASES_URL: &str = "https://api.github.com/repos/usecornercase/cornercase-gpui/releases/latest";
const URL_VARIABLE: &str = "CORNERCASE_GPUI_RELEASES_URL";
const RELEASED: bool = option_env!("CORNERCASE_GPUI_RELEASE").is_some();
const CURRENT: &str = env!("CARGO_PKG_VERSION");

pub(crate) struct Updater {
    exe: Option<PathBuf>,
    url: String,
    found: Option<Found>,
    error: Option<String>,
    install: InstallState,
    relaunch: Option<Relaunch>,
}

struct GlobalUpdater(Entity<Updater>);
impl Global for GlobalUpdater {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ChipState {
    Installing(String),
    Installed(String),
    Found(String),
}

impl ChipState {
    fn label(&self) -> String {
        match self {
            Self::Installing(version) => format!("installing window {version}…"),
            Self::Installed(version) => format!("↻ restart window for {version}"),
            Self::Found(version) => format!("↑ window {version}"),
        }
    }

    fn tip(&self) -> String {
        match self {
            Self::Installing(version) => format!("Installing cornercase-gpui {version}…"),
            Self::Installed(version) => {
                format!("cornercase-gpui {version} is installed; this window runs {CURRENT} until it restarts")
            }
            Self::Found(version) => format!("cornercase-gpui {version} is out: see what changed"),
        }
    }
}

fn releases_url(asked: Option<String>, debug_build: bool) -> Option<String> {
    if !debug_build {
        return Some(RELEASES_URL.to_string());
    }
    asked.filter(|url| !url.trim().is_empty())
}

fn wants_check(config: Option<&Value>) -> bool {
    let settings = config.and_then(|config| config.get("settings")).and_then(Value::as_array);
    let setting =
        settings.and_then(|all| all.iter().find(|s| s.get("key").and_then(Value::as_str) == Some("check_updates")));
    setting.and_then(|s| s.get("value")).and_then(Value::as_bool).unwrap_or(true)
}

pub(crate) fn start(cx: &mut App) {
    let exe = std::env::current_exe().ok();
    if let Some(exe) = exe.clone() {
        cx.background_spawn(async move { install::sweep(&exe) }).detach();
    }
    let Some(url) = releases_url(std::env::var(URL_VARIABLE).ok(), cfg!(debug_assertions)) else { return };
    let updater = cx.new(|_| Updater::new(url, exe));
    cx.set_global(GlobalUpdater(updater.clone()));
    cx.spawn(async move |cx| {
        cx.background_executor().timer(FIRST_CHECK).await;
        loop {
            if updater.update(cx, |_, cx| Updater::check(cx)).is_err() {
                return;
            }
            cx.background_executor().timer(CHECK_EVERY).await;
        }
    })
    .detach();
}

pub(crate) fn entity(cx: &App) -> Option<Entity<Updater>> {
    cx.try_global::<GlobalUpdater>().map(|global| global.0.clone())
}

pub(crate) fn take_relaunch(cx: &mut App) -> Option<Relaunch> {
    entity(cx)?.update(cx, |this, _| this.relaunch.take())
}

pub(crate) fn status_item(cx: &App) -> Option<AnyElement> {
    let updater = entity(cx)?;
    let state = updater.read(cx).chip_state()?;
    let lead = matches!(state, ChipState::Installing(_))
        .then(|| ui::spinner("window-installing", 10., colours::accent()).into_any_element());
    Some(
        notify::chip("window-update", state.label(), lead, colours::accent())
            .tooltip(notify::tip(state.tip()))
            .on_click(move |_: &ClickEvent, window, cx| dialog::show(updater.clone(), window, cx))
            .into_any_element(),
    )
}

impl Updater {
    fn new(url: String, exe: Option<PathBuf>) -> Self {
        Self { exe, url, found: None, error: None, install: InstallState::Idle, relaunch: None }
    }

    fn chip_state(&self) -> Option<ChipState> {
        let version = |found: &Found| found.latest.version.clone();
        match (&self.install, &self.found) {
            (InstallState::Running, Some(found)) => Some(ChipState::Installing(version(found))),
            (InstallState::Done(done), _) => Some(ChipState::Installed(done.clone())),
            (_, Some(found)) if found.on_disk => Some(ChipState::Installed(version(found))),
            (_, Some(found)) => Some(ChipState::Found(version(found))),
            (_, None) => None,
        }
    }

    fn check(cx: &mut Context<Self>) {
        store::run_json(cx, args(&["config", "--json"]), |this, config, cx| {
            if wants_check(config.ok().as_ref()) {
                this.look(cx);
            }
        });
    }

    fn look(&mut self, cx: &mut Context<Self>) {
        let (url, exe, known) = (self.url.clone(), self.exe.clone(), self.found.clone());
        let task = cx.background_spawn(async move { look(&url, exe.as_deref(), known.as_ref()) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| this.checked(result, cx));
        })
        .detach();
    }

    fn checked(&mut self, result: anyhow::Result<Option<Found>>, cx: &mut Context<Self>) {
        match result {
            Ok(found) => {
                self.error = None;
                if matches!(self.install, InstallState::Idle | InstallState::Failed(_)) {
                    self.found = found;
                }
            }
            Err(e) => self.error = Some(format!("{e:#}")),
        }
        cx.notify();
    }

    pub(crate) fn install(&mut self, cx: &mut Context<Self>) {
        let Some(Found { latest, way: Way::Replace { asset }, .. }) = self.found.clone() else { return };
        let Some(exe) = self.exe.clone() else { return };
        if self.install == InstallState::Running {
            return;
        }
        self.install = InstallState::Running;
        cx.notify();
        let busy = Busy::new("window-update", format!("Installing cornercase-gpui {}…", latest.version));
        let secure = fetch::secure(&self.url);
        let work = move || install::install(&latest, &asset, &exe, Os::CURRENT, secure);
        store::run_local_busy(cx, busy, work, |this, result, cx| {
            this.install = match result {
                Ok(version) => InstallState::Done(version),
                Err(e) => InstallState::Failed(format!("{e:#}")),
            };
            cx.notify();
        });
    }

    pub(crate) fn restart(&mut self, cx: &mut Context<Self>) {
        let Some(program) = self.exe.clone() else { return };
        let tab = Store::get(cx).read(cx).session.as_ref().and_then(|session| session.shown().tab);
        let args = relaunch::relaunch_args(&cli::target().flags(), tab);
        self.relaunch = Some(Relaunch { program, args });
        cx.quit();
    }
}

fn look(url: &str, exe: Option<&Path>, known: Option<&Found>) -> anyhow::Result<Option<Found>> {
    let Some(answer) = fetch::latest(url)? else { return Ok(None) };
    let Some(latest) = release::parse(&answer) else { return Ok(None) };
    if !release::newer(&latest.version, CURRENT) {
        return Ok(None);
    }
    let notes = match known.filter(|found| found.latest.version == latest.version) {
        Some(found) => found.notes.clone(),
        None => notes_of(&latest, fetch::secure(url)),
    };
    Ok(Some(found_for(latest, notes, exe)))
}

fn notes_of(latest: &Latest, https_only: bool) -> Vec<Notes> {
    let changelog =
        latest.asset("CHANGELOG.md").and_then(|asset| fetch::download(&asset.url, fetch::NOTES_LIMIT, https_only).ok());
    let notes = changelog
        .map(|bytes| release::notes_since(&String::from_utf8_lossy(&bytes), CURRENT, &latest.version))
        .unwrap_or_default();
    if notes.is_empty() { release::body_notes(&latest.version, &latest.body) } else { notes }
}

fn found_for(latest: Latest, notes: Vec<Notes>, exe: Option<&Path>) -> Found {
    let machine = (Os::CURRENT, std::env::consts::ARCH);
    let way = install::way(RELEASED, machine, exe, &latest, install::writable);
    let on_disk = exe.and_then(install::installed_version).is_some_and(|version| version == latest.version);
    Found { latest, notes, way, on_disk }
}

#[cfg(test)]
mod tests;
