use gpui::{App, AppContext as _, Context, Entity, Focusable as _, Subscription, Window};

use super::QueuePanel;
use super::data::Entry;
use super::draft::{Draft, Target, When};
use super::time::local;
use crate::model::Session;
use crate::store::{self, Store, ToastKind, args};
use crate::ui::{TextField, TextFieldEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Spot {
    Keep,
    Workspace(u64),
    NewWorktree,
}

impl From<Option<u64>> for Spot {
    fn from(workspace: Option<u64>) -> Self {
        workspace.map_or(Spot::NewWorktree, Spot::Workspace)
    }
}

pub(super) struct Form {
    pub(super) edit: Option<u64>,
    pub(super) issue: bool,
    pub(super) prompt: Entity<TextField>,
    pub(super) issue_key: Entity<TextField>,
    pub(super) source: Option<String>,
    pub(super) when: When,
    pub(super) clock: Entity<TextField>,
    pub(super) date: Entity<TextField>,
    pub(super) amount: Entity<TextField>,
    pub(super) unit: char,
    pub(super) project: Option<u64>,
    pub(super) spot: Spot,
    pub(super) branch: Entity<TextField>,
    pub(super) agent: Option<String>,
    pub(super) mode: Option<String>,
    pub(super) name: Entity<TextField>,
    pub(super) old_name: String,
    pub(super) old_prompt: String,
    pub(super) error: Option<String>,
    pub(super) busy: bool,
    _subscriptions: Vec<Subscription>,
}

#[derive(Default)]
struct Fields {
    subscriptions: Vec<Subscription>,
}

impl Fields {
    fn add(
        &mut self,
        placeholder: &str,
        text: &str,
        window: &mut Window,
        cx: &mut Context<QueuePanel>,
    ) -> Entity<TextField> {
        let field = cx.new(|cx| {
            let mut field = TextField::new(placeholder.to_string(), cx);
            if !text.is_empty() {
                field.set_text(text.to_string(), cx);
            }
            field
        });
        self.subscriptions.push(cx.subscribe_in(&field, window, |this, _, event, _, cx| match event {
            TextFieldEvent::Submit => this.save(cx),
            TextFieldEvent::Cancel => this.close_form(cx),
            TextFieldEvent::Changed => cx.notify(),
        }));
        field
    }
}

fn start_place(entry: Option<&Entry>, session: Option<&Session>) -> (Option<u64>, Spot) {
    let Some(entry) = entry else {
        let shown = session.map(Session::shown).unwrap_or_default();
        return (shown.project, shown.workspace.into());
    };
    let project = session.and_then(|s| s.projects.iter().find(|p| p.path == entry.place.project));
    (project.map(|p| p.id), Spot::Keep)
}

impl Form {
    fn new(
        entry: Option<&Entry>,
        session: Option<&Session>,
        window: &mut Window,
        cx: &mut Context<QueuePanel>,
    ) -> Self {
        let at = entry.and_then(|e| local(e.at));
        let mut f = Fields::default();
        let prompt = f.add("What the agent should do", entry.map_or("", |e| &e.prompt), window, cx);
        let issue_key = f.add("https://github.com/org/repo/issues/12, #12 or ENG-42", "", window, cx);
        let clock = f.add("14:30", &at.map(|c| c.clock()).unwrap_or_default(), window, cx);
        let date = f.add("2026-10-24", &at.map(|c| c.date()).unwrap_or_default(), window, cx);
        let amount = f.add("1", if entry.is_some() { "" } else { "1" }, window, cx);
        let branch = f.add("feat/my-branch", "", window, cx);
        let name = f.add("the tab's name (optional)", entry.map_or("", |e| &e.label), window, cx);
        let (project, spot) = start_place(entry, session);
        Self {
            edit: entry.map(|e| e.id),
            issue: false,
            prompt,
            issue_key,
            source: None,
            when: if entry.is_some() { When::Keep } else { When::In },
            clock,
            date,
            amount,
            unit: 'h',
            project,
            spot,
            branch,
            agent: None,
            mode: None,
            name,
            old_name: entry.map(|e| e.label.clone()).unwrap_or_default(),
            old_prompt: entry.map(|e| e.prompt.clone()).unwrap_or_default(),
            error: None,
            busy: false,
            _subscriptions: f.subscriptions,
        }
    }

    pub(super) fn editing(&self) -> bool {
        self.edit.is_some()
    }

    fn target(&self, cx: &App) -> Target {
        match (self.spot, self.project) {
            (Spot::Workspace(id), _) => Target::Workspace(id),
            (Spot::NewWorktree, Some(project)) => Target::Worktree { project, branch: self.branch.read(cx).text() },
            (Spot::Keep | Spot::NewWorktree, _) => Target::Keep,
        }
    }

    fn draft(&self, cx: &App) -> Draft {
        let text = |f: &Entity<TextField>| f.read(cx).text();
        let editing = self.editing();
        let mut body = if self.issue { text(&self.issue_key) } else { text(&self.prompt) };
        if editing && !self.issue && body.trim() == self.old_prompt.trim() {
            body.clear();
        }
        let name = text(&self.name);
        let name = if editing && name.trim() == self.old_name.trim() { None } else { Some(name) };
        Draft {
            edit: self.edit,
            issue: self.issue,
            text: body,
            source: self.source.clone(),
            when: self.when,
            clock: text(&self.clock),
            date: text(&self.date),
            amount: text(&self.amount),
            unit: self.unit,
            target: self.target(cx),
            agent: self.agent.clone(),
            mode: self.mode.clone(),
            name,
        }
    }
}

impl QueuePanel {
    pub(super) fn open_form(&mut self, entry: Option<&Entry>, window: &mut Window, cx: &mut Context<Self>) {
        if self.config.is_none() {
            Self::load_config(cx);
        }
        let session = Store::get(cx).read(cx).session.clone();
        let form = Form::new(entry, session.as_ref(), window, cx);
        let focus = form.prompt.read(cx).focus_handle(cx);
        self.form = Some(form);
        window.focus(&focus);
        cx.notify();
    }

    fn load_config(cx: &mut Context<Self>) {
        store::run_json(cx, args(&["config", "--json"]), |this: &mut Self, config, cx| {
            if let Ok(config) = config {
                this.config = Some(config);
                cx.notify();
            }
        });
    }

    pub(super) fn close_form(&mut self, cx: &mut Context<Self>) {
        self.form = None;
        cx.notify();
    }

    pub(super) fn save(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = self.form.as_ref().map(|f| f.draft(cx)) else { return };
        let Some(form) = self.form.as_mut().filter(|f| !f.busy) else { return };
        let command = match draft.args() {
            Ok(command) => command,
            Err(e) => {
                form.error = Some(e);
                cx.notify();
                return;
            }
        };
        form.busy = true;
        form.error = None;
        let editing = draft.edit.is_some();
        store::run_json(cx, command, move |this: &mut Self, answer, cx| match answer {
            Ok(_) => this.saved(editing, cx),
            Err(e) => this.save_failed(&e.to_string(), cx),
        });
        cx.notify();
    }

    fn saved(&mut self, editing: bool, cx: &mut Context<Self>) {
        self.form = None;
        let text = if editing { "Queue entry saved" } else { "Scheduled" };
        Store::get(cx).update(cx, |s, cx| s.toast(text, ToastKind::Done, cx));
        cx.notify();
    }

    fn save_failed(&mut self, error: &str, cx: &mut Context<Self>) {
        if let Some(form) = self.form.as_mut() {
            form.busy = false;
            form.error = Some(error.strip_prefix("cornercase queue: ").unwrap_or(error).to_string());
        }
        cx.notify();
    }

    pub(super) fn update_form(&mut self, change: impl Fn(&mut Form), cx: &mut Context<Self>) {
        if let Some(form) = self.form.as_mut() {
            change(form);
            form.error = None;
            cx.notify();
        }
    }
}
