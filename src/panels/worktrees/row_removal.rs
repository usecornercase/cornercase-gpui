use gpui::{
    App, AppContext as _, Context, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    Window, div,
};

use super::actions::{NOT_REMOVED, ask, removal_job, removal_question};
use super::removal::{Answer, Folder, cannot_go, check_command, summary};
use crate::cli::{self, Target};
use crate::dialogs;
use crate::model::Aim;
use crate::panels::poll::parse_text;
use crate::store::{self, Busy, Store, ToastKind};
use crate::ui;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Whose {
    Here,
    There(Aim),
}

#[derive(Clone, Debug)]
pub(crate) struct WorktreeRow {
    pub(crate) whose: Whose,
    pub(crate) path: String,
    pub(crate) name: String,
}

struct Checking {
    row: WorktreeRow,
    focus: FocusHandle,
}

fn place_name(row: &WorktreeRow) -> String {
    match &row.whose {
        Whose::Here => row.name.clone(),
        Whose::There(aim) => format!("{} on {}", row.name, aim.machine),
    }
}

fn loading_title(row: &WorktreeRow) -> String {
    format!("Remove the worktree {}?", place_name(row))
}

fn check_job(row: &WorktreeRow) -> Busy {
    Busy::new(format!("check-removal:{}", row.path), format!("Checking what removing {} would lose…", place_name(row)))
}

fn checked(row: &WorktreeRow, result: anyhow::Result<String>) -> Folder {
    Folder::asked(&row.path, row.whose.machine()).answered(parse_text::<Answer>(result))
}

impl Whose {
    fn machine(&self) -> Option<&str> {
        match self {
            Self::Here => None,
            Self::There(aim) => Some(&aim.machine),
        }
    }

    fn target_in(&self, store: &Store) -> Result<Target, String> {
        match self {
            Self::Here => Ok(cli::target()),
            Self::There(aim) => store.reach_machine(aim),
        }
    }

    fn job(&self, folder: &Folder) -> Busy {
        match self {
            Self::Here => removal_job(folder),
            Self::There(aim) => Busy::leaving(aim, format!("Removing {}…", folder.title())),
        }
    }

    fn going(self) -> impl Fn(&[Folder], bool, &mut App) + Clone + 'static {
        move |folders: &[Folder], branches: bool, cx: &mut App| {
            for folder in folders.iter().filter(|folder| folder.goes()) {
                self.remove(folder, branches, cx);
            }
        }
    }

    fn remove(&self, folder: &Folder, branches: bool, cx: &mut App) {
        let (whose, asked) = (self.clone(), folder.clone());
        let (command, busy) = (folder.remove_command(branches), self.job(folder));
        Store::get(cx).update(cx, move |s, cx| match whose.target_in(s) {
            Ok(target) => {
                let work = move || cli::run_partial_on(&target, &command);
                s.run_then(busy, work, move |s, result, cx| whose.settle(s, &asked, result, cx), cx);
            }
            Err(why) => s.not_sent(&why, cx),
        });
    }

    fn settle(&self, s: &mut Store, asked: &Folder, result: anyhow::Result<String>, cx: &mut Context<Store>) {
        let folder = asked.clone().answered(parse_text::<Answer>(result));
        if folder.removed() {
            self.went(s, &folder, cx);
        }
        let (text, kind) = summary(std::slice::from_ref(&folder));
        s.toast(text, kind, cx);
    }

    fn went(&self, s: &mut Store, folder: &Folder, cx: &mut Context<Store>) {
        match (self, folder.workspace()) {
            (Self::Here, Some(workspace)) => s.worktree_went(workspace, cx),
            (Self::There(aim), _) => s.worktree_went_there(aim.clone(), cx),
            (Self::Here, None) => {}
        }
    }
}

pub(crate) fn remove_worktree(row: WorktreeRow, window: &mut Window, cx: &mut App) {
    let target = row.whose.target_in(Store::get(cx).read(cx));
    let target = match target {
        Ok(target) => target,
        Err(why) => return Store::get(cx).update(cx, |s, cx| s.not_sent(&why, cx)),
    };
    let view = cx.new(|cx| Checking { row, focus: cx.focus_handle() });
    view.update(cx, |this, cx| this.check(target, cx));
    let focus = view.read(cx).focus.clone();
    dialogs::show(view.into(), &focus, window, cx);
}

impl Checking {
    fn check(&mut self, target: Target, cx: &mut Context<Self>) {
        let command = check_command(std::slice::from_ref(&self.row.path));
        store::run_partial_on_busy(cx, check_job(&self.row), target, command, Self::heard);
    }

    fn heard(&mut self, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        let folder = checked(&self.row, result);
        if !folder.goes() {
            let text = cannot_go(std::slice::from_ref(&folder));
            Store::get(cx).update(cx, |s, cx| {
                s.toast(text, ToastKind::Error, cx);
                s.close_dialog(cx);
            });
            return;
        }
        ask(removal_question(vec![folder], self.row.whose.clone().going()), NOT_REMOVED, cx);
    }
}

impl Render for Checking {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let waiting = ui::busy("checking", "Checking what removing it would lose…");
        let buttons = dialogs::cancel_and(ui::busy_button("remove", "Checking…", ui::Tone::Danger));
        div().track_focus(&self.focus).child(dialogs::frame(loading_title(&self.row), waiting, buttons))
    }
}

#[cfg(test)]
mod tests;
