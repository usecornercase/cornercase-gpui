use gpui::{
    App, AppContext as _, ClickEvent, ClipboardItem, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Rgba, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::feed::{Notes, Release, Update};
use super::{InstallState, Notifier};
use crate::cli::Version;
use crate::dialogs::frame;
use crate::store::{Store, ToastKind};
use crate::theme::{colours, radius};
use crate::ui;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Chip {
    Pending,
    Restart(String),
    Installing(String),
    Found(String),
}

impl Chip {
    pub(crate) fn of(
        pending: bool,
        installed: Option<String>,
        release: Option<&Release>,
        install: &InstallState,
    ) -> Option<Self> {
        if pending {
            return Some(Self::Pending);
        }
        let out = release.map(|release| release.version.clone());
        match (install, installed) {
            (InstallState::Running, installed) => out.or(installed).map(Self::Installing),
            (_, Some(version)) => Some(Self::Restart(version)),
            (_, None) => out.map(Self::Found),
        }
    }

    pub(crate) fn label(&self) -> String {
        match self {
            Self::Pending => "↻ cornercase restart pending".to_string(),
            Self::Restart(version) => format!("↻ restart cornercase for {version}"),
            Self::Installing(version) => format!("installing cornercase {version}…"),
            Self::Found(version) => format!("↑ cornercase {version}"),
        }
    }

    pub(crate) fn tip(&self) -> String {
        match self {
            Self::Pending => "Restarts once no agent is in the middle of a turn. Click to cancel.".to_string(),
            Self::Restart(version) => {
                format!("cornercase {version} is installed; the server runs the old one until it restarts.")
            }
            Self::Installing(version) => format!("Installing cornercase {version}…"),
            Self::Found(version) => format!("cornercase {version} is out: see what changed"),
        }
    }
}

pub(crate) fn installed(install: &InstallState, update: Option<&Update>, on_disk: Option<Version>) -> Option<String> {
    if let InstallState::Done(version) = install {
        return Some(version.clone());
    }
    let update = update?;
    let release = update.release.as_ref()?;
    if update.installed {
        return Some(release.version.clone());
    }
    let out = Version::parse(&release.version)?;
    on_disk.filter(|disk| *disk >= out).map(|disk| disk.to_string())
}

pub(crate) fn close(cx: &mut App) {
    Store::get(cx).update(cx, Store::close_dialog);
}

pub(crate) fn show<V: Render>(make: impl FnOnce(FocusHandle, &mut Context<V>) -> V, window: &mut Window, cx: &mut App) {
    let focus = cx.focus_handle();
    let view = cx.new(|cx| make(focus.clone(), cx));
    Store::get(cx).update(cx, |s, cx| s.show_dialog(view.into(), cx));
    window.focus(&focus);
}

pub(super) struct UpdateDialog {
    notifier: Entity<Notifier>,
    focus: FocusHandle,
}

pub(super) fn show_update(notifier: Entity<Notifier>, window: &mut Window, cx: &mut App) {
    show(
        |focus, cx| {
            cx.observe(&notifier, |_, _, cx| cx.notify()).detach();
            UpdateDialog { notifier, focus }
        },
        window,
        cx,
    );
}

impl Render for UpdateDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let n = self.notifier.read(cx);
        let update = n.app.update.clone().unwrap_or_default();
        let running = n.app.version.clone().unwrap_or_default();
        let install = n.install.clone();
        let dialog = div().track_focus(&self.focus);
        let Some(release) = update.release.clone() else {
            return dialog.child(frame(
                "No update",
                ui::dim(format!("cornercase {running} is the latest release.")),
                ui::button("close", "Close").on_click(|_: &ClickEvent, _, cx| close(cx)),
            ));
        };
        let command = update.install.as_ref().filter(|i| i.method == "command").and_then(|i| i.command.clone());
        let installed = n.installed_version();
        let status = install_status(&install, &update, installed.as_deref());
        let body = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(ui::dim(format!("You run cornercase {running}. What changed since:")))
            .child(release_notes(&release))
            .child(div().text_size(px(12.)).text_color(colours::muted()).child(how_to_install(command.as_deref())))
            .when_some(status, |d, (text, colour)| d.child(div().text_size(px(12.)).text_color(colour).child(text)));
        let buttons = update_buttons(self.notifier.clone(), (command, installed.is_some()), &install, &release.version);
        dialog.child(frame(format!("cornercase {} is out", release.version), body, buttons))
    }
}

pub(crate) fn release_notes(release: &Release) -> impl IntoElement + use<> {
    div()
        .id("update-notes")
        .max_h(px(300.))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(px(10.))
        .p(px(12.))
        .rounded(px(radius::CONTROL))
        .bg(colours::terminal())
        .border_1()
        .border_color(colours::border())
        .when(release.notes.is_empty(), |d| d.child(ui::dim("This release has no notes.")))
        .children(release.notes.iter().map(notes_section))
}

fn notes_section(section: &Notes) -> impl IntoElement + use<> {
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(div().text_size(px(12.)).text_color(colours::accent()).child(section.version.clone()))
        .child(
            div()
                .text_size(px(12.))
                .text_color(colours::text())
                .flex()
                .flex_col()
                .children(section.text.lines().map(|l| div().child(l.to_string()))),
        )
}

fn install_status(install: &InstallState, update: &Update, installed: Option<&str>) -> Option<(String, Rgba)> {
    match (install, installed) {
        (InstallState::Running, _) => Some(("Downloading and installing…".into(), colours::muted())),
        (InstallState::Failed(e), _) => Some((e.clone(), colours::failed())),
        (_, Some(v)) => Some((format!("cornercase {v} is installed. Restart to use it."), colours::done())),
        (_, None) => update.install_error.clone().map(|e| (e, colours::failed())),
    }
}

fn how_to_install(command: Option<&str>) -> String {
    match command {
        Some(cmd) => format!("This cornercase is updated with a command of its own:\n{cmd}"),
        None => "Installing replaces the cornercase binary; the server keeps running until you restart it.".to_string(),
    }
}

fn update_buttons(
    notifier: Entity<Notifier>,
    (command, installed): (Option<String>, bool),
    install: &InstallState,
    version: &str,
) -> impl IntoElement + use<> {
    let action = match (command, install) {
        (_, InstallState::Running) => ui::busy_button("installing", "Installing…", ui::Tone::Primary),
        _ if installed => ui::danger_button("restart", "Restart…")
            .on_click(move |_: &ClickEvent, window, cx| show_restart(notifier.clone(), window, cx)),
        (Some(cmd), _) => ui::primary_button("copy", "Copy command").on_click(move |_: &ClickEvent, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(cmd.clone()));
            Store::get(cx).update(cx, |s, cx| s.toast("Copied", ToastKind::Info, cx));
        }),
        (None, _) => ui::primary_button("install", format!("Install {version}"))
            .on_click(move |_: &ClickEvent, _, cx| notifier.update(cx, Notifier::install)),
    };
    div()
        .flex()
        .gap(px(8.))
        .child(ui::button("close", "Later").on_click(|_: &ClickEvent, _, cx| close(cx)))
        .child(action)
}

pub(super) struct RestartDialog {
    notifier: Entity<Notifier>,
    focus: FocusHandle,
}

pub(super) fn show_restart(notifier: Entity<Notifier>, window: &mut Window, cx: &mut App) {
    show(|focus, _| RestartDialog { notifier, focus }, window, cx);
}

impl Render for RestartDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let version = self.notifier.read(cx).installed_version().unwrap_or_else(|| "the new version".into());
        let (now, idle) = (self.notifier.clone(), self.notifier.clone());
        div().track_focus(&self.focus).child(frame(
            format!("Restart cornercase on {version}?"),
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .text_color(colours::muted())
                .child("Every program running in the terminals stops. Agents resume their conversations afterwards, and this window reconnects by itself.")
                .child(ui::dim("“When agents are idle” waits until no agent is in the middle of a turn.")),
            div()
                .flex()
                .gap(px(8.))
                .child(ui::button("cancel", "Later").on_click(|_: &ClickEvent, _, cx| close(cx)))
                .child(ui::button("idle", "When agents are idle").on_click(move |_: &ClickEvent, _, cx| {
                    idle.update(cx, |n, cx| n.restart(true, cx));
                    close(cx);
                }))
                .child(ui::danger_button("now", "Restart now").on_click(move |_: &ClickEvent, _, cx| {
                    now.update(cx, |n, cx| n.restart(false, cx));
                    close(cx);
                })),
        ))
    }
}

#[cfg(test)]
mod tests;
