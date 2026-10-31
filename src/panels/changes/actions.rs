use gpui::{App, ClipboardItem, Context, WeakEntity, Window};

use crate::dialogs;
use crate::model::Session;
use crate::store::{self, Busy, Store, ToastKind, args};

use super::pieces::{find_mut, toast};
use super::wire::{FileInfo, FileStatus, Mode, Staging};
use super::{ChangesPanel, diff};

const UNCHANGED_ONLY: &str = "Only the version shown is discarded: if the file changed since, it is left alone.";

fn agent_pane(session: &Session, workspace: u64) -> Option<(u64, String)> {
    let (_, ws) = session.workspace(workspace)?;
    let mut tabs: Vec<_> = ws.tabs.iter().collect();
    tabs.sort_by_key(|t| !t.active);
    tabs.iter().flat_map(|t| &t.panes).find_map(|p| Some((p.id, p.agent.clone()?)))
}

fn file_args(verb: &str, workspace: u64, path: &str) -> Vec<String> {
    args(&["changes", verb, path, "--workspace", &workspace.to_string()])
}

fn with_digest(mut a: Vec<String>, digest: &str) -> Vec<String> {
    if !digest.is_empty() {
        a.extend(args(&["--digest", digest]));
    }
    a
}

pub(super) fn viewed_command(workspace: u64, file: &FileInfo, mode: Mode) -> Vec<String> {
    let mut a = file_args("viewed", workspace, &file.path);
    if file.viewed {
        a.extend(args(&["--mode", mode.name(), "--clear"]));
        return a;
    }
    if file.digest.is_empty() {
        a.extend(args(&["--mode", mode.name()]));
        return a;
    }
    with_digest(a, &file.digest)
}

pub(super) fn stage_command(workspace: u64, file: &FileInfo) -> Vec<String> {
    let verb = if file.staged == Some(Staging::Yes) { "unstage" } else { "stage" };
    with_digest(file_args(verb, workspace, &file.path), &file.digest)
}

pub(super) fn discard_command(workspace: u64, file: &FileInfo) -> Vec<String> {
    let mut a = with_digest(file_args("discard", workspace, &file.path), &file.digest);
    a.push("--yes".into());
    a
}

pub(super) fn file_busy(workspace: u64, path: &str) -> String {
    format!("changes-file:{workspace}:{path}")
}

fn file_name(file: &FileInfo) -> &str {
    file.path.rsplit('/').next().unwrap_or(&file.path)
}

fn on_refusal(panel: WeakEntity<ChangesPanel>) -> impl FnOnce(anyhow::Result<String>, &mut App) + 'static {
    move |result, cx| {
        if let Err(error) = result {
            let _ = panel.update(cx, |this, cx| this.refused(&error, cx));
        }
    }
}

impl ChangesPanel {
    pub(super) fn toggle_viewed(&mut self, workspace: u64, file: &FileInfo, cx: &mut Context<Self>) {
        let command = viewed_command(workspace, file, self.scope.mode());
        let Some(listed) = self.summary.as_mut().and_then(|s| find_mut(s, workspace, &file.path)) else { return };
        listed.viewed = !file.viewed;
        store::queued(cx, command, on_refusal(cx.weak_entity()));
        cx.notify();
    }

    pub(super) fn toggle_staged(workspace: u64, file: &FileInfo, cx: &mut Context<Self>) {
        let doing = if file.staged == Some(Staging::Yes) { "Unstaging" } else { "Staging" };
        let busy = Busy::new(file_busy(workspace, &file.path), format!("{doing} {}…", file_name(file)));
        let refused = on_refusal(cx.weak_entity());
        store::queued_busy(cx, busy, stage_command(workspace, file), refused);
    }

    pub(super) fn discard(workspace: u64, file: &FileInfo, window: &mut Window, cx: &mut Context<Self>) {
        let deletes = matches!(file.status, FileStatus::Untracked | FileStatus::Added);
        let (title, button) =
            if deletes { ("Delete the file?", "Delete") } else { ("Discard the changes?", "Discard") };
        let command = discard_command(workspace, file);
        let message = format!("{}\n\n{UNCHANGED_ONLY}", diff::discard_question(file));
        let doing = if deletes { "Deleting" } else { "Discarding the changes of" };
        let busy = Busy::new(file_busy(workspace, &file.path), format!("{doing} {}…", file_name(file)));
        let question =
            dialogs::Question::new(title, message, button, command).on_reply(on_refusal(cx.weak_entity())).busy(busy);
        dialogs::confirm(question, window, cx);
    }

    fn refused(&mut self, error: &anyhow::Error, cx: &mut Context<Self>) {
        toast(error.to_string(), ToastKind::Error, cx);
        self.reread(cx);
    }

    pub(super) fn open_in_editor(workspace: u64, path: &str, line: u32, cx: &mut App) {
        let store = Store::get(cx);
        let s = store.read(cx);
        let mut a = args(&["new-tab", "--workspace", &workspace.to_string(), "--focus"]);
        if let Some(view) = s.view_id {
            a.extend(["--view".to_string(), view.to_string()]);
        }
        a.push("--".into());
        a.push(diff::editor_command(path, line));
        s.cmd(a);
    }

    pub(super) fn ask_agent(workspace: u64, reference: &str, cx: &mut App) {
        let store = Store::get(cx);
        let pane = store.read(cx).session.as_ref().and_then(|s| agent_pane(s, workspace));
        let Some((pane, agent)) = pane else {
            cx.write_to_clipboard(ClipboardItem::new_string(reference.to_string()));
            toast(format!("No agent in this workspace: copied {}", reference.trim_end()), ToastKind::Info, cx);
            return;
        };
        store.update(cx, |s, cx| {
            s.cmd(args(&["send", "--pane", &pane.to_string(), reference]));
            s.focus("pane", pane);
            s.toast(format!("Sent {} to {agent}", reference.trim_end()), ToastKind::Info, cx);
        });
    }
}

#[cfg(test)]
mod tests;
