//! The update's two dialogs: the release notes with the way to install (`cornercase update`, or
//! the command to copy when cornercase cannot replace itself, as with Homebrew), then the restart
//! onto the new version (`cornercase restart`, at once or once no agent is working).

use gpui::{
    App, AppContext as _, ClickEvent, ClipboardItem, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::{InstallState, Notifier};
use crate::dialogs::frame;
use crate::store::Store;
use crate::theme::colours;
use crate::ui;

fn close(cx: &mut App) {
    Store::get(cx).update(cx, Store::close_dialog);
}

/// Shows a dialog that closes on escape.
fn show<V: Render>(make: impl FnOnce(FocusHandle, &mut Context<V>) -> V, window: &mut Window, cx: &mut App) {
    let focus = cx.focus_handle();
    let view = cx.new(|cx| make(focus.clone(), cx));
    Store::get(cx).update(cx, |s, cx| s.show_dialog(view.into(), cx));
    window.focus(&focus);
}

fn on_escape(e: &KeyDownEvent, _: &mut Window, cx: &mut App) {
    if e.keystroke.key == "escape" {
        close(cx);
    }
}

// ---------------------------------------------------------------------------------------------

pub(super) struct UpdateDialog {
    notifier: Entity<Notifier>,
    focus: FocusHandle,
}

pub(super) fn show_update(notifier: Entity<Notifier>, window: &mut Window, cx: &mut App) {
    show(
        |focus, cx| {
            // The install's progress shows in the dialog.
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
        let Some(release) = update.release.clone() else {
            return div().track_focus(&self.focus).on_key_down(on_escape).child(frame(
                "No update",
                ui::dim(format!("cornercase {running} is the latest release.")),
                ui::button("close", "Close").on_click(|_: &ClickEvent, _, cx| close(cx)),
            ));
        };
        let command = update.install.as_ref().filter(|i| i.method == "command").and_then(|i| i.command.clone());

        let notes = div()
            .id("update-notes")
            .max_h(px(300.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(10.))
            .p(px(10.))
            .rounded(px(6.))
            .bg(colours::terminal())
            .border_1()
            .border_color(colours::border())
            .when(release.notes.is_empty(), |d| d.child(ui::dim("This release has no notes.")))
            .children(release.notes.iter().map(|section| {
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
            }));

        let status: Option<(String, gpui::Rgba)> = match &install {
            InstallState::Running => Some(("Downloading and installing…".into(), colours::muted())),
            InstallState::Done(v) => {
                Some((format!("cornercase {v} is installed. Restart to use it."), colours::done()))
            }
            InstallState::Failed(e) => Some((e.clone(), colours::failed())),
            InstallState::Idle => update.install_error.clone().map(|e| (e, colours::failed())),
        };
        let how = match &command {
            Some(cmd) => format!("This cornercase is updated with a command of its own:\n{cmd}"),
            None => {
                "Installing replaces the cornercase binary; the server keeps running until you restart it.".to_string()
            }
        };

        let body = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(ui::dim(format!("You run {running}. What changed since:")))
            .child(notes)
            .child(div().text_size(px(12.)).text_color(colours::muted()).child(how))
            .when_some(status, |d, (text, colour)| d.child(div().text_size(px(12.)).text_color(colour).child(text)));

        let notifier = self.notifier.clone();
        let buttons = div()
            .flex()
            .gap(px(8.))
            .child(ui::button("close", "Later").on_click(|_: &ClickEvent, _, cx| close(cx)))
            .map(|d| match (&command, &install) {
                (Some(cmd), _) => {
                    let cmd = cmd.clone();
                    d.child(ui::primary_button("copy", "Copy command").on_click(move |_: &ClickEvent, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(cmd.clone()));
                        Store::get(cx).update(cx, |s, cx| s.toast("Copied", crate::store::ToastKind::Info, cx));
                    }))
                }
                (None, InstallState::Running) => d.child(ui::button("installing", "Installing…")),
                (None, InstallState::Done(_)) => d.child(
                    ui::danger_button("restart", "Restart…")
                        .on_click(move |_: &ClickEvent, window, cx| show_restart(notifier.clone(), window, cx)),
                ),
                (None, _) => d.child(
                    ui::primary_button("install", format!("Install {}", release.version))
                        .on_click(move |_: &ClickEvent, _, cx| notifier.update(cx, Notifier::install)),
                ),
            });

        div().track_focus(&self.focus).on_key_down(on_escape).child(frame(
            format!("cornercase {} is out", release.version),
            body,
            buttons,
        ))
    }
}

// ---------------------------------------------------------------------------------------------

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
        div().track_focus(&self.focus).on_key_down(on_escape).child(frame(
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
