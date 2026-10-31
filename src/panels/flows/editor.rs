use std::time::Duration;

use gpui::{App, AppContext as _, Context, Entity, WeakEntity, Window};
use serde_json::Value;

use super::FlowsPanel;
use super::actions::{Imported, announce, run_stdin};
use super::draft::{
    Draft, Place, RoleDraft, Saved, StageDraft, fresh_role_name, has_breaks, parse_saved, place_of, removed_from,
    renamed_in, save_args,
};
use super::model::{self, FlowInfo};
use crate::ui::{TextField, TextFieldEvent};

const CHECK_DELAY: Duration = Duration::from_millis(400);

pub(super) enum Text {
    Field(Entity<TextField>),
    Kept(String),
}

impl Text {
    fn new(text: &str, placeholder: &str, window: &mut Window, cx: &mut Context<FlowEditor>) -> Self {
        if has_breaks(text) { Self::Kept(text.to_string()) } else { Self::Field(input(text, placeholder, window, cx)) }
    }

    pub(super) fn value(&self, cx: &App) -> String {
        match self {
            Self::Field(field) => field.read(cx).text(),
            Self::Kept(text) => text.clone(),
        }
    }
}

pub(super) struct RoleFields {
    base: Value,
    pub name: Entity<TextField>,
    pub agent: Entity<TextField>,
    pub model: Entity<TextField>,
    pub effort: Entity<TextField>,
    pub prompt: Text,
    pub edits: bool,
    pub own_worktree: bool,
    last_name: String,
}

impl RoleFields {
    fn new(role: &RoleDraft, window: &mut Window, cx: &mut Context<FlowEditor>) -> Self {
        Self {
            base: role.base.clone(),
            name: input(&role.name, "Name", window, cx),
            agent: input(&role.agent, "claude, codex, gemini…", window, cx),
            model: input(&role.model, "The agent's default", window, cx),
            effort: input(&role.effort, "The agent's default", window, cx),
            prompt: Text::new(&role.prompt, "What this role does", window, cx),
            edits: role.edits,
            own_worktree: role.own_worktree,
            last_name: role.name.clone(),
        }
    }

    fn draft(&self, cx: &App) -> RoleDraft {
        RoleDraft {
            base: self.base.clone(),
            name: self.name.read(cx).text(),
            agent: self.agent.read(cx).text(),
            model: self.model.read(cx).text(),
            effort: self.effort.read(cx).text(),
            prompt: self.prompt.value(cx),
            edits: self.edits,
            own_worktree: self.own_worktree,
        }
    }

    fn current_name(&self, cx: &App) -> String {
        self.name.read(cx).text().trim().to_string()
    }
}

pub(super) struct StageFields {
    base: Value,
    pub roles: Entity<TextField>,
    pub back_to: Entity<TextField>,
    pub when: Entity<TextField>,
    pub at_most: Entity<TextField>,
}

impl StageFields {
    fn new(stage: &StageDraft, window: &mut Window, cx: &mut Context<FlowEditor>) -> Self {
        Self {
            base: stage.base.clone(),
            roles: input(&stage.roles, "coder, reviewer", window, cx),
            back_to: input(&stage.back_to, "No loop", window, cx),
            when: input(&stage.when, "findings", window, cx),
            at_most: input(&stage.at_most, "2", window, cx),
        }
    }

    fn draft(&self, cx: &App) -> StageDraft {
        StageDraft {
            base: self.base.clone(),
            roles: self.roles.read(cx).text(),
            back_to: self.back_to.read(cx).text(),
            when: self.when.read(cx).text(),
            at_most: self.at_most.read(cx).text(),
        }
    }
}

pub(super) enum Verdict {
    Waiting,
    Checking,
    Checked(Saved),
    Invalid(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Editing,
    Saving,
    Discarding,
}

pub(super) struct Opening {
    pub draft: Draft,
    pub place: Place,
    pub title: String,
    pub note: Option<String>,
}

pub(super) struct FlowEditor {
    panel: WeakEntity<FlowsPanel>,
    pub project: Option<u64>,
    pub title: String,
    pub note: Option<String>,
    pub place: Place,
    pub new: bool,
    original: Result<Value, String>,
    base: Value,
    pub name: Entity<TextField>,
    pub description: Text,
    pub notes: Text,
    pub commits: bool,
    pub orchestrator: RoleFields,
    pub roles: Vec<RoleFields>,
    pub stages: Vec<StageFields>,
    pub verdict: Verdict,
    generation: u64,
    pub mode: Mode,
    pub error: Option<String>,
}

fn input(text: &str, placeholder: &str, window: &mut Window, cx: &mut Context<FlowEditor>) -> Entity<TextField> {
    let text = text.to_string();
    let field = cx.new(|cx| {
        let mut field = TextField::new(placeholder.to_string(), cx).in_tab_order();
        field.set_text(text, cx);
        field
    });
    cx.subscribe_in(&field, window, |this: &mut FlowEditor, _, event, _, cx| match event {
        TextFieldEvent::Submit => this.save(cx),
        TextFieldEvent::Changed => this.changed(cx),
        TextFieldEvent::Cancel => {}
    })
    .detach();
    field
}

impl FlowsPanel {
    pub(super) fn open_editor(&mut self, flow: Option<&FlowInfo>, window: &mut Window, cx: &mut Context<Self>) {
        let opening = match flow {
            Some(flow) => editing(flow),
            None => self.creating(),
        };
        let panel = cx.entity().downgrade();
        let project = self.project;
        let editor = cx.new(|cx| FlowEditor::new(panel, project, opening, window, cx));
        self.editor = Some(editor);
        cx.notify();
    }

    fn creating(&self) -> Opening {
        let taken = self.taken_names();
        let name = model::unique_name("New flow", &taken.iter().map(String::as_str).collect::<Vec<_>>());
        Opening { draft: Draft::blank(&name), place: Place::Yours, title: "New flow".into(), note: None }
    }

    pub(super) fn close_editor(&mut self, saved: Option<&Saved>, cx: &mut Context<Self>) {
        self.editor = None;
        if let Some(saved) = saved {
            let imported = Imported {
                name: saved.name.clone(),
                origin: saved.origin.clone(),
                path: saved.path.clone(),
                note: None,
            };
            self.select_imported(&imported);
            self.refresh(cx);
        }
        cx.notify();
    }
}

fn editing(flow: &FlowInfo) -> Opening {
    let note = (flow.origin == "built-in")
        .then(|| format!("{} is built in: saving makes your version of it, as Settings › Flows does.", flow.name));
    Opening { draft: Draft::from_value(&flow.raw), place: place_of(flow), title: format!("Edit {}", flow.name), note }
}

impl FlowEditor {
    fn new(
        panel: WeakEntity<FlowsPanel>,
        project: Option<u64>,
        opening: Opening,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let Opening { draft, place, title, note } = opening;
        let mut this = Self {
            panel,
            project,
            title,
            note,
            new: place == Place::Yours && draft.base.is_null(),
            place,
            original: draft.to_value(),
            base: draft.base.clone(),
            name: input(&draft.name, "Name", window, cx),
            description: Text::new(&draft.description, "What the flow is for", window, cx),
            notes: Text::new(&draft.notes, "Notes for the orchestrator (optional)", window, cx),
            commits: draft.commits,
            orchestrator: RoleFields::new(&draft.orchestrator, window, cx),
            roles: draft.roles.iter().map(|r| RoleFields::new(r, window, cx)).collect(),
            stages: draft.stages.iter().map(|s| StageFields::new(s, window, cx)).collect(),
            verdict: Verdict::Waiting,
            generation: 0,
            mode: Mode::Editing,
            error: None,
        };
        this.schedule_check(cx);
        this
    }

    fn draft(&self, cx: &App) -> Draft {
        Draft {
            base: self.base.clone(),
            name: self.name.read(cx).text(),
            description: self.description.value(cx),
            notes: self.notes.value(cx),
            commits: self.commits,
            orchestrator: self.orchestrator.draft(cx),
            roles: self.roles.iter().map(|r| r.draft(cx)).collect(),
            stages: self.stages.iter().map(|s| s.draft(cx)).collect(),
        }
    }

    pub(super) fn dirty(&self, cx: &App) -> bool {
        self.new || self.draft(cx).to_value() != self.original
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.follow_renames(cx);
        if self.mode == Mode::Discarding {
            self.mode = Mode::Editing;
        }
        self.schedule_check(cx);
    }

    fn follow_renames(&mut self, cx: &mut Context<Self>) {
        for i in 0..self.roles.len() {
            let name = self.roles[i].current_name(cx);
            if name.is_empty() || name == self.roles[i].last_name {
                continue;
            }
            let old = std::mem::replace(&mut self.roles[i].last_name, name.clone());
            self.rewrite_stages(|list| renamed_in(list, &old, &name), cx);
        }
    }

    fn rewrite_stages(&self, rewrite: impl Fn(&str) -> Option<String>, cx: &mut Context<Self>) {
        for stage in &self.stages {
            if let Some(list) = rewrite(&stage.roles.read(cx).text()) {
                stage.roles.update(cx, |field, cx| field.set_text(list, cx));
            }
        }
    }

    pub(super) fn schedule_check(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(CHECK_DELAY).await;
            let _ = this.update(cx, |this, cx| this.check(generation, cx));
        })
        .detach();
        cx.notify();
    }

    fn check(&mut self, generation: u64, cx: &mut Context<Self>) {
        if generation != self.generation {
            return;
        }
        let value = match self.draft(cx).to_value() {
            Ok(value) => value,
            Err(e) => {
                self.verdict = Verdict::Invalid(e);
                cx.notify();
                return;
            }
        };
        self.verdict = Verdict::Checking;
        let command = save_args(&self.place, self.project, true);
        run_stdin(cx, command, value.to_string(), move |this, result, cx| this.checked(generation, result, cx));
        cx.notify();
    }

    fn checked(&mut self, generation: u64, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        if generation != self.generation {
            return;
        }
        self.verdict = match result.and_then(|out| parse_saved(&out)) {
            Ok(saved) => Verdict::Checked(saved),
            Err(e) => Verdict::Invalid(e.to_string()),
        };
        cx.notify();
    }

    pub(super) fn save(&mut self, cx: &mut Context<Self>) {
        if self.mode == Mode::Saving {
            return;
        }
        let value = match self.draft(cx).to_value() {
            Ok(value) => value,
            Err(e) => {
                self.error = Some(e);
                cx.notify();
                return;
            }
        };
        self.mode = Mode::Saving;
        self.error = None;
        run_stdin(cx, save_args(&self.place, self.project, false), value.to_string(), Self::saved);
        cx.notify();
    }

    fn saved(&mut self, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        self.mode = Mode::Editing;
        match result.and_then(|out| parse_saved(&out)) {
            Ok(saved) => {
                announce(saved.check.as_deref(), format!("Saved {}", saved.name), cx);
                let _ = self.panel.update(cx, |panel, cx| panel.close_editor(Some(&saved), cx));
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        cx.notify();
    }

    pub(super) fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.dirty(cx) && self.mode == Mode::Editing {
            self.mode = Mode::Discarding;
            cx.notify();
            return;
        }
        let _ = self.panel.update(cx, |panel, cx| panel.close_editor(None, cx));
    }

    pub(super) fn keep_editing(&mut self, cx: &mut Context<Self>) {
        self.mode = Mode::Editing;
        cx.notify();
    }

    pub(super) fn set_place(&mut self, place: Place, cx: &mut Context<Self>) {
        self.place = place;
        self.schedule_check(cx);
    }

    pub(super) fn toggle_commits(&mut self, commits: bool, cx: &mut Context<Self>) {
        self.commits = commits;
        self.schedule_check(cx);
    }

    pub(super) fn toggle_role(&mut self, index: usize, own_worktree: bool, cx: &mut Context<Self>) {
        let Some(role) = self.roles.get_mut(index) else { return };
        if own_worktree {
            role.own_worktree = !role.own_worktree;
        } else {
            role.edits = !role.edits;
        }
        self.schedule_check(cx);
    }

    pub(super) fn add_role(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let taken: Vec<String> = self.roles.iter().map(|r| r.current_name(cx)).collect();
        let name = fresh_role_name(&taken);
        let role = RoleFields::new(&RoleDraft::new(&name, "claude", false), window, cx);
        self.roles.push(role);
        let stage = StageFields::new(&StageDraft::with_roles(&name), window, cx);
        self.stages.push(stage);
        self.schedule_check(cx);
    }

    pub(super) fn remove_role(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.roles.len() {
            return;
        }
        let gone = self.roles.remove(index).current_name(cx);
        self.rewrite_stages(|list| removed_from(list, &gone), cx);
        self.schedule_check(cx);
    }

    pub(super) fn add_stage(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let stage = StageFields::new(&StageDraft::default(), window, cx);
        self.stages.push(stage);
        self.schedule_check(cx);
    }

    pub(super) fn remove_stage(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.stages.len() {
            self.stages.remove(index);
            self.schedule_check(cx);
        }
    }
}
