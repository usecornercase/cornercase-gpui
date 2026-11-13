use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{App, AppContext as _, Context, Entity, Focusable as _, ScrollHandle, WeakEntity, Window};
use serde_json::Value;

use super::FlowsPanel;
use super::actions::{Imported, announce, note, run_stdin};
use super::diagram::{Board, Played};
use super::draft::{
    self, Back, BackDraft, Draft, Place, RoleDraft, Saved, Stage, fresh_role_name, gone_note, has_breaks, neighbour,
    parse_saved, place_of, save_args, seat, swapped,
};
use super::model::{self, FlowInfo, FlowSpec, RoleSpec};
use crate::ui::{TextField, TextFieldEvent};

mod about;
mod check;
mod loops;
mod plan;
mod team;

pub(super) use self::check::{Check, Pill, Verdict};
pub(super) use self::plan::{Layout, PLAN_PAD, layout};

const CHECK_DELAY: Duration = Duration::from_millis(400);

pub(super) struct RoleFields {
    pub id: u64,
    base: Value,
    pub name: Entity<TextField>,
    pub agent: Entity<TextField>,
    pub model: Entity<TextField>,
    pub effort: Entity<TextField>,
    pub args: Entity<TextField>,
    pub env: Entity<TextField>,
    pub prompt: Entity<TextField>,
    pub edits: bool,
    pub own_worktree: bool,
    pub command_open: bool,
}

impl RoleFields {
    fn new(id: u64, role: &RoleDraft, inputs: &Inputs, window: &mut Window, cx: &mut Context<FlowEditor>) -> Self {
        Self {
            id,
            base: role.base.clone(),
            name: inputs.line(&role.name, "Name", window, cx),
            agent: inputs.line(&role.agent, "claude, codex, gemini…", window, cx),
            model: inputs.line(&role.model, "Default", window, cx),
            effort: inputs.line(&role.effort, "Default", window, cx),
            args: inputs.wrapped(&role.args, "Added to the agent's command", window, cx),
            env: inputs.wrapped(&role.env, "KEY=value KEY='a value'", window, cx),
            prompt: inputs.lines(&role.prompt, "What this role does", window, cx),
            edits: role.edits,
            own_worktree: role.own_worktree,
            command_open: !role.args.trim().is_empty() || !role.env.trim().is_empty(),
        }
    }

    fn draft(&self, cx: &App) -> RoleDraft {
        RoleDraft {
            base: self.base.clone(),
            name: self.name.read(cx).text(),
            agent: self.agent.read(cx).text(),
            model: self.model.read(cx).text(),
            effort: self.effort.read(cx).text(),
            args: self.args.read(cx).text(),
            env: self.env.read(cx).text(),
            prompt: self.prompt.read(cx).text(),
            edits: self.edits,
            own_worktree: self.own_worktree,
        }
    }

    pub(super) fn spec(&self, cx: &App) -> RoleSpec {
        self.draft(cx).spec()
    }

    pub(super) fn current_name(&self, cx: &App) -> String {
        self.name.read(cx).text().trim().to_string()
    }
}

pub(super) type StageFields = Stage<Entity<TextField>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Open {
    Lead,
    Role(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum View {
    Form,
    Plan,
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
    pub inputs: Inputs,
    pub name: Entity<TextField>,
    pub description: Entity<TextField>,
    pub notes: Entity<TextField>,
    pub commits: bool,
    pub orchestrator: RoleFields,
    pub roles: Vec<RoleFields>,
    pub stages: Vec<StageFields>,
    pub plan: FlowSpec,
    pub board: Board,
    pub played: Played,
    pub check: Check,
    next_id: u64,
    pub open: Option<Open>,
    pub notes_open: bool,
    pub view: View,
    pub width: Rc<Cell<f32>>,
    pub opened: Option<String>,
    pub mode: Mode,
    pub error: Option<String>,
}

#[derive(Default)]
pub(super) struct Inputs {
    pub scroller: ScrollHandle,
}

impl Inputs {
    fn line(&self, text: &str, hint: &str, window: &mut Window, cx: &mut Context<FlowEditor>) -> Entity<TextField> {
        self.field((text, hint), |field| field, window, cx)
    }

    fn wrapped(&self, text: &str, hint: &str, window: &mut Window, cx: &mut Context<FlowEditor>) -> Entity<TextField> {
        self.field((text, hint), TextField::wraps, window, cx)
    }

    fn lines(&self, text: &str, hint: &str, window: &mut Window, cx: &mut Context<FlowEditor>) -> Entity<TextField> {
        self.field((text, hint), TextField::prompt, window, cx)
    }

    fn sentence(&self, text: &str, hint: &str, window: &mut Window, cx: &mut Context<FlowEditor>) -> Entity<TextField> {
        if has_breaks(text) { self.lines(text, hint, window, cx) } else { self.wrapped(text, hint, window, cx) }
    }

    fn field(
        &self,
        (text, hint): (&str, &str),
        shape: impl FnOnce(TextField) -> TextField,
        window: &mut Window,
        cx: &mut Context<FlowEditor>,
    ) -> Entity<TextField> {
        let text = text.to_string();
        let scroller = self.scroller.clone();
        let field = cx.new(|cx| {
            let mut field = shape(TextField::new(hint.to_string(), cx).in_tab_order().inside(scroller));
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
        if flow.is_none() {
            let focus = editor.read(cx).name.read(cx).focus_handle(cx);
            window.focus(&focus);
        }
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

fn seated(
    stages: Vec<Stage<String>>,
    inputs: &Inputs,
    window: &mut Window,
    cx: &mut Context<FlowEditor>,
) -> Vec<StageFields> {
    stages.into_iter().map(|s| s.with_when(|text| inputs.line(&text, "problems", window, cx))).collect()
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
        let inputs = Inputs::default();
        let seating = seat(&draft);
        let opened = seating.note();
        let roles: Vec<RoleFields> =
            (0u64..).zip(&draft.roles).map(|(id, role)| RoleFields::new(id, role, &inputs, window, cx)).collect();
        let stages = seated(seating.stages, &inputs, window, cx);
        let mut this = Self {
            panel,
            project,
            title,
            note,
            new: place == Place::Yours && draft.base.is_null(),
            place,
            original: draft.to_value(),
            base: draft.base.clone(),
            name: inputs.line(&draft.name, "Name", window, cx),
            description: inputs.sentence(&draft.description, "What the flow is for", window, cx),
            notes: inputs.lines(&draft.notes, "Notes for the orchestrator (optional)", window, cx),
            commits: draft.commits,
            orchestrator: RoleFields::new(0, &draft.orchestrator, &inputs, window, cx),
            next_id: roles.len() as u64,
            roles,
            stages,
            plan: draft.plan().unwrap_or_default(),
            board: Board::new(super::BOARD),
            played: Played::default(),
            inputs,
            check: Check::default(),
            open: None,
            notes_open: !draft.notes.is_empty(),
            view: View::Form,
            width: Rc::new(Cell::new(super::SHEET_AT_1280)),
            opened,
            mode: Mode::Editing,
            error: None,
        };
        this.schedule_check(cx);
        this
    }

    fn draft(&self, cx: &App) -> Draft {
        let names: Vec<(u64, String)> = self.roles.iter().map(|r| (r.id, r.current_name(cx))).collect();
        Draft {
            base: self.base.clone(),
            name: self.name.read(cx).text(),
            description: self.description.read(cx).text(),
            notes: self.notes.read(cx).text(),
            commits: self.commits,
            orchestrator: self.orchestrator.draft(cx),
            roles: self.roles.iter().map(|r| r.draft(cx)).collect(),
            stages: self.stages.iter().map(|s| s.draft(&names, s.when.read(cx).text())).collect(),
        }
    }

    pub(super) fn dirty(&self, cx: &App) -> bool {
        self.new || self.draft(cx).to_value() != self.original
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        if self.mode == Mode::Discarding {
            self.mode = Mode::Editing;
        }
        self.schedule_check(cx);
    }

    pub(super) fn schedule_check(&mut self, cx: &mut Context<Self>) {
        if let Some(plan) = self.draft(cx).plan() {
            self.plan = plan;
        }
        self.error = None;
        let generation = self.check.changed();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(CHECK_DELAY).await;
            let _ = this.update(cx, |this, cx| this.check(generation, cx));
        })
        .detach();
        cx.notify();
    }

    fn check(&mut self, generation: u64, cx: &mut Context<Self>) {
        if !self.check.is_current(generation) {
            return;
        }
        match self.draft(cx).to_value() {
            Ok(value) => {
                let command = save_args(&self.place, self.project, true);
                run_stdin(cx, command, value.to_string(), move |this, result, cx| this.checked(generation, result, cx));
            }
            Err(e) => self.check.answer(generation, Verdict::Invalid(e)),
        }
        cx.notify();
    }

    fn checked(&mut self, generation: u64, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        let verdict = match result.and_then(|out| parse_saved(&out)) {
            Ok(saved) => Verdict::Checked(saved),
            Err(e) => Verdict::Failed(e.to_string()),
        };
        self.check.answer(generation, verdict);
        cx.notify();
    }

    pub(super) fn save(&mut self, cx: &mut Context<Self>) {
        if self.mode == Mode::Saving || self.check.rests() {
            return;
        }
        let value = match self.draft(cx).to_value() {
            Ok(value) => value,
            Err(e) => {
                self.check.refuse(e);
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

    pub(super) fn set_view(&mut self, view: View, cx: &mut Context<Self>) {
        self.view = view;
        cx.notify();
    }

    fn focus_on(field: &Entity<TextField>, window: &mut Window, cx: &mut Context<Self>) {
        let handle = field.read(cx).focus_handle(cx);
        window.focus(&handle);
        let again = field.clone();
        window.on_next_frame(move |_, cx| again.update(cx, TextField::reveal));
    }

    pub(super) fn open_role(&mut self, open: Open, window: &mut Window, cx: &mut Context<Self>) {
        self.open = Some(open);
        let field = match open {
            Open::Lead => Some(self.orchestrator.agent.clone()),
            Open::Role(id) => self.roles.iter().find(|role| role.id == id).map(|role| role.name.clone()),
        };
        if let Some(field) = field {
            Self::focus_on(&field, window, cx);
        }
        cx.notify();
    }

    pub(super) fn close_role(&mut self, cx: &mut Context<Self>) {
        self.open = None;
        cx.notify();
    }

    pub(super) fn toggle_notes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.notes_open = true;
        let field = self.notes.clone();
        Self::focus_on(&field, window, cx);
        cx.notify();
    }

    pub(super) fn toggle_command(&mut self, open: Open, cx: &mut Context<Self>) {
        let role = match open {
            Open::Lead => Some(&mut self.orchestrator),
            Open::Role(id) => self.roles.iter_mut().find(|role| role.id == id),
        };
        if let Some(role) = role {
            role.command_open = !role.command_open;
        }
        cx.notify();
    }

    pub(super) fn toggle_role(&mut self, id: u64, own_worktree: bool, cx: &mut Context<Self>) {
        let Some(role) = self.roles.iter_mut().find(|role| role.id == id) else { return };
        if own_worktree {
            role.own_worktree = !role.own_worktree;
        } else {
            role.edits = !role.edits;
        }
        self.schedule_check(cx);
    }

    fn add_role_field(&mut self, window: &mut Window, cx: &mut Context<Self>) -> u64 {
        let taken: Vec<String> = self.roles.iter().map(|r| r.current_name(cx)).collect();
        let name = fresh_role_name(&taken);
        let id = self.next_id;
        self.next_id += 1;
        let role = RoleFields::new(id, &RoleDraft::new(&name, "claude", false), &self.inputs, window, cx);
        self.roles.push(role);
        id
    }

    pub(super) fn add_role_beside(&mut self, stage: usize, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.add_role_field(window, cx);
        if let Some(stage) = self.stages.get_mut(stage) {
            stage.roles.push(id);
        }
        self.schedule_check(cx);
        self.open_role(Open::Role(id), window, cx);
    }

    pub(super) fn add_stage(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.add_role_field(window, cx);
        let when = self.inputs.line("", "problems", window, cx);
        self.stages.push(Stage::of(id, when));
        self.schedule_check(cx);
        self.open_role(Open::Role(id), window, cx);
    }

    pub(super) fn remove_role(&mut self, id: u64, cx: &mut Context<Self>) {
        self.roles.retain(|role| role.id != id);
        let lost = draft::remove_role(&mut self.stages, id);
        note(gone_note(&lost).as_deref(), cx);
        if self.open == Some(Open::Role(id)) {
            self.open = None;
        }
        self.schedule_check(cx);
    }

    pub(super) fn move_role(&mut self, id: u64, to: usize, cx: &mut Context<Self>) {
        if draft::stage_of(&self.stages, id) == Some(to) {
            return;
        }
        let lost = draft::move_role(&mut self.stages, id, to);
        note(gone_note(&lost).as_deref(), cx);
        self.schedule_check(cx);
    }

    pub(super) fn in_new_stage(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if draft::alone(&self.stages, id) {
            return;
        }
        let when = self.inputs.line("", "problems", window, cx);
        self.stages.push(Stage { base: Value::Null, roles: Vec::new(), back: None, when });
        self.move_role(id, self.stages.len() - 1, cx);
    }

    pub(super) fn move_stage(&mut self, index: usize, up: bool, cx: &mut Context<Self>) {
        let Some(other) = neighbour(index, up, self.stages.len()) else { return };
        self.stages.swap(index, other);
        let lost = draft::follow_loops(&mut self.stages, swapped(index + 1, other + 1));
        note(gone_note(&lost).as_deref(), cx);
        self.schedule_check(cx);
    }

    pub(super) fn add_loop(&mut self, stage: usize, cx: &mut Context<Self>) {
        let Some(fields) = self.stages.get_mut(stage) else { return };
        let BackDraft { to, when, at_most } = BackDraft::default();
        fields.back = Some(Back { to, at_most });
        fields.when.update(cx, |field, cx| field.set_text(when, cx));
        self.schedule_check(cx);
    }

    pub(super) fn remove_loop(&mut self, stage: usize, cx: &mut Context<Self>) {
        if let Some(fields) = self.stages.get_mut(stage) {
            fields.back = None;
        }
        self.schedule_check(cx);
    }

    pub(super) fn set_loop_to(&mut self, stage: usize, to: usize, cx: &mut Context<Self>) {
        let Some(back) = self.stages.get_mut(stage).and_then(|fields| fields.back.as_mut()) else { return };
        if back.to == to {
            return;
        }
        back.to = to;
        self.schedule_check(cx);
    }

    pub(super) fn step_loop(&mut self, stage: usize, up: bool, cx: &mut Context<Self>) {
        if let Some(back) = self.stages.get_mut(stage).and_then(|fields| fields.back.as_mut()) {
            *back = back.step(up);
        }
        self.schedule_check(cx);
    }
}
