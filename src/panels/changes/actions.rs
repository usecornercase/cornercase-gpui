use gpui::{App, ClipboardItem, Context, Window};

use crate::dialogs;
use crate::model::Session;
use crate::store::{Store, ToastKind, args};

use super::pieces::{find_mut, toast};
use super::wire::{FileInfo, FileStatus, Staging};
use super::{ChangesPanel, diff};

const DISCARD_CAVEAT: &str = "It discards the file as it is now, even if it changed since this diff was read \
     (the command line cannot name the version seen: usecornercase/cornercase-terminal#470).";

fn agent_pane(session: &Session, workspace: u64) -> Option<(u64, String)> {
    let (_, ws) = session.workspace(workspace)?;
    let mut tabs: Vec<_> = ws.tabs.iter().collect();
    tabs.sort_by_key(|t| !t.active);
    tabs.iter().flat_map(|t| &t.panes).find_map(|p| Some((p.id, p.agent.clone()?)))
}

impl ChangesPanel {
    pub(super) fn toggle_viewed(&mut self, workspace: u64, path: &str, cx: &mut Context<Self>) {
        let mode = self.scope.mode();
        let Some(file) = self.summary.as_mut().and_then(|s| find_mut(s, workspace, path)) else { return };
        let mut a = args(&["changes", "viewed", path, "--workspace", &workspace.to_string(), "--mode", mode.name()]);
        if file.viewed {
            a.push("--clear".into());
        }
        file.viewed = !file.viewed;
        Store::get(cx).read(cx).cmd(a);
        cx.notify();
    }

    pub(super) fn toggle_staged(workspace: u64, file: &FileInfo, cx: &mut App) {
        let verb = if file.staged == Some(Staging::Yes) { "unstage" } else { "stage" };
        Store::get(cx).read(cx).cmd(args(&["changes", verb, &file.path, "--workspace", &workspace.to_string()]));
    }

    pub(super) fn discard(workspace: u64, file: &FileInfo, window: &mut Window, cx: &mut App) {
        let deletes = matches!(file.status, FileStatus::Untracked | FileStatus::Added);
        let (title, button) =
            if deletes { ("Delete the file?", "Delete") } else { ("Discard the changes?", "Discard") };
        dialogs::confirm(
            title,
            format!("{}\n\n{DISCARD_CAVEAT}", diff::discard_question(file)),
            button,
            args(&["changes", "discard", &file.path, "--workspace", &workspace.to_string(), "--yes"]),
            false,
            window,
            cx,
        );
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
