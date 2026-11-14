use gpui::{App, Context, Entity};

use super::WorktreesPanel;
use super::data::Worktree;
use super::frees::{
    Asked, CHECKING_MERGED, Freed, Merged, ask_merged, checking, free_command, free_merged_command, free_message,
    free_title, freeing, freeing_label, merged_toast, row_toast,
};
use super::removal::{Answer, Folder, branch_label, button, cannot_go, check_command, said, summary, title};
use super::rows::said_lines;
use crate::dialogs::{self, Question, Tick};
use crate::panels::PanelKind;
use crate::panels::poll::parse_text;
use crate::store::{self, Busy, Store, ToastKind};

pub(super) const NOT_REMOVED: &str = "Nothing was removed: there is no window to ask in.";
const NOT_FREED: &str = "Nothing was freed: there is no window to ask in.";

fn toast(text: String, kind: ToastKind, cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| s.toast(text, kind, cx));
}

pub(super) fn ask(question: Question, unasked: &str, cx: &mut App) {
    let window = cx.active_window().or_else(|| cx.windows().first().copied());
    let asked =
        window.is_some_and(|window| window.update(cx, |_, window, cx| dialogs::confirm(question, window, cx)).is_ok());
    if !asked {
        toast(unasked.to_string(), ToastKind::Error, cx);
    }
}

pub(super) fn removing(path: &str) -> String {
    format!("remove-folder:{path}")
}

pub(super) fn removal_job(folder: &Folder) -> Busy {
    let title = folder.title();
    match folder.workspace() {
        Some(workspace) => Busy::remove_worktree(workspace, &title),
        None => Busy::new(removing(&folder.path), format!("Removing {title}…")),
    }
}

pub(super) fn removal_question(
    folders: Vec<Folder>,
    go: impl Fn(&[Folder], bool, &mut App) + Clone + 'static,
) -> Question {
    let (kept, deleted) = (said(&folders, false), said(&folders, true));
    let yes = move |folders: Vec<Folder>, branches: bool| {
        let go = go.clone();
        move |cx: &mut App| go(&folders, branches, cx)
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

fn after_free(name: &str, this: &mut WorktreesPanel, result: anyhow::Result<String>, cx: &mut Context<WorktreesPanel>) {
    let (text, kind) =
        parse_text::<Freed>(result).map_or_else(|e| (e, ToastKind::Error), |freed| row_toast(&freed, name));
    toast(text, kind, cx);
    this.reload(cx);
}

fn start_row_free(this: &Entity<WorktreesPanel>, busy: &Busy, command: &[String], name: &str, cx: &mut App) {
    let name = name.to_string();
    this.update(cx, |_, cx| {
        store::run_partial_busy(cx, busy.clone(), command.to_vec(), move |this, result, cx| {
            after_free(&name, this, result, cx);
        });
    });
}

fn start_merged_free(
    this: &Entity<WorktreesPanel>,
    job: &Busy,
    command: &[String],
    confirmed: Vec<String>,
    cx: &mut App,
) {
    this.update(cx, |_, cx| {
        store::run_partial_busy(cx, job.clone(), command.to_vec(), move |this, result, cx| {
            after_free_merged(&confirmed, this, result, cx);
        });
    });
}

fn after_free_merged(
    confirmed: &[String],
    this: &mut WorktreesPanel,
    result: anyhow::Result<String>,
    cx: &mut Context<WorktreesPanel>,
) {
    let rows = this.list.as_ref().map(|list| list.worktrees.as_slice()).unwrap_or_default();
    let (text, kind) = parse_text::<Merged>(result)
        .map_or_else(|e| (e, ToastKind::Error), |answer| merged_toast(&answer, rows, confirmed));
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
        let this = cx.entity();
        let go = move |folders: &[Folder], branches: bool, cx: &mut App| {
            this.update(cx, |this, cx| this.remove(folders, branches, cx));
        };
        ask(removal_question(folders, go), NOT_REMOVED, cx);
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
            if let Some(list) = &mut self.list {
                list.drop_row(&folder.path);
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
        let force = !freed.running.is_empty();
        let command = free_command(&worktree.path, false, force);
        let name = worktree.title();
        let button = if force { "Free anyway" } else { "Free" };
        let busy = Busy::new(freeing(&worktree.path), freeing_label(&name));
        let this = cx.entity();
        let question = Question::new(free_title(&name), free_message(freed), button, Vec::new())
            .on_yes(move |cx| start_row_free(&this, &busy, &command, &name, cx));
        ask(question, NOT_FREED, cx);
    }

    pub(super) fn check_free_merged(&mut self, cx: &mut Context<Self>) {
        let project = self.project;
        let busy = Busy::new(CHECKING_MERGED, "Measuring the builds of merged worktrees…");
        store::run_partial_busy(
            cx,
            busy,
            free_merged_command(true, project),
            move |this, result, cx| match parse_text::<Merged>(result) {
                Ok(answer) => this.ask_free_merged(&answer, project, cx),
                Err(e) => toast(e, ToastKind::Error, cx),
            },
        );
    }

    fn ask_free_merged(&mut self, answer: &Merged, project: Option<u64>, cx: &mut Context<Self>) {
        let filtering = !self.filter.read(cx).text().trim().is_empty();
        let rows = self.list.as_ref().map(|list| list.worktrees.as_slice()).unwrap_or_default();
        match ask_merged(answer, rows, filtering) {
            Asked::Nothing(text) => {
                toast(text, ToastKind::Info, cx);
                self.reload(cx);
            }
            Asked::Cannot(text) => toast(text, ToastKind::Error, cx),
            Asked::Question { title, lines, job, confirmed } => {
                let this = cx.entity();
                let command = free_merged_command(false, project);
                let question = Question::new(title, "", "Free", Vec::new())
                    .body(move |_| said_lines(&lines))
                    .on_yes(move |cx| start_merged_free(&this, &job, &command, confirmed.clone(), cx));
                ask(question, NOT_FREED, cx);
            }
        }
    }
}
