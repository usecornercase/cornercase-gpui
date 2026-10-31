use gpui::{App, Context, Window};

use super::WorktreesPanel;
use super::data::{Freed, Worktree, free_command, free_message, remove_message};
use crate::dialogs::{self, Question};
use crate::panels::PanelKind;
use crate::panels::files::disk::human_size;
use crate::panels::poll::parse_text;
use crate::store::{self, Busy, Store, ToastKind};

fn toast(text: String, kind: ToastKind, cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| s.toast(text, kind, cx));
}

pub(super) fn checking(path: &str) -> String {
    format!("check-builds:{path}")
}

pub(super) fn freeing(path: &str) -> String {
    format!("free-builds:{path}")
}

fn after_free(this: &mut WorktreesPanel, result: anyhow::Result<String>, cx: &mut Context<WorktreesPanel>) {
    let (text, kind) = match parse_text::<Freed>(result) {
        Ok(freed) if freed.failed.is_empty() => (format!("Freed {}", human_size(freed.bytes)), ToastKind::Info),
        Ok(freed) => (format!("{} folders could not be deleted", freed.failed.len()), ToastKind::Error),
        Err(e) => (e, ToastKind::Error),
    };
    toast(text, kind, cx);
    this.reload(cx);
}

impl WorktreesPanel {
    pub(super) fn open_changes(workspace: u64, cx: &mut App) {
        Store::get(cx).update(cx, |s, cx| {
            s.focus("workspace", workspace);
            s.close_sheet(cx);
            s.open(PanelKind::Changes, cx);
        });
    }

    pub(super) fn ask_remove(worktrees: &[Worktree], window: &mut Window, cx: &mut Context<Self>) {
        let removed: Vec<Worktree> = worktrees.iter().filter(|w| w.remove_command().is_some()).cloned().collect();
        if removed.is_empty() {
            return;
        }
        let title = match removed.as_slice() {
            [one] => format!("Remove the worktree {}?", one.title()),
            many => format!("Remove {} worktrees?", many.len()),
        };
        let message = remove_message(&removed.iter().collect::<Vec<_>>());
        let this = cx.entity();
        let question = Question::new(title, message, "Remove", Vec::new()).on_yes(move |cx| {
            let removed = removed.clone();
            this.update(cx, |this, cx| removed.iter().for_each(|w| this.remove(w, cx)));
        });
        dialogs::confirm(question, window, cx);
    }

    fn remove(&mut self, worktree: &Worktree, cx: &mut Context<Self>) {
        let (Some(command), Some(workspace)) = (worktree.remove_command(), worktree.workspace) else { return };
        let path = worktree.path.clone();
        self.ticked.remove(&path);
        let busy = Busy::remove_worktree(workspace, &worktree.title());
        store::run_busy(cx, busy, command, move |this, result, cx| {
            if result.is_ok()
                && let Some(worktrees) = &mut this.worktrees
            {
                worktrees.retain(|w| w.path != path);
            }
            Store::get(cx).update(cx, |s, cx| s.settle_removal(workspace, &result, cx));
            this.reload(cx);
        });
    }

    pub(super) fn check_free(worktree: Worktree, cx: &mut Context<Self>) {
        let busy = Busy::new(checking(&worktree.path), format!("Measuring the builds of {}…", worktree.title()));
        store::run_busy(cx, busy, free_command(&worktree.path, true, false), move |_, result, cx| {
            match parse_text::<Freed>(result) {
                Ok(freed) => Self::ask_free(&worktree, &freed, cx),
                Err(e) => toast(e, ToastKind::Error, cx),
            }
        });
    }

    fn ask_free(worktree: &Worktree, freed: &Freed, cx: &mut Context<Self>) {
        let Some(window) = cx.active_window() else { return };
        let force = !freed.running.is_empty();
        let command = free_command(&worktree.path, false, force);
        let title = format!("Free the build folders of {}?", worktree.title());
        let button = if force { "Free anyway" } else { "Free" };
        let busy = Busy::new(freeing(&worktree.path), format!("Freeing the builds of {}…", worktree.title()));
        let this = cx.entity();
        let question = Question::new(title, free_message(freed), button, Vec::new()).on_yes(move |cx| {
            this.update(cx, |_, cx| store::run_busy(cx, busy.clone(), command.clone(), after_free));
        });
        let _ = window.update(cx, |_, window, cx| dialogs::confirm(question, window, cx));
    }
}
