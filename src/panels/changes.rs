//! The workspace's git changes as the changes panel shows them (`cornercase changes`): the files
//! of the workspace our view shows, followed live (`changes list --follow`), in one of the TUI's
//! modes (uncommitted, the branch's commits, everything since a base) or every project's
//! uncommitted changes; a file's patch as a coloured diff below the list, with the actions of the
//! TUI's panel: viewed marks, staging, discarding, and per hunk open, ask agent and copy.

mod diff;
mod wire;

use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    AnyElement, AnyView, App, AppContext as _, ClickEvent, ClipboardItem, Context, FontStyle, FontWeight,
    HighlightStyle, Hsla, InteractiveElement as _, IntoElement, ListAlignment, ListOffset, ListState,
    ParentElement as _, Render, Rgba, SharedString, StatefulInteractiveElement as _, Styled as _, StyledText,
    WeakEntity, Window, div, list, prelude::FluentBuilder as _, px, relative,
};

use crate::theme::FONT_FAMILY;
use crate::cli::Stream;
use crate::dialogs;
use crate::model::{Colours, Session};
use crate::panels::PanelKind;
use crate::store::{self, Store, ToastKind, args};
use crate::theme::{Painter, colours};
use crate::ui;

use diff::Row;
use wire::{Base, FileInfo, FileStatus, FoldClass, Gap, LineKind, Mode, Patch, Segment, Staging, State, Summary};

/// What the panel shows: one workspace in a mode, or every project's uncommitted changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    Project(Mode),
    Every,
}

impl Scope {
    fn mode(self) -> Mode {
        match self {
            Self::Project(mode) => mode,
            Self::Every => Mode::Uncommitted,
        }
    }
}

/// What the running stream follows: the scope, and the workspace in project scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Follow {
    scope: Scope,
    workspace: Option<u64>,
}

/// The file whose patch is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Selected {
    workspace: u64,
    path: String,
    mode: Mode,
}

/// A style of the patch's table, resolved to what we draw with.
#[derive(Debug, Clone, Copy)]
struct Look {
    fg: Hsla,
    bold: bool,
    italic: bool,
}

/// A patch ready to draw: what `changes file` and `changes gap` answered, and the rows.
struct DiffData {
    selected: Selected,
    patch: Patch,
    looks: Vec<Look>,
    gaps: HashMap<usize, (Gap, Vec<Look>)>,
    rows: Vec<Row>,
}

/// The selected file's patch: the last one read, and whether a newer one is being read.
#[derive(Default)]
struct Open {
    /// The digest of the version asked for, so a change in the summary reads it again.
    digest: String,
    data: Option<Rc<DiffData>>,
    loading: bool,
    error: Option<String>,
    /// Bumped on each read, so a late answer for an older version is dropped.
    generation: u64,
}

pub(crate) struct ChangesPanel {
    scope: Scope,
    /// The mode project scope comes back to from Every project.
    project_mode: Mode,
    /// The workspace our view shows.
    shown: Option<u64>,
    follow: Option<Follow>,
    stream: Option<Stream>,
    stream_generation: u64,
    summary: Option<Summary>,
    /// Why the stream stopped, when it did.
    ended: Option<String>,
    selected: Option<Selected>,
    open: Open,
    base: Option<Base>,
    base_open: bool,
    diff_list: ListState,
}

impl ChangesPanel {
    pub(crate) fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::get(cx);
        cx.observe(&store, |this: &mut Self, _, cx| this.sync(cx)).detach();
        let mut panel = Self {
            scope: Scope::Project(Mode::Uncommitted),
            project_mode: Mode::Uncommitted,
            shown: None,
            follow: None,
            stream: None,
            stream_generation: 0,
            summary: None,
            ended: None,
            selected: None,
            open: Open::default(),
            base: None,
            base_open: false,
            diff_list: ListState::new(0, ListAlignment::Top, px(400.)),
        };
        panel.sync(cx);
        panel
    }

    // -----------------------------------------------------------------------------------------
    // Following the changes.

    /// Follows what the store says: the shown workspace, and whether the panel is docked at all
    /// (a closed panel follows nothing).
    fn sync(&mut self, cx: &mut Context<Self>) {
        let store = Store::get(cx);
        let s = store.read(cx);
        let docked = s.dock == Some(PanelKind::Changes);
        let shown = s.session.as_ref().and_then(|s| s.shown().workspace);
        if shown != self.shown {
            self.shown = shown;
            self.base = None;
            self.base_open = false;
            if matches!(self.scope, Scope::Project(_)) {
                self.select(None, cx);
            }
        }
        let want = docked
            .then(|| match self.scope {
                Scope::Project(_) => shown.map(|w| Follow { scope: self.scope, workspace: Some(w) }),
                Scope::Every => Some(Follow { scope: Scope::Every, workspace: None }),
            })
            .flatten();
        if want != self.follow {
            self.restart(want, cx);
        }
    }

    fn restart(&mut self, follow: Option<Follow>, cx: &mut Context<Self>) {
        self.follow = follow;
        self.stream = None;
        self.summary = None;
        self.ended = None;
        self.stream_generation += 1;
        let generation = self.stream_generation;
        let Some(follow) = follow else {
            cx.notify();
            return;
        };
        let mut a = args(&["changes", "list", "--follow"]);
        match follow.workspace {
            Some(w) => {
                a.extend(["--workspace", &w.to_string(), "--mode", follow.scope.mode().name()].map(String::from))
            }
            // Every project is only ever uncommitted, and takes no mode.
            None => a.push("--every".into()),
        }
        self.stream = store::follow_until(
            cx,
            a,
            move |this, line, cx| {
                if this.stream_generation == generation {
                    this.on_summary(&line, cx);
                }
            },
            move |this, err, cx| {
                if this.stream_generation == generation {
                    this.stream = None;
                    let err = err.trim();
                    this.ended = Some(if err.is_empty() { "the changes stopped".to_string() } else { err.to_string() });
                    cx.notify();
                }
            },
        );
        cx.notify();
    }

    fn on_summary(&mut self, line: &str, cx: &mut Context<Self>) {
        let Ok(summary) = wire::summary(line) else { return };
        self.summary = Some(summary);
        self.refresh_open(cx);
        cx.notify();
    }

    /// Reads the selected file again when its change changed, and lets it go when it is gone.
    fn refresh_open(&mut self, cx: &mut Context<Self>) {
        let Some(selected) = self.selected.clone() else { return };
        let Some(summary) = &self.summary else { return };
        if summary.state != State::Ready {
            return;
        }
        match find(summary, selected.workspace, &selected.path) {
            Some(file) if file.digest != self.open.digest => {
                let digest = file.digest.clone();
                self.load_patch(digest, cx);
            }
            Some(_) => {}
            None => self.select(None, cx),
        }
    }

    // -----------------------------------------------------------------------------------------
    // Choosing what to show.

    fn set_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        if scope == self.scope {
            return;
        }
        if let Scope::Project(mode) = scope {
            self.project_mode = mode;
        }
        self.scope = scope;
        self.base_open = false;
        self.select(None, cx);
        self.sync(cx);
    }

    fn select(&mut self, selected: Option<Selected>, cx: &mut Context<Self>) {
        if selected == self.selected {
            return;
        }
        self.selected = selected;
        self.open = Open { generation: self.open.generation + 1, ..Open::default() };
        self.diff_list.reset(0);
        if let Some(selected) = &self.selected {
            let digest = self
                .summary
                .as_ref()
                .and_then(|s| find(s, selected.workspace, &selected.path))
                .map(|f| f.digest.clone())
                .unwrap_or_default();
            self.load_patch(digest, cx);
        }
        cx.notify();
    }

    fn load_patch(&mut self, digest: String, cx: &mut Context<Self>) {
        let Some(selected) = self.selected.clone() else { return };
        self.open.digest = digest;
        self.open.loading = true;
        self.open.generation += 1;
        let generation = self.open.generation;
        let a = args(&[
            "changes",
            "file",
            &selected.path,
            "--workspace",
            &selected.workspace.to_string(),
            "--mode",
            selected.mode.name(),
            "--json",
        ]);
        store::run(cx, a, move |this: &mut Self, result, cx| {
            if this.open.generation != generation {
                return;
            }
            this.open.loading = false;
            match result.and_then(|out| serde_json::from_str::<Patch>(&out).map_err(anyhow::Error::from)) {
                Ok(patch) => {
                    // Keep the gaps that were read while the patch stays the same version.
                    let same = this.open.data.as_ref().is_some_and(|d| d.selected == selected);
                    let gaps = match &this.open.data {
                        Some(d) if same && d.patch.hunks == patch.hunks => d.gaps.clone(),
                        _ => HashMap::new(),
                    };
                    this.open.error = None;
                    this.set_data(selected, patch, gaps, same);
                }
                Err(e) => this.open.error = Some(e.to_string()),
            }
            cx.notify();
        });
        cx.notify();
    }

    fn set_data(
        &mut self,
        selected: Selected,
        patch: Patch,
        gaps: HashMap<usize, (Gap, Vec<Look>)>,
        keep_scroll: bool,
    ) {
        let looks = looks(&patch.styles);
        let gap_lines: HashMap<usize, Gap> = gaps.iter().map(|(k, (g, _))| (*k, g.clone())).collect();
        let rows = diff::rows(&patch.hunks, &gap_lines);
        let top = self.diff_list.logical_scroll_top();
        self.diff_list.reset(rows.len());
        if keep_scroll && top.item_ix < rows.len() {
            self.diff_list.scroll_to(top);
        } else {
            self.diff_list.scroll_to(ListOffset { item_ix: 0, offset_in_item: px(0.) });
        }
        self.open.data = Some(Rc::new(DiffData { selected, patch, looks, gaps, rows }));
    }

    /// Reads the unchanged lines before hunk `hunk` (from 0, never the first) and shows them in place.
    fn load_gap(&mut self, hunk: usize, cx: &mut Context<Self>) {
        let Some(data) = self.open.data.clone() else { return };
        let s = &data.selected;
        let a = args(&[
            "changes",
            "gap",
            &s.path,
            // The index from 0 of the hunk after the gap, whatever the help says
            // (usecornercase/cornercase-terminal#471).
            "--hunk",
            &hunk.to_string(),
            "--workspace",
            &s.workspace.to_string(),
            "--mode",
            s.mode.name(),
            "--json",
        ]);
        let generation = self.open.generation;
        store::run(cx, a, move |this: &mut Self, result, cx| {
            if this.open.generation != generation {
                return;
            }
            match result.and_then(|out| serde_json::from_str::<Gap>(&out).map_err(anyhow::Error::from)) {
                Ok(gap) => {
                    if let Some(data) = this.open.data.clone() {
                        let mut gaps = data.gaps.clone();
                        let gap_looks = looks(&gap.styles);
                        gaps.insert(hunk, (gap, gap_looks));
                        this.set_data(data.selected.clone(), data.patch.clone(), gaps, true);
                    }
                }
                Err(e) => toast(e.to_string(), ToastKind::Error, cx),
            }
            cx.notify();
        });
    }

    fn open_base(&mut self, cx: &mut Context<Self>) {
        self.base_open = !self.base_open;
        let Some(workspace) = self.follow.and_then(|f| f.workspace) else { return };
        if self.base_open {
            let a = args(&["changes", "base", "--workspace", &workspace.to_string(), "--json"]);
            store::run(cx, a, |this: &mut Self, result, cx| {
                match result.and_then(|out| serde_json::from_str::<Base>(&out).map_err(anyhow::Error::from)) {
                    Ok(base) => this.base = Some(base),
                    Err(e) => {
                        this.base_open = false;
                        toast(e.to_string(), ToastKind::Error, cx);
                    }
                }
                cx.notify();
            });
        }
        cx.notify();
    }

    /// `changes base BRANCH` (or `--default`): shared with the window, so the stream answers it.
    fn choose_base(&mut self, branch: Option<String>, cx: &mut Context<Self>) {
        self.base_open = false;
        let Some(workspace) = self.follow.and_then(|f| f.workspace) else { return };
        let mut a = args(&["changes", "base", "--workspace", &workspace.to_string()]);
        match branch {
            Some(branch) => a.push(branch),
            None => a.push("--default".into()),
        }
        if let Some(base) = &mut self.base {
            base.base = None;
        }
        Store::get(cx).read(cx).cmd(a);
        cx.notify();
    }

    // -----------------------------------------------------------------------------------------
    // Acting on a file.

    fn toggle_viewed(&mut self, workspace: u64, path: &str, cx: &mut Context<Self>) {
        let mode = self.scope.mode();
        let Some(file) = self.summary.as_mut().and_then(|s| find_mut(s, workspace, path)) else { return };
        let mut a = args(&["changes", "viewed", path, "--workspace", &workspace.to_string(), "--mode", mode.name()]);
        if file.viewed {
            a.push("--clear".into());
        }
        // Shown at once; the next summary confirms it.
        file.viewed = !file.viewed;
        Store::get(cx).read(cx).cmd(a);
        cx.notify();
    }

    /// Stages while anything is unstaged and unstages once everything is, as the TUI's `+`.
    fn toggle_staged(workspace: u64, file: &FileInfo, cx: &mut App) {
        let verb = if file.staged == Some(Staging::Yes) { "unstage" } else { "stage" };
        Store::get(cx).read(cx).cmd(args(&["changes", verb, &file.path, "--workspace", &workspace.to_string()]));
    }

    fn discard(workspace: u64, file: &FileInfo, window: &mut Window, cx: &mut App) {
        let deletes = matches!(file.status, FileStatus::Untracked | FileStatus::Added);
        let (title, button) =
            if deletes { ("Delete the file?", "Delete") } else { ("Discard the changes?", "Discard") };
        dialogs::confirm(
            dialogs::Question::new(
                title,
                format!(
                    "{}\n\nIt discards the file as it is now, even if it changed since this diff was read \
                     (the command line cannot name the version seen: usecornercase/cornercase-terminal#470).",
                    diff::discard_question(file)
                ),
                button,
                args(&["changes", "discard", &file.path, "--workspace", &workspace.to_string(), "--yes"]),
            ),
            window,
            cx,
        );
    }

    /// Opens the file in the user's editor in a new tab of its workspace, as the TUI's ` open `.
    fn open_in_editor(workspace: u64, path: &str, line: u32, cx: &mut App) {
        let store = Store::get(cx);
        let s = store.read(cx);
        let mut a = args(&["new-tab", "--workspace", &workspace.to_string(), "--focus"]);
        // `--view` goes before `--`: after it, it would be typed into the shell.
        if let Some(view) = s.view_id {
            a.extend(["--view".to_string(), view.to_string()]);
        }
        a.push("--".into());
        a.push(diff::editor_command(path, line));
        s.cmd(a);
    }

    /// Pastes `path:a-b ` into the workspace's agent and shows it, as the TUI's ` ask agent `;
    /// without an agent it copies the reference.
    fn ask_agent(workspace: u64, reference: String, cx: &mut App) {
        let store = Store::get(cx);
        let pane = store.read(cx).session.as_ref().and_then(|s| agent_pane(s, workspace));
        if let Some((pane, agent)) = pane {
            store.update(cx, |s, cx| {
                s.cmd(args(&["send", "--pane", &pane.to_string(), &reference]));
                s.focus("pane", pane);
                s.toast(format!("Sent {} to {agent}", reference.trim_end()), ToastKind::Info, cx);
            })
        } else {
            cx.write_to_clipboard(ClipboardItem::new_string(reference.clone()));
            store.update(cx, |s, cx| {
                s.toast(format!("No agent in this workspace: copied {}", reference.trim_end()), ToastKind::Info, cx);
            });
        }
    }

    // -----------------------------------------------------------------------------------------
    // Drawing.

    fn render_header(&self, session: Option<&Session>, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let tab = |id: &'static str, label: &'static str, scope: Scope, current: Scope| {
            ui::segment(id, label, scope == current)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_scope(scope, cx)))
                .into_any_element()
        };
        let tabs = ui::segments(vec![
            tab("mode-uncommitted", "Uncommitted", Scope::Project(Mode::Uncommitted), self.scope),
            tab("mode-commits", "Commits", Scope::Project(Mode::Commits), self.scope),
            tab("mode-all", "All", Scope::Project(Mode::All), self.scope),
            tab("mode-every", "Every project", Scope::Every, self.scope),
        ]);
        let place = match (self.scope, self.shown.and_then(|w| session?.workspace(w))) {
            (Scope::Every, _) => "uncommitted changes of every project".to_string(),
            (_, Some((p, w))) if p.workspaces.len() > 1 => format!("{} › {}", p.name, w.name),
            (_, Some((p, _))) => p.name.clone(),
            (_, None) => String::new(),
        };
        let totals = self.summary.as_ref().filter(|s| s.state == State::Ready).map(|s| {
            let files: usize = s.workspaces.iter().map(|w| w.files.len()).sum();
            let added: usize = s.workspaces.iter().map(|w| w.added).sum();
            let removed: usize = s.workspaces.iter().map(|w| w.removed).sum();
            (files, added, removed)
        });
        let base_label = match self.scope {
            Scope::Project(Mode::Commits | Mode::All) => {
                let base = self.summary.as_ref().and_then(|s| s.workspaces.first()).and_then(|w| w.base.clone());
                Some(format!("vs {} ▾", base.unwrap_or_else(|| "…".into())))
            }
            _ => None,
        };
        div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(8.))
            .px(px(10.))
            .pt(px(10.))
            .pb(px(8.))
            .border_b_1()
            .border_color(colours::border())
            .child(tabs)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(11.))
                    .child(div().flex_1().min_w(px(0.)).truncate().text_color(colours::muted()).child(place))
                    .when_some(totals, |d, (files, added, removed)| {
                        d.child(
                            div()
                                .flex()
                                .flex_none()
                                .gap(px(6.))
                                .child(div().text_color(colours::muted()).child(plural(files, "file")))
                                .child(div().text_color(colours::done()).child(format!("+{added}")))
                                .child(div().text_color(colours::failed()).child(format!("−{removed}"))),
                        )
                    })
                    .when_some(base_label, |d, label| {
                        d.child(
                            ui::button("base", label)
                                .py(px(1.))
                                .text_size(px(11.))
                                .when(self.base_open, |b| b.border_color(colours::accent()))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_base(cx))),
                        )
                    }),
            )
            .when(self.base_open, |d| d.child(self.render_bases(cx)))
    }

    /// The branches to compare with, under the header while the base button is open.
    fn render_bases(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let Some(base) = &self.base else {
            return div().child(ui::dim("reading the branches…"));
        };
        let default_label = match &base.default {
            Some(d) => format!("The default branch ({d})"),
            None => "The default branch".to_string(),
        };
        let mut col = div()
            .id("bases")
            .max_h(px(220.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .p(px(4.))
            .rounded(px(6.))
            .bg(colours::panel())
            .border_1()
            .border_color(colours::border())
            .child(
                ui::row("base-default", false)
                    .py(px(3.))
                    .child(chosen_dot(base.base.is_none()))
                    .child(div().flex_1().child(default_label))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.choose_base(None, cx))),
            );
        for (i, branch) in base.branches.iter().enumerate() {
            let chosen = base.base.as_deref() == Some(branch.as_str());
            let name = branch.clone();
            col = col.child(
                ui::row(SharedString::from(format!("base-{i}")), false)
                    .py(px(3.))
                    .child(chosen_dot(chosen))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .font_family(FONT_FAMILY)
                            .text_size(px(12.))
                            .child(branch.clone()),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.choose_base(Some(name.clone()), cx))),
            );
        }
        div().child(col)
    }

    /// The list of files, or what stands in for it while there is none.
    fn render_files(&self, session: Option<&Session>, cx: &mut Context<Self>) -> AnyElement {
        if let Some(note) = self.state_note(session) {
            return note;
        }
        let Some(summary) = &self.summary else { return ui::note("").into_any_element() };
        let mode = self.scope.mode();
        let every = self.scope == Scope::Every;
        let mut col = div().flex().flex_col().gap(px(1.)).p(px(6.));
        for ws in &summary.workspaces {
            if every {
                col = col.child(group_header(session, ws.project, ws.workspace, ws.files.len(), ws.added, ws.removed));
            }
            for file in &ws.files {
                col = col.child(self.render_file(ws.workspace, file, mode, cx));
            }
        }
        div().id("changes-files").size_full().overflow_y_scroll().child(col).into_any_element()
    }

    /// Loading, failed, not a git workspace or nothing changed; `None` when there are files.
    fn state_note(&self, session: Option<&Session>) -> Option<AnyElement> {
        let centred = |title: String, detail: Option<String>, colour: Rgba| {
            div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .p(px(24.))
                .child(div().text_color(colour).child(title))
                .when_some(detail, |d, text| {
                    d.child(
                        div().max_w(px(320.)).text_center().text_size(px(11.)).text_color(colours::faint()).child(text),
                    )
                })
                .into_any_element()
        };
        if session.is_none() {
            return Some(centred("Connecting…".into(), None, colours::muted()));
        }
        if self.follow.is_none() {
            return Some(centred(
                "No workspace shown".into(),
                Some("Open a project to see its changes.".into()),
                colours::muted(),
            ));
        }
        if let Some(err) = &self.ended {
            return Some(centred("The changes are not available".into(), Some(err.clone()), colours::failed()));
        }
        let Some(summary) = &self.summary else {
            return Some(centred("Reading changes…".into(), None, colours::muted()));
        };
        match summary.state {
            State::Loading if summary.workspaces.iter().all(|w| w.files.is_empty()) => {
                Some(centred("Reading changes…".into(), None, colours::muted()))
            }
            State::Failed => Some(centred(
                "Could not read the changes".into(),
                Some(summary.error.clone().unwrap_or_else(|| "git failed".into())),
                colours::failed(),
            )),
            _ if summary.workspaces.iter().all(|w| w.files.is_empty()) => {
                let (title, detail) = match self.scope {
                    Scope::Every => ("No changes in any project", None),
                    Scope::Project(Mode::Uncommitted) => {
                        ("No uncommitted changes", Some("The working tree matches HEAD."))
                    }
                    Scope::Project(Mode::Commits) => {
                        ("No commits on this branch", Some("Nothing was committed since the base."))
                    }
                    Scope::Project(Mode::All) => ("No changes since the base", None),
                };
                Some(centred(format!("✓ {title}"), detail.map(Into::into), colours::muted()))
            }
            _ => None,
        }
    }

    fn render_file(
        &self,
        workspace: u64,
        file: &FileInfo,
        mode: Mode,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let selected = self.selected.as_ref().is_some_and(|s| s.workspace == workspace && s.path == file.path);
        let id = SharedString::from(format!("file-{workspace}-{}", file.path));
        let (dir, name) = split_path(&file.path);
        let muted = file.viewed || file.fold != FoldClass::Open;
        let tag = match file.fold {
            FoldClass::Lockfile => Some("lockfile"),
            FoldClass::Binary => Some("binary"),
            FoldClass::Large => Some("too large"),
            _ => None,
        };
        let path = file.path.clone();
        let viewed_path = file.path.clone();
        let stage_file = file.clone();
        let group = SharedString::from(format!("row-{workspace}-{}", file.path));
        ui::row(id, selected)
            .group(group.clone())
            .gap(px(6.))
            .py(px(3.))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                let pick = Selected { workspace, path: path.clone(), mode };
                let same = this.selected.as_ref() == Some(&pick);
                this.select((!same).then_some(pick), cx);
            }))
            .child(
                div()
                    .w(px(12.))
                    .flex_none()
                    .font_family(FONT_FAMILY)
                    .text_size(px(11.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(status_colour(file.status))
                    .child(file.status.letter()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .items_baseline()
                    .gap(px(5.))
                    .overflow_hidden()
                    .child(
                        div()
                            .flex_none()
                            .max_w(relative(0.7))
                            .truncate()
                            .text_color(if muted { colours::muted() } else { colours::text() })
                            .child(name),
                    )
                    .child(
                        div().flex_1().min_w(px(0.)).truncate().text_size(px(11.)).text_color(colours::faint()).child(
                            match &file.old_path {
                                Some(old) => format!("{old} →"),
                                None => dir,
                            },
                        ),
                    ),
            )
            .when_some(tag, |d, tag| d.child(tag_badge(tag)))
            .child(counts(file.added, file.removed))
            .when(mode.uncommitted(), |d| {
                d.child(stage_button(&group, file.staged).on_click(move |_: &ClickEvent, _, cx| {
                    cx.stop_propagation();
                    Self::toggle_staged(workspace, &stage_file, cx);
                }))
            })
            .child(viewed_box(workspace, &file.path, file.viewed).on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    cx.stop_propagation();
                    this.toggle_viewed(workspace, &viewed_path, cx);
                },
            )))
    }

    /// The selected file's patch: its header with the file's actions, then the diff.
    fn render_open(&self, selected: &Selected, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let file = self
            .summary
            .as_ref()
            .and_then(|s| find(s, selected.workspace, &selected.path))
            .cloned()
            .or_else(|| self.open.data.as_ref().map(|d| d.patch.file.clone()))
            .unwrap_or_else(|| FileInfo { path: selected.path.clone(), ..FileInfo::default() });
        let workspace = selected.workspace;
        let first_line = self.open.data.as_ref().and_then(|d| d.patch.hunks.first()).map_or(1, |h| diff::changed(h).0);
        let (viewed_path, open_path) = (file.path.clone(), file.path.clone());
        let (stage_file, discard_file) = (file.clone(), file.clone());
        let can_open = file.status != FileStatus::Deleted;
        let actions =
            div()
                .flex()
                .flex_wrap()
                .gap(px(6.))
                .child(
                    ui::button("open-viewed", if file.viewed { "✓ Viewed" } else { "Mark viewed" })
                        .when(file.viewed, |b| b.text_color(colours::done()))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.toggle_viewed(workspace, &viewed_path, cx)
                        })),
                )
                .when(selected.mode.uncommitted(), |d| {
                    let staged = file.staged == Some(Staging::Yes);
                    d.child(
                        ui::button("open-stage", if staged { "− Unstage" } else { "+ Stage" })
                            .on_click(move |_: &ClickEvent, _, cx| Self::toggle_staged(workspace, &stage_file, cx)),
                    )
                    .child(
                        ui::button_coloured("open-discard", "↺ Discard", colours::failed()).on_click(
                            move |_: &ClickEvent, window, cx| Self::discard(workspace, &discard_file, window, cx),
                        ),
                    )
                })
                .when(can_open, |d| {
                    d.child(ui::button("open-editor", "Open in editor").on_click(move |_: &ClickEvent, _, cx| {
                        Self::open_in_editor(workspace, &open_path, first_line, cx)
                    }))
                });
        let (dir, name) = split_path(&file.path);
        let header = div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(6.))
            .px(px(10.))
            .py(px(8.))
            .bg(colours::panel())
            .border_y_1()
            .border_color(colours::border())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex_none()
                            .font_family(FONT_FAMILY)
                            .text_size(px(11.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(status_colour(file.status))
                            .child(file.status.letter()),
                    )
                    .child(
                        div().flex_none().max_w(relative(0.6)).truncate().font_weight(FontWeight::SEMIBOLD).child(name),
                    )
                    .child(
                        div().flex_1().min_w(px(0.)).truncate().text_size(px(11.)).text_color(colours::faint()).child(
                            match &file.old_path {
                                Some(old) => format!("from {old}"),
                                None => dir,
                            },
                        ),
                    )
                    .when(self.open.loading && self.open.data.is_some(), |d| d.child(ui::dim("updating…")))
                    .child(counts(file.added, file.removed))
                    .child(
                        ui::icon_button("open-close", "✕")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.select(None, cx))),
                    ),
            )
            .child(actions);
        div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .child(header)
            .child(div().flex_1().min_h(px(0.)).bg(colours::terminal()).child(self.render_diff(&file, cx)))
    }

    fn render_diff(&self, file: &FileInfo, cx: &mut Context<Self>) -> AnyElement {
        let Some(data) = self.open.data.clone() else {
            return match &self.open.error {
                Some(err) => ui::note(err.clone()).text_color(colours::failed()).into_any_element(),
                None => ui::note("Reading the diff…").into_any_element(),
            };
        };
        if data.rows.is_empty() {
            let text = match file.fold {
                FoldClass::Binary => "Binary file: no lines to show.",
                FoldClass::Large => "Too large to show: only its counts are listed.",
                FoldClass::Lockfile => "A lockfile with no lines to show.",
                _ if file.status == FileStatus::Renamed => "Renamed without changes.",
                _ => "No lines to show.",
            };
            return ui::note(text).into_any_element();
        }
        let panel = cx.entity().downgrade();
        list(self.diff_list.clone(), move |ix, _window, _cx| diff_row(&data, ix, &panel)).size_full().into_any_element()
    }

    fn render_footer(&self) -> impl IntoElement + use<> {
        let (viewed, files) = self.summary.as_ref().map_or((0, 0), |s| {
            let files = s.workspaces.iter().flat_map(|w| &w.files);
            (files.clone().filter(|f| f.viewed).count(), files.count())
        });
        let (dot, state) = match (&self.ended, &self.summary, self.stream.is_some()) {
            (Some(_), _, _) => (colours::failed(), "stopped"),
            (_, Some(s), true) if s.state == State::Ready => (colours::done(), "live"),
            (_, Some(s), _) if s.state == State::Failed => (colours::failed(), "failed"),
            (_, _, true) => (colours::working(), "reading"),
            _ => (colours::faint(), "idle"),
        };
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .h(px(26.))
            .px(px(10.))
            .border_t_1()
            .border_color(colours::border())
            .text_size(px(11.))
            .text_color(colours::faint())
            .child(if files > 0 { format!("{viewed} of {files} viewed") } else { String::new() })
            .child(
                div().flex().items_center().gap(px(5.)).child(div().size(px(6.)).rounded_full().bg(dot)).child(state),
            )
    }
}

impl Render for ChangesPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = Store::get(cx);
        let session = store.read(cx).session.clone();
        let session = session.as_ref();
        let selected = self.selected.clone();
        let files = self.render_files(session, cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .text_size(px(12.))
            .child(self.render_header(session, cx))
            .child(match selected {
                Some(_) => div().flex_none().max_h(relative(0.36)).min_h(px(60.)).flex().flex_col().child(files),
                None => div().flex_1().min_h(px(0.)).flex().flex_col().child(files),
            })
            .when_some(selected, |d, selected| d.child(self.render_open(&selected, cx)))
            .child(self.render_footer())
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| ChangesPanel::new(window, cx)).into()
}

// ---------------------------------------------------------------------------------------------
// The diff's rows.

fn diff_row(data: &DiffData, ix: usize, panel: &WeakEntity<ChangesPanel>) -> AnyElement {
    let Some(row) = data.rows.get(ix) else { return div().into_any_element() };
    match *row {
        Row::Gap { hunk, lines } => {
            let panel = panel.clone();
            div()
                .id(("gap", hunk))
                .w_full()
                .flex()
                .justify_center()
                .py(px(3.))
                .bg(colours::sidebar())
                .text_size(px(11.))
                .text_color(colours::faint())
                .cursor_pointer()
                .hover(|s| s.text_color(colours::accent()).bg(colours::hover()))
                .child(format!("⋯ {} unchanged", plural(lines as usize, "line")))
                .on_click(move |_: &ClickEvent, _, cx| {
                    let _ = panel.update(cx, |this, cx| this.load_gap(hunk, cx));
                })
                .into_any_element()
        }
        Row::GapLine { hunk, line } => {
            let Some((gap, looks)) = data.gaps.get(&hunk) else { return div().into_any_element() };
            let Some(l) = gap.lines.get(line) else { return div().into_any_element() };
            code_line(Some(l.old), Some(l.new), ' ', &l.text, l.syntax.as_deref(), &[], looks, None, true)
        }
        Row::Line { hunk, line } => {
            let Some(l) = data.patch.hunks.get(hunk).and_then(|h| h.lines.get(line)) else {
                return div().into_any_element();
            };
            let (sign, tint) = match l.kind {
                LineKind::Context => (' ', None),
                LineKind::Removed => ('-', Some(colours::failed())),
                LineKind::Added => ('+', Some(colours::done())),
            };
            code_line(l.old, l.new, sign, &l.text, l.syntax.as_deref(), &l.emphasis, &data.looks, tint, false)
        }
        Row::Hunk(i) => hunk_header(data, i),
    }
}

fn hunk_header(data: &DiffData, i: usize) -> AnyElement {
    let Some(hunk) = data.patch.hunks.get(i) else { return div().into_any_element() };
    let s = &data.selected;
    let (workspace, path) = (s.workspace, data.patch.file.path.clone());
    let changed = diff::changed(hunk);
    let reference = diff::reference(&path, changed);
    let patch_text = diff::patch(hunk);
    let group = SharedString::from(format!("hunk-{i}"));
    let action = |id: &'static str, label: &'static str| {
        div()
            .id((id, i))
            .px(px(6.))
            .rounded(px(4.))
            .text_size(px(11.))
            .text_color(colours::muted())
            .cursor_pointer()
            .hover(|s| s.bg(colours::selected()).text_color(colours::text()))
            .child(label)
    };
    let deleted = data.patch.file.status == FileStatus::Deleted;
    div()
        .group(group)
        .w_full()
        .flex()
        .items_center()
        .gap(px(4.))
        .mt(px(if i == 0 { 0. } else { 6. }))
        .px(px(8.))
        .py(px(3.))
        .bg(Rgba { a: 0.10, ..colours::accent() })
        .border_y_1()
        .border_color(Rgba { a: 0.18, ..colours::accent() })
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .truncate()
                .font_family(FONT_FAMILY)
                .text_size(px(11.))
                .text_color(colours::accent())
                .child(diff::header(hunk)),
        )
        .when(!deleted, |d| {
            let path = path.clone();
            d.child(action("hunk-open", "Open").on_click(move |_: &ClickEvent, _, cx| {
                ChangesPanel::open_in_editor(workspace, &path, changed.0, cx);
            }))
        })
        .child(action("hunk-ask", "Ask agent").on_click(move |_: &ClickEvent, _, cx| {
            ChangesPanel::ask_agent(workspace, reference.clone(), cx);
        }))
        .child(action("hunk-copy", "Copy").on_click(move |_: &ClickEvent, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(patch_text.clone()));
            toast("Copied the hunk as a patch", ToastKind::Info, cx);
        }))
        .into_any_element()
}

/// One line of code: both line numbers, the sign, and the text with its highlighting, tinted
/// green or red for added and removed lines, its changed words more strongly.
#[allow(clippy::too_many_arguments, reason = "the parts of a line, drawn the same for patch and gap lines")]
fn code_line(
    old: Option<u32>,
    new: Option<u32>,
    sign: char,
    text: &str,
    syntax: Option<&[Segment]>,
    emphasis: &[[usize; 2]],
    looks: &[Look],
    tint: Option<Rgba>,
    dim: bool,
) -> AnyElement {
    let number = |n: Option<u32>| {
        div()
            .w(px(34.))
            .flex_none()
            .pr(px(6.))
            .text_right()
            .text_color(colours::faint())
            .child(n.map(|n| n.to_string()).unwrap_or_default())
    };
    let highlights: Vec<_> = diff::runs(text, syntax, emphasis)
        .into_iter()
        .map(|run| {
            let look = looks.get(run.style).copied();
            let mut colour = look.map_or_else(|| colours::text().into(), |l| l.fg);
            if dim {
                colour.a *= 0.6;
            }
            let style = HighlightStyle {
                color: Some(colour),
                background_color: match (run.emphasis, tint) {
                    (true, Some(t)) => Some(Rgba { a: 0.32, ..t }.into()),
                    _ => None,
                },
                font_weight: look.filter(|l| l.bold).map(|_| FontWeight::BOLD),
                font_style: look.filter(|l| l.italic).map(|_| FontStyle::Italic),
                ..HighlightStyle::default()
            };
            (run.range, style)
        })
        .collect();
    let body: SharedString = if text.is_empty() { " ".into() } else { text.to_string().into() };
    let styled =
        if text.is_empty() { StyledText::new(body) } else { StyledText::new(body).with_highlights(highlights) };
    div()
        .w_full()
        .flex()
        .font_family(FONT_FAMILY)
        .text_size(px(12.))
        .line_height(px(18.))
        .when_some(tint, |d, t| d.bg(Rgba { a: 0.11, ..t }))
        .when(dim, |d| d.bg(colours::sidebar()))
        .child(number(old))
        .child(number(new))
        .child(
            div().w(px(14.)).flex_none().text_color(tint.map_or(colours::faint(), Into::into)).child(sign.to_string()),
        )
        .child(div().flex_1().min_w(px(0.)).pr(px(8.)).text_color(colours::text()).child(styled))
        .into_any_element()
}

// ---------------------------------------------------------------------------------------------
// Small pieces.

fn find<'a>(summary: &'a Summary, workspace: u64, path: &str) -> Option<&'a FileInfo> {
    summary.workspaces.iter().filter(|w| w.workspace == workspace).flat_map(|w| &w.files).find(|f| f.path == path)
}

fn find_mut<'a>(summary: &'a mut Summary, workspace: u64, path: &str) -> Option<&'a mut FileInfo> {
    summary
        .workspaces
        .iter_mut()
        .filter(|w| w.workspace == workspace)
        .flat_map(|w| &mut w.files)
        .find(|f| f.path == path)
}

/// The workspace's agent pane, the shown tab's first: what ` ask agent ` pastes into.
fn agent_pane(session: &Session, workspace: u64) -> Option<(u64, String)> {
    let (_, ws) = session.workspace(workspace)?;
    let mut tabs: Vec<_> = ws.tabs.iter().collect();
    tabs.sort_by_key(|t| !t.active);
    tabs.iter().flat_map(|t| &t.panes).find_map(|p| Some((p.id, p.agent.clone()?)))
}

fn looks(styles: &[crate::model::Style]) -> Vec<Look> {
    let colours = Colours::default();
    let painter = Painter { colours: &colours };
    styles.iter().map(|style| Look { fg: painter.resolve(style).0, bold: style.bold, italic: style.italic }).collect()
}

fn toast(text: impl Into<String>, kind: ToastKind, cx: &mut App) {
    let text = text.into();
    Store::get(cx).update(cx, |s, cx| s.toast(text, kind, cx));
}

fn split_path(path: &str) -> (String, String) {
    match path.rsplit_once('/') {
        Some((dir, name)) => (format!("{dir}/"), name.to_string()),
        None => (String::new(), path.to_string()),
    }
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 { format!("1 {word}") } else { format!("{n} {word}s") }
}

fn status_colour(status: FileStatus) -> Rgba {
    match status {
        FileStatus::Modified => colours::working(),
        FileStatus::Added | FileStatus::Untracked => colours::done(),
        FileStatus::Deleted => colours::failed(),
        FileStatus::Renamed => colours::accent(),
        FileStatus::Other => colours::muted(),
    }
}

fn counts(added: usize, removed: usize) -> impl IntoElement {
    div()
        .flex_none()
        .flex()
        .gap(px(4.))
        .font_family(FONT_FAMILY)
        .text_size(px(11.))
        .child(div().text_color(colours::done()).child(format!("+{added}")))
        .child(div().text_color(colours::failed()).child(format!("−{removed}")))
}

fn tag_badge(text: &'static str) -> impl IntoElement {
    div()
        .flex_none()
        .px(px(4.))
        .rounded(px(3.))
        .border_1()
        .border_color(colours::border())
        .text_size(px(10.))
        .text_color(colours::faint())
        .child(text)
}

/// The index state in git's colours: nothing staged is blank (a dim `+` on a hovered row),
/// partly `◐`, fully `●`. A click stages, or unstages once everything is.
fn stage_button(group: &SharedString, staged: Option<Staging>) -> gpui::Stateful<gpui::Div> {
    let all = staged == Some(Staging::Yes);
    let (glyph, colour) = match staged {
        Some(Staging::Yes) => ("●", colours::done()),
        Some(Staging::Partly) => ("◐", colours::done()),
        _ => ("+", Rgba { a: 0., ..colours::faint() }),
    };
    let unstaged = !matches!(staged, Some(Staging::Yes | Staging::Partly));
    div()
        .id(SharedString::from(format!("stage-{group}")))
        .w(px(18.))
        .flex_none()
        .flex()
        .justify_center()
        .rounded(px(4.))
        .text_size(px(12.))
        .text_color(colour)
        .cursor_pointer()
        .when(unstaged, |d| d.group_hover(group.clone(), |s| s.text_color(colours::faint())))
        .hover(move |s| s.bg(colours::selected()).text_color(if all { colours::working() } else { colours::done() }))
        .child(glyph)
}

/// The mark of the base in use, in a fixed slot so the names line up.
fn chosen_dot(chosen: bool) -> impl IntoElement {
    div().w(px(12.)).flex_none().text_size(px(10.)).text_color(colours::accent()).child(if chosen { "●" } else { "" })
}

/// The viewed mark: a box that fills with `✓`.
fn viewed_box(workspace: u64, path: &str, viewed: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(format!("viewed-{workspace}-{path}")))
        .size(px(14.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.))
        .border_1()
        .text_size(px(10.))
        .cursor_pointer()
        .when(viewed, |d| d.bg(colours::done()).border_color(colours::done()).text_color(colours::sidebar()).child("✓"))
        .when(!viewed, |d| d.border_color(colours::faint()).hover(|s| s.border_color(colours::text())))
}

/// A workspace's header in Every project: its project (and workspace when the project has
/// several) with its totals.
fn group_header(
    session: Option<&Session>,
    project: u64,
    workspace: u64,
    files: usize,
    added: usize,
    removed: usize,
) -> impl IntoElement {
    let label = session.and_then(|s| s.workspace(workspace)).map_or_else(
        || format!("project {project}"),
        |(p, w)| if p.workspaces.len() > 1 { format!("{} › {}", p.name, w.name) } else { p.name.clone() },
    );
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(8.))
        .pt(px(10.))
        .pb(px(4.))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .truncate()
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colours::muted())
                .child(label),
        )
        .child(div().text_size(px(11.)).text_color(colours::faint()).child(plural(files, "file")))
        .child(counts(added, removed))
}
