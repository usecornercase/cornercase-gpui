use gpui::{
    App, ClickEvent, ClipboardItem, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Rgba, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::install::{Way, Why, program_name};
use super::release::Found;
use super::{CURRENT, Updater};
use crate::dialogs::frame;
use crate::notify::{InstallState, update};
use crate::platform::Os;
use crate::store::{Store, ToastKind};
use crate::theme::colours;
use crate::ui;

pub(super) struct UpdateDialog {
    updater: Entity<Updater>,
    focus: FocusHandle,
}

pub(super) fn show(updater: Entity<Updater>, window: &mut Window, cx: &mut App) {
    update::show(
        |focus, cx| {
            cx.observe(&updater, |_, _, cx| cx.notify()).detach();
            UpdateDialog { updater, focus }
        },
        window,
        cx,
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Action {
    Busy,
    Restart,
    Install,
    Download(String),
    Copy(String),
}

fn action(install: &InstallState, found: &Found) -> Action {
    match (install, &found.way) {
        (InstallState::Running, _) => Action::Busy,
        (InstallState::Done(_), _) => Action::Restart,
        _ if found.on_disk => Action::Restart,
        (InstallState::Failed(_), Way::Replace { asset }) => Action::Download(asset.url.clone()),
        (_, Way::Replace { .. }) => Action::Install,
        (_, Way::Download { url, .. }) => Action::Download(url.clone()),
        (_, Way::Command(command)) => Action::Copy(command.clone()),
    }
}

fn how(way: &Way, os: Os) -> String {
    match way {
        Way::Replace { .. } => {
            "Installing replaces this window's program; Restart window then opens it again, and the terminals and agents keep running."
                .to_string()
        }
        Way::Download { why: Why::Bundle, .. } => "Open the download and drag cornercase to Applications, replacing \
            this one. macOS asks before it opens it, as it did the first time."
            .to_string(),
        Way::Download { why: Why::Folder(folder), .. } => format!(
            "This window cannot write to {}, so it cannot replace itself: download the new version and put it in \
             place of {}.",
            folder.display(),
            program_name(os)
        ),
        Way::Download { why: Why::Unplaced, .. } => "This window cannot tell where its program is, so it cannot \
            replace itself: download the new version."
            .to_string(),
        Way::Download { why: Why::NoDownload, .. } => {
            "There is no download for this machine: see the release.".to_string()
        }
        Way::Command(command) => {
            let note = if os == Os::Windows {
                "\nClose the window first: Windows does not let a running program be replaced."
            } else {
                ""
            };
            format!("This window was built from source. Update it with:\n{command}{note}")
        }
    }
}

fn status(install: &InstallState, found: &Found, error: Option<&str>) -> Option<(String, Rgba)> {
    let installed = |version: &str| {
        (format!("cornercase-gpui {version} is installed. Restart the window to use it."), colours::done())
    };
    match install {
        InstallState::Running => Some(("Downloading and installing…".to_string(), colours::muted())),
        InstallState::Done(version) => Some(installed(version)),
        InstallState::Failed(why) => Some((why.clone(), colours::failed())),
        InstallState::Idle if found.on_disk => Some(installed(&found.latest.version)),
        InstallState::Idle => error.map(|why| (format!("The last check failed: {why}"), colours::muted())),
    }
}

fn close(cx: &mut App) {
    Store::get(cx).update(cx, Store::close_dialog);
}

impl Render for UpdateDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let updater = self.updater.read(cx);
        let dialog = div().track_focus(&self.focus);
        let Some(found) = updater.found.clone() else {
            return dialog.child(frame(
                "No update",
                ui::dim("This window runs the latest release."),
                ui::button("close", "Close").on_click(|_: &ClickEvent, _, cx| close(cx)),
            ));
        };
        let done = status(&updater.install, &found, updater.error.as_deref());
        let act = action(&updater.install, &found);
        let how = (act != Action::Restart).then(|| how(&found.way, Os::CURRENT));
        let body = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(ui::dim(format!("This window runs {CURRENT}. What changed since:")))
            .child(update::release_notes(&found.release()))
            .when_some(how, |d, how| d.child(div().text_size(px(12.)).text_color(colours::muted()).child(how)))
            .when_some(done, |d, (text, colour)| d.child(div().text_size(px(12.)).text_color(colour).child(text)));
        let title = format!("cornercase-gpui {} is out", found.latest.version);
        dialog.child(frame(title, body, buttons(self.updater.clone(), act, &found.latest.version)))
    }
}

fn buttons(updater: Entity<Updater>, act: Action, version: &str) -> impl IntoElement + use<> {
    let action = match act {
        Action::Busy => ui::busy_button("installing", "Installing…", ui::Tone::Primary),
        Action::Restart => ui::danger_button("restart", "Restart window")
            .on_click(move |_: &ClickEvent, _, cx| updater.update(cx, Updater::restart)),
        Action::Install => ui::primary_button("install", format!("Install {version}"))
            .on_click(move |_: &ClickEvent, _, cx| updater.update(cx, Updater::install)),
        Action::Download(url) => {
            ui::primary_button("download", "Download").on_click(move |_: &ClickEvent, _, cx| cx.open_url(&url))
        }
        Action::Copy(command) => ui::primary_button("copy", "Copy command").on_click(move |_: &ClickEvent, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(command.clone()));
            Store::get(cx).update(cx, |s, cx| s.toast("Copied", ToastKind::Info, cx));
        }),
    };
    div()
        .flex()
        .gap(px(8.))
        .child(ui::button("close", "Later").on_click(|_: &ClickEvent, _, cx| close(cx)))
        .child(action)
}

#[cfg(test)]
mod tests;
