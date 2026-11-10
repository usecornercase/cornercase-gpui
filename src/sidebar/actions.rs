use gpui::{AppContext as _, Context, Focusable as _, Window};

use super::order::{self, Place};
use super::{Fold, Rename, Renaming, Target, cmd};
use crate::app::{Msg, Root};
use crate::model::Session;
use crate::store::MachineAct;
use crate::ui::{TextField, TextFieldEvent};

pub(super) fn into_group(project: u64, group: u64) -> Vec<String> {
    cmd(&["move", "--project", &project.to_string(), "--into", &group.to_string()])
}

pub(super) fn ungroup(project: u64) -> Vec<String> {
    cmd(&["move", "--project", &project.to_string(), "--ungroup"])
}

pub(super) fn place_args(dragged: Target, place: Place) -> Vec<String> {
    let id = dragged.id().to_string();
    match place {
        Place::Before(before) => cmd(&["move", dragged.flag(), &id, "--before", &before.to_string()]),
        Place::End => cmd(&["move", dragged.flag(), &id, "--last"]),
    }
}

fn existing_group_of(session: &Session, project: u64) -> Option<u64> {
    let group = session.projects.iter().find(|p| p.id == project).and_then(|p| p.group)?;
    session.groups.iter().any(|g| g.id == group).then_some(group)
}

fn project_drop(session: &Session, project: u64, target: u64) -> Option<Vec<String>> {
    let sections = session.sections();
    let siblings = sections.into_iter().find(|(_, ids)| ids.contains(&target)).map(|(_, ids)| ids).unwrap_or_default();
    order::drop_place(&siblings, project, target).map(|place| match place {
        Place::Before(_) => place_args(Target::Project(project), place),
        Place::End => match existing_group_of(session, target) {
            Some(group) => into_group(project, group),
            None => ungroup(project),
        },
    })
}

fn group_drop(session: &Session, group: u64, target: Target) -> Option<Vec<String>> {
    let group_ids: Vec<u64> = session.groups.iter().map(|g| g.id).collect();
    let onto = match target {
        Target::Project(project) => existing_group_of(session, project),
        other => Some(other.id()),
    };
    let place = order::drop_place(&group_ids, group, onto?)?;
    Some(place_args(Target::Group(group), place))
}

fn within(siblings: &[u64], dragged: Target, target: u64) -> Option<Vec<String>> {
    if !siblings.contains(&dragged.id()) {
        return None;
    }
    order::drop_place(siblings, dragged.id(), target).map(|place| place_args(dragged, place))
}

fn workspace_siblings(session: &Session, workspace: u64) -> Vec<u64> {
    session
        .projects
        .iter()
        .find(|p| p.workspaces.iter().any(|w| w.id == workspace))
        .map(|p| p.workspaces.iter().map(|w| w.id).collect())
        .unwrap_or_default()
}

fn tab_siblings(session: &Session, tab: u64) -> Vec<u64> {
    session
        .projects
        .iter()
        .flat_map(|p| &p.workspaces)
        .find(|w| w.tabs.iter().any(|t| t.id == tab))
        .map(|w| w.tabs.iter().map(|t| t.id).collect())
        .unwrap_or_default()
}

fn drop_command(session: &Session, dragged: Target, target: Target) -> Option<Vec<String>> {
    match (dragged, target) {
        (Target::Project(p), Target::Group(g)) => Some(into_group(p, g)),
        (Target::Project(p), Target::Project(q)) => project_drop(session, p, q),
        (Target::Group(a), Target::Group(_) | Target::Project(_)) => group_drop(session, a, target),
        (Target::Workspace(_), Target::Workspace(b)) => within(&workspace_siblings(session, b), dragged, b),
        (Target::Tab(_), Target::Tab(b)) => within(&tab_siblings(session, b), dragged, b),
        _ => None,
    }
}

fn rename_placeholder(what: &Renaming) -> &'static str {
    match what {
        Renaming::Here(Target::Group(_)) => "group name",
        Renaming::Here(Target::Project(_)) => "empty: the folder's name",
        Renaming::Here(Target::Workspace(_)) | Renaming::There(_) => "empty: the branch's name",
        Renaming::Here(Target::Tab(_)) => "empty: the program's name",
    }
}

impl Root {
    pub(crate) fn set_folded(&mut self, fold: Fold, collapsed: bool) {
        self.side.folds.set(fold, collapsed, self.session.get_mut().as_mut());
        let tx = self.tx.clone();
        self.own.queue.cmd_then(fold.args(collapsed), move |result| {
            let _ = tx.unbounded_send(Msg::Folded(fold, collapsed, result.err().map(|e| e.to_string())));
        });
    }

    pub(crate) fn fold_answered(&mut self, fold: Fold, collapsed: bool, error: Option<&str>, cx: &mut Context<Self>) {
        self.side.folds.answered(fold, collapsed, error.is_none(), self.session.get_mut().as_mut());
        if let Some(error) = error {
            self.say(error, cx);
        }
    }

    fn current_name(&self, target: Target) -> Option<String> {
        let session = self.session.as_ref()?;
        let projects = &session.projects;
        let workspaces = || projects.iter().flat_map(|p| &p.workspaces);
        match target {
            Target::Group(id) => session.groups.iter().find(|g| g.id == id).map(|g| g.name.clone()),
            Target::Project(id) => {
                projects.iter().find(|p| p.id == id).map(|p| p.custom_name.clone().unwrap_or(p.name.clone()))
            }
            Target::Workspace(id) => {
                workspaces().find(|w| w.id == id).map(|w| w.custom_name.clone().unwrap_or(w.name.clone()))
            }
            Target::Tab(id) => workspaces()
                .flat_map(|w| &w.tabs)
                .find(|t| t.id == id)
                .map(|t| t.custom_name.clone().unwrap_or(t.name.clone())),
        }
    }

    pub(crate) fn start_rename(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        let Some(initial) = self.current_name(target) else { return };
        self.open_rename(Renaming::Here(target), initial, window, cx);
    }

    pub(super) fn open_rename(&mut self, what: Renaming, initial: String, window: &mut Window, cx: &mut Context<Self>) {
        let field = cx.new(|cx| {
            let mut f = TextField::new(rename_placeholder(&what), cx);
            f.set_text(initial.clone(), cx);
            f
        });
        cx.subscribe_in(&field, window, |this, _, event, window, cx| match event {
            TextFieldEvent::Submit => this.finish_rename(true, window, cx),
            TextFieldEvent::Cancel => this.finish_rename(false, window, cx),
            TextFieldEvent::Changed => {}
        })
        .detach();
        window.focus(&field.read(cx).focus_handle(cx));
        self.side.rename = Some(Rename { what, field, initial });
        cx.notify();
    }

    pub(crate) fn finish_rename(&mut self, save: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rename) = self.side.rename.take() else { return };
        let text = rename.field.read(cx).text().trim().to_string();
        if save && text != rename.initial {
            self.send_rename(rename.what, text, cx);
        }
        window.focus(&self.focus);
        cx.notify();
    }

    fn send_rename(&mut self, what: Renaming, text: String, cx: &mut Context<Self>) {
        match what {
            Renaming::Here(Target::Group(_)) if text.is_empty() => {}
            Renaming::Here(target) => {
                self.act(vec!["rename".into(), target.flag().into(), target.id().to_string(), text]);
            }
            Renaming::There(aim) => {
                self.store.update(cx, |s, cx| s.act_on_machine(aim, MachineAct::Rename(text), cx));
            }
        }
    }

    pub(crate) fn drop_on(&mut self, dragged: Target, target: Target) {
        let Some(session) = self.session.as_ref() else { return };
        if let Some(args) = drop_command(session, dragged, target) {
            self.own.queue.cmd(args);
        }
    }
}
