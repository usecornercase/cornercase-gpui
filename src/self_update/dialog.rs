use gpui::{
    App, ClickEvent, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    Rgba, StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use super::install::{Way, Why, program_name};
use super::release::Found;
use super::{CURRENT, NAME, Updater};
use crate::dialogs::frame;
use crate::notify::InstallState;
use crate::notify::update::{self, ParsedNotes};
use crate::platform::Os;
use crate::store::Store;
use crate::theme::colours;
use crate::ui;

pub(super) struct UpdateDialog {
    updater: Entity<Updater>,
    focus: FocusHandle,
    notes: ParsedNotes,
}

pub(super) fn show(updater: Entity<Updater>, window: &mut Window, cx: &mut App) {
    update::show(
        |focus, cx| {
            cx.observe(&updater, |_, _, cx| cx.notify()).detach();
            UpdateDialog { updater, focus, notes: ParsedNotes::default() }
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
        (_, Way::Replace { .. }) => Action::Install,
        (_, Way::Download { url, .. }) => Action::Download(url.clone()),
        (_, Way::Command(command)) => Action::Copy(command.clone()),
    }
}

fn fallback(install: &InstallState, found: &Found) -> Option<String> {
    match (install, &found.way) {
        (InstallState::Failed(_), Way::Replace { asset }) => Some(asset.url.clone()),
        _ => None,
    }
}

fn how(way: &Way, os: Os) -> String {
    match way {
        Way::Replace { .. } => {
            "Install puts the new version in place of this one, and Restart opens it on the same tab; \
            the terminals and agents keep running."
                .to_string()
        }
        Way::Download { why: Why::Bundle, .. } => "Open the download and drag cornercase to Applications, replacing \
            this one. macOS asks before it opens it, as it did the first time."
            .to_string(),
        Way::Download { why: Why::Folder(folder), .. } if os == Os::Mac => format!(
            "{NAME} cannot write to {}, so it cannot replace itself: open the download and drag cornercase to \
             Applications, replacing this one.",
            folder.display()
        ),
        Way::Download { why: Why::Folder(folder), .. } => format!(
            "{NAME} cannot write to {}, so it cannot replace itself: download the new version and put it in place \
             of {}.",
            folder.display(),
            program_name(os)
        ),
        Way::Download { why: Why::Unplaced, .. } => {
            format!("{NAME} cannot tell where its program is, so it cannot replace itself: download the new version.")
        }
        Way::Download { why: Why::NoDownload, .. } => {
            "There is no download for this machine: see the release.".to_string()
        }
        Way::Command(command) => from_source(command, os),
    }
}

fn from_source(command: &str, os: Os) -> String {
    let note = if os == Os::Windows {
        "\nClose the window first: Windows does not let a running program be replaced."
    } else {
        ""
    };
    format!("This copy was built from source. Update it with:\n{command}{note}")
}

fn status(install: &InstallState, found: &Found, error: Option<&str>) -> Option<(String, Rgba)> {
    let installed = |version: &str| (format!("{NAME} {version} is installed. Restart to use it."), colours::done());
    match install {
        InstallState::Running => Some(("Downloading and installing…".to_string(), colours::muted())),
        InstallState::Done(version) => Some(installed(version)),
        InstallState::Failed(why) => Some((why.clone(), colours::failed())),
        InstallState::Idle | InstallState::Manual(_) if found.on_disk => Some(installed(&found.latest.version)),
        InstallState::Idle | InstallState::Manual(_) => {
            error.map(|why| (format!("The last check failed: {why}"), colours::muted()))
        }
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
                ui::dim(format!("{NAME} {CURRENT} is the latest release.")),
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
            .child(ui::dim(format!("You run {NAME} {CURRENT}. What changed since:")))
            .child(update::release_notes(&found.notes, &mut self.notes))
            .when_some(how, |d, how| d.child(div().text_size(px(12.)).text_color(colours::muted()).child(how)))
            .when_some(done, |d, (text, colour)| d.child(div().text_size(px(12.)).text_color(colour).child(text)));
        let title = format!("{NAME} {} is out", found.latest.version);
        dialog.child(frame(
            title,
            body,
            buttons(self.updater.clone(), act, fallback(&updater.install, &found), &found.latest.version),
        ))
    }
}

fn buttons(updater: Entity<Updater>, act: Action, fallback: Option<String>, version: &str) -> impl IntoElement + use<> {
    let action = match act {
        Action::Busy => ui::busy_button("installing", "Installing…", ui::Tone::Primary),
        Action::Restart => ui::danger_button("restart", format!("Restart {NAME}"))
            .on_click(move |_: &ClickEvent, _, cx| updater.update(cx, Updater::restart)),
        Action::Install => ui::primary_button("install", format!("Install {version}"))
            .on_click(move |_: &ClickEvent, _, cx| updater.update(cx, Updater::install)),
        Action::Download(url) => {
            ui::primary_button("download", "Download").on_click(move |_: &ClickEvent, _, cx| cx.open_url(&url))
        }
        Action::Copy(command) => update::copy_command(command, "Copied".into()),
    };
    div()
        .flex()
        .gap(px(8.))
        .child(ui::button("close", "Later").on_click(|_: &ClickEvent, _, cx| close(cx)))
        .when_some(fallback, |d, url| {
            d.child(ui::button("download", "Download").on_click(move |_: &ClickEvent, _, cx| cx.open_url(&url)))
        })
        .child(action)
}

#[cfg(test)]
mod tests;
