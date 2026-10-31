use crate::model::{Project, Session, Workspace};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fold {
    Group(u64),
    Project(u64),
    Workspace(u64),
    Pane(u64),
}

impl Fold {
    pub(crate) fn args(self, collapsed: bool) -> Vec<String> {
        let (flag, id) = match self {
            Fold::Group(id) => ("--group", id),
            Fold::Project(id) => ("--project", id),
            Fold::Workspace(id) => ("--workspace", id),
            Fold::Pane(id) => ("--pane", id),
        };
        let verb = if collapsed { "collapse" } else { "expand" };
        super::cmd(&[verb, flag, &id.to_string()])
    }

    fn flag_in(self, session: &mut Session) -> Option<&mut bool> {
        let projects = &mut session.projects;
        match self {
            Fold::Group(id) => session.groups.iter_mut().find(|g| g.id == id).map(|g| &mut g.collapsed),
            Fold::Project(id) => projects.iter_mut().find(|p| p.id == id).map(|p| &mut p.collapsed),
            Fold::Workspace(id) => workspaces(projects).find(|w| w.id == id).map(|w| &mut w.collapsed),
            Fold::Pane(id) => workspaces(projects)
                .flat_map(|w| &mut w.tabs)
                .flat_map(|t| &mut t.panes)
                .find(|p| p.id == id)
                .map(|p| &mut p.collapsed),
        }
    }
}

fn workspaces(projects: &mut [Project]) -> impl Iterator<Item = &mut Workspace> {
    projects.iter_mut().flat_map(|p| &mut p.workspaces)
}

#[derive(Debug, Clone, Copy)]
struct Pending {
    fold: Fold,
    collapsed: bool,
    answered: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Folds {
    pending: Vec<Pending>,
}

impl Folds {
    pub(crate) fn set(&mut self, fold: Fold, collapsed: bool, session: Option<&mut Session>) {
        self.pending.retain(|p| p.fold != fold);
        self.pending.push(Pending { fold, collapsed, answered: false });
        if let Some(flag) = session.and_then(|s| fold.flag_in(s)) {
            *flag = collapsed;
        }
    }

    pub(crate) fn answered(&mut self, fold: Fold, collapsed: bool, ok: bool, session: Option<&mut Session>) {
        let Some(at) = self.pending.iter().position(|p| p.fold == fold && p.collapsed == collapsed) else { return };
        if ok {
            self.pending[at].answered = true;
            return;
        }
        self.pending.remove(at);
        if let Some(flag) = session.and_then(|s| fold.flag_in(s)) {
            *flag = !collapsed;
        }
    }

    pub(crate) fn apply(&mut self, session: &mut Session) {
        self.pending.retain(|p| {
            let Some(flag) = p.fold.flag_in(session) else { return false };
            if *flag == p.collapsed || p.answered {
                return false;
            }
            *flag = p.collapsed;
            true
        });
    }
}

#[cfg(test)]
mod tests;
