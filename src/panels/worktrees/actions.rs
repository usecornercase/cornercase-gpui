use gpui::{App, Context};

use super::WorktreesPanel;
use super::data::{Freed, Worktree, free_command, free_message};
use super::removal::{Answer, Folder, branch_label, button, cannot_go, check_command, said, summary, title};
use super::rows::said_lines;
use crate::dialogs::{self, Question, Tick};
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

pub(super) fn removing(path: &str) -> String {
    format!("remove-folder:{path}")
}

fn removal_job(folder: &Folder) -> Busy {
    let title = folder.title();
    match folder.workspace() {
        Some(workspace) => Busy::remove_worktree(workspace, &title),
        None => Busy::new(removing(&folder.path), format!("Removing {title}…")),
    }
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

    pub(super) fn check_remove(&mut self, paths: Vec<String>, cx: &mut Context<Self>) {
        if !self.asking.is_empty() {
            return;
        }
        let command = check_command(&paths);
        self.asking = paths;
        let busy = Busy::new("check-removal", "Checking what removing would lose…");
        store::run_partial_busy(cx, busy, command, |this, result, cx| {
            this.asking.clear();
            cx.notify();
            match parse_text::<Answer>(result) {
                Ok(answer) => this.ask_remove(answer.folders, cx),
                Err(e) => toast(e, ToastKind::Error, cx),
            }
        });
    }

    fn ask_remove(&mut self, folders: Vec<Folder>, cx: &mut Context<Self>) {
        if !self.open || folders.is_empty() {
            return;
        }
        if !folders.iter().any(Folder::goes) {
            toast(cannot_go(&folders), ToastKind::Error, cx);
            self.reload(cx);
            return;
        }
        let Some(window) = cx.active_window() else { return };
        let question = Self::remove_question(folders, cx);
        let _ = window.update(cx, |_, window, cx| dialogs::confirm(question, window, cx));
    }

    fn remove_question(folders: Vec<Folder>, cx: &mut Context<Self>) -> Question {
        let (kept, deleted) = (said(&folders, false), said(&folders, true));
        let this = cx.entity();
        let yes = move |folders: Vec<Folder>, branches: bool| {
            let this = this.clone();
            move |cx: &mut App| this.update(cx, |this, cx| this.remove(&folders, branches, cx))
        };
        let label = branch_label(&folders);
        let question = Question::new(title(&folders), "", button(&folders, false), Vec::new())
            .body(move |branches| said_lines(if branches { &deleted } else { &kept }))
            .on_yes(yes(folders.clone(), false));
        match label {
            Some(label) => question.tick(Tick::new(label, button(&folders, true), yes(folders, true))),
            None => question,
        }
    }

    fn remove(&mut self, folders: &[Folder], branches: bool, cx: &mut Context<Self>) {
        for folder in folders.iter().filter(|folder| folder.goes()) {
            self.ticked.remove(&folder.path);
            self.removals.running += 1;
            let asked = folder.clone();
            store::run_partial_busy(
                cx,
                removal_job(folder),
                folder.remove_command(branches),
                move |this, result, cx| {
                    this.removal_answered(asked.answered(parse_text::<Answer>(result)), cx);
                },
            );
        }
    }

    fn removal_answered(&mut self, folder: Folder, cx: &mut Context<Self>) {
        if folder.removed() {
            if let Some(worktrees) = &mut self.worktrees {
                worktrees.retain(|w| w.path != folder.path);
            }
            if let Some(workspace) = folder.workspace() {
                Store::get(cx).update(cx, |s, cx| s.worktree_went(workspace, cx));
            }
        }
        self.removals.answers.push(folder);
        self.removals.running -= 1;
        if self.removals.running == 0 {
            let (text, kind) = summary(&std::mem::take(&mut self.removals.answers));
            toast(text, kind, cx);
            self.reload(cx);
        }
        cx.notify();
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
