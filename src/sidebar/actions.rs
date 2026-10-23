use gpui::{AppContext as _, Context, Focusable as _, PathPromptOptions, Window};
use serde_json::Value;

use super::order::{self, Place};
use super::{Rename, Target, cmd};
use crate::app::Root;
use crate::model::Session;
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
    let sections = order::sections(session);
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

fn rename_placeholder(target: Target) -> &'static str {
    match target {
        Target::Group(_) => "group name",
        Target::Project(_) => "empty: the folder's name",
        Target::Workspace(_) => "empty: the branch's name",
        Target::Tab(_) => "empty: the program's name",
    }
}

impl Root {
    pub(crate) fn set_folded(&mut self, target: Target, collapsed: bool) {
        let verb = if collapsed { "collapse" } else { "expand" };
        self.queue.cmd(cmd(&[verb, target.flag(), &target.id().to_string()]));
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
        let field = cx.new(|cx| {
            let mut f = TextField::new(rename_placeholder(target), cx);
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
        self.side.rename = Some(Rename { target, field, initial });
        cx.notify();
    }

    pub(crate) fn finish_rename(&mut self, save: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rename) = self.side.rename.take() else { return };
        let text = rename.field.read(cx).text().trim().to_string();
        let empty_group = matches!(rename.target, Target::Group(_)) && text.is_empty();
        if save && text != rename.initial && !empty_group {
            self.act(vec!["rename".into(), rename.target.flag().into(), rename.target.id().to_string(), text]);
        }
        window.focus(&self.focus);
        cx.notify();
    }

    pub(crate) fn drop_on(&mut self, dragged: Target, target: Target) {
        let Some(session) = &self.session else { return };
        if let Some(args) = drop_command(session, dragged, target) {
            self.queue.cmd(args);
        }
    }

    #[expect(clippy::unused_self, reason = "a menu item's action and ⌘O call it as a method of the window")]
    pub(crate) fn open_project(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        Self::open_project_into(None, cx);
    }

    pub(super) fn open_project_into(group: Option<u64>, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(if group.is_some() { "Add project".into() } else { "Open project".into() }),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else { return };
            let Some(path) = paths.first() else { return };
            let path = path.to_string_lossy().into_owned();
            let _ = this.update(cx, |this, cx| this.open_folder(path, group, cx));
        })
        .detach();
    }

    fn open_folder(&mut self, path: String, group: Option<u64>, cx: &mut Context<Self>) {
        let Some(group) = group else {
            self.act(vec!["open".into(), path, "--focus".into()]);
            return;
        };
        let mut args = vec!["open".into(), path, "--focus".into(), "--json".into()];
        if let Some(view) = self.view_id {
            args.extend(["--view".into(), view.to_string()]);
        }
        crate::store::run_json(cx, args, move |this: &mut Root, result, cx| this.opened_into(result, group, cx));
    }

    fn opened_into(&mut self, result: anyhow::Result<Value>, group: u64, cx: &mut Context<Self>) {
        match result {
            Ok(v) => {
                let Some(project) = v.get("project").and_then(Value::as_u64) else { return };
                self.queue.cmd(into_group(project, group));
                self.set_folded(Target::Group(group), false);
            }
            Err(e) => self.say(e.to_string(), cx),
        }
    }
}
