use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Rgba,
    SharedString, StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px, relative,
};

use crate::model::Session;
use crate::notify::tip;
use crate::panels::on_machine;
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui;

use super::actions::file_busy;
use super::pieces::{counts, faint_detail, plural, split_path, status_letter};
use super::wire::{FileInfo, FoldClass, Mode, Staging, State, Summary, WorkspaceChanges};
use super::{ChangesPanel, Scope, Selected, Whose};

const NO_MATCH: &str = "No file matches the filter.";

fn centred(title: String, detail: Option<String>, colour: Rgba) -> AnyElement {
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
            d.child(div().max_w(px(320.)).text_center().text_size(px(11.)).text_color(colours::faint()).child(text))
        })
        .into_any_element()
}

fn centred_busy(id: &'static str, text: &'static str) -> AnyElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p(px(24.))
        .child(ui::busy(id, text).text_size(px(13.)))
        .into_any_element()
}

fn reading() -> AnyElement {
    centred_busy("changes-reading", "Reading changes…")
}

fn nothing_changed(scope: Scope) -> AnyElement {
    let (title, detail) = match scope {
        Scope::Every => ("No changes in any project", None),
        Scope::Project(Mode::Uncommitted) => ("No uncommitted changes", Some("The working tree matches HEAD.")),
        Scope::Project(Mode::Commits) => ("No commits on this branch", Some("Nothing was committed since the base.")),
        Scope::Project(Mode::All) => ("No changes since the base", None),
    };
    centred(format!("✓ {title}"), detail.map(Into::into), colours::muted())
}

fn summary_note(summary: &Summary, scope: Scope) -> Option<AnyElement> {
    let empty = summary.workspaces.iter().all(|w| w.files.is_empty());
    match summary.state {
        State::Loading if empty => Some(reading()),
        State::Failed => Some(centred(
            "Could not read the changes".into(),
            Some(summary.error.clone().unwrap_or_else(|| "git failed".into())),
            colours::failed(),
        )),
        _ if empty => Some(nothing_changed(scope)),
        _ => None,
    }
}

fn fold_tag(fold: FoldClass) -> Option<&'static str> {
    match fold {
        FoldClass::Lockfile => Some("lockfile"),
        FoldClass::Binary => Some("binary"),
        FoldClass::Large => Some("too large"),
        _ => None,
    }
}

fn tag_badge(text: &'static str) -> impl IntoElement {
    div()
        .flex_none()
        .px(px(5.))
        .rounded(px(radius::chip()))
        .border_1()
        .border_color(colours::border())
        .text_size(px(10.))
        .text_color(colours::faint())
        .child(text)
}

fn stage_glyph(staged: Option<Staging>) -> &'static str {
    match staged {
        Some(Staging::Yes) => "●",
        Some(Staging::Partly) => "◐",
        _ => "+",
    }
}

fn stage_button(group: &SharedString, staged: Option<Staging>) -> gpui::Stateful<gpui::Div> {
    let all = staged == Some(Staging::Yes);
    let glyph = stage_glyph(staged);
    let colour = if matches!(staged, Some(Staging::Yes | Staging::Partly)) {
        colours::done()
    } else {
        Rgba { a: 0., ..colours::faint() }
    };
    let unstaged = !matches!(staged, Some(Staging::Yes | Staging::Partly));
    div()
        .id(SharedString::from(format!("stage-{group}")))
        .w(px(18.))
        .flex_none()
        .flex()
        .justify_center()
        .rounded(px(radius::chip()))
        .text_size(px(12.))
        .text_color(colour)
        .cursor_pointer()
        .when(unstaged, |d| d.group_hover(group.clone(), |s| s.text_color(colours::faint())))
        .hover(move |s| s.bg(colours::selected()).text_color(if all { colours::working() } else { colours::done() }))
        .child(glyph)
}

fn viewed_box(workspace: u64, path: &str, viewed: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(format!("viewed-{workspace}-{path}")))
        .size(px(15.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::chip()))
        .border_1()
        .text_size(px(10.))
        .cursor_pointer()
        .when(viewed, |d| d.bg(colours::done()).border_color(colours::done()).text_color(colours::ink()).child("✓"))
        .when(!viewed, |d| d.border_color(colours::faint()).hover(|s| s.border_color(colours::text())))
}

fn marks_there(file: &FileInfo, mode: Mode) -> Vec<AnyElement> {
    let stage = match file.staged {
        Some(Staging::Yes | Staging::Partly) => stage_glyph(file.staged),
        _ => "",
    };
    let staged = div().w(px(18.)).flex_none().flex().justify_center().text_color(colours::done()).child(stage);
    let viewed = div()
        .size(px(15.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.))
        .text_color(colours::done())
        .child(if file.viewed { "✓" } else { "" });
    let mut marks = Vec::new();
    if mode.uncommitted() {
        marks.push(staged.into_any_element());
    }
    marks.push(viewed.into_any_element());
    marks
}

fn is_muted(file: &FileInfo) -> bool {
    file.viewed || file.fold != FoldClass::Open
}

fn shared_mark(workspace: u64, file: &FileInfo, hint: String) -> impl IntoElement + use<> {
    div()
        .id(SharedString::from(format!("shared-{workspace}-{}", file.path)))
        .flex_none()
        .text_size(px(11.))
        .text_color(if is_muted(file) { colours::muted() } else { colours::working() })
        .child("⇄")
        .tooltip(tip(hint))
}

fn shared_hint(workspace: u64, file: &FileInfo, cx: &Context<ChangesPanel>) -> Option<String> {
    let store = Store::get(cx);
    let session = store.read(cx).session.as_ref()?;
    session.file_shared_tip(workspace, &file.path, file.old_path.as_deref())
}

fn file_name(file: &FileInfo) -> impl IntoElement + use<> {
    let (dir, name) = split_path(&file.path);
    let muted = is_muted(file);
    let detail = match &file.old_path {
        Some(old) => format!("{old} →"),
        None => dir,
    };
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
                .flex()
                .max_w(relative(0.7))
                .text_color(if muted { colours::muted() } else { colours::text() })
                .child(ui::one_line(name)),
        )
        .child(faint_detail(detail))
}

fn group_header(session: Option<&Session>, ws: &WorkspaceChanges) -> impl IntoElement + use<> {
    let label = session.and_then(|s| s.workspace(ws.workspace)).map_or_else(
        || format!("project {}", ws.project),
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
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colours::muted())
                .child(ui::one_line(label)),
        )
        .child(div().text_size(px(11.)).text_color(colours::faint()).child(plural(ws.files.len(), "file")))
        .child(counts(ws.added, ws.removed))
}

impl ChangesPanel {
    pub(super) fn render_files(&self, session: Option<&Session>, cx: &mut Context<Self>) -> AnyElement {
        if let Some(note) = self.state_note(session) {
            return note;
        }
        let Some(summary) = &self.summary else { return ui::note("").into_any_element() };
        let mode = self.scope.mode();
        let every = self.scope == Scope::Every;
        let query = self.query(cx);
        let mut rows: Vec<AnyElement> = Vec::new();
        for ws in &summary.workspaces {
            let kept: Vec<&FileInfo> = ws.files.iter().filter(|file| query.keeps(file)).collect();
            if every && !kept.is_empty() {
                rows.push(group_header(session, ws).into_any_element());
            }
            rows.extend(kept.into_iter().map(|file| self.render_file(ws.workspace, file, mode, cx).into_any_element()));
        }
        if rows.is_empty() {
            return ui::note(NO_MATCH).into_any_element();
        }
        let col = div().flex().flex_col().gap(px(2.)).p(px(8.)).children(rows);
        div()
            .id("changes-files")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.files_scroll)
            .child(col)
            .into_any_element()
    }

    fn state_note(&self, session: Option<&Session>) -> Option<AnyElement> {
        if session.is_none() {
            return Some(centred_busy("changes-connecting", "Connecting…"));
        }
        if self.follow.is_none() {
            let detail = Some("Open a project to see its changes.".into());
            return Some(centred("No workspace shown".into(), detail, colours::muted()));
        }
        if let Some(err) = &self.ended {
            return Some(centred("The changes are not available".into(), Some(err.clone()), colours::failed()));
        }
        let Some(summary) = &self.summary else { return Some(self.nothing_yet().unwrap_or_else(reading)) };
        summary_note(summary, self.scope)
    }

    fn nothing_yet(&self) -> Option<AnyElement> {
        self.machine_note()?;
        Some(centred(on_machine::nothing_read(self.machine_shown()?), None, colours::muted()))
    }

    fn render_file(
        &self,
        workspace: u64,
        file: &FileInfo,
        mode: Mode,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let whose = self.whose_row(workspace);
        let selected = self.selected.as_ref().is_some_and(|s| s.workspace == whose && s.path == file.path);
        let id = SharedString::from(format!("file-{workspace}-{}", file.path));
        let group = SharedString::from(format!("row-{workspace}-{}", file.path));
        let hint = whose.here().and_then(|here| shared_hint(here, file, cx));
        let marks = Self::row_marks(&group, &whose, file, mode, cx);
        let pick = Selected { workspace: whose, path: file.path.clone(), mode };
        ui::row(id, selected)
            .group(group.clone())
            .gap(px(6.))
            .py(px(5.))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_selected(&pick, cx)))
            .child(status_letter(file.status).w(px(12.)).flex_none())
            .child(file_name(file))
            .when_some(hint, |d, hint| d.child(shared_mark(workspace, file, hint)))
            .when_some(fold_tag(file.fold), |d, tag| d.child(tag_badge(tag)))
            .child(counts(file.added, file.removed))
            .children(marks)
    }

    fn row_marks(
        group: &SharedString,
        whose: &Whose,
        file: &FileInfo,
        mode: Mode,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Whose::Here(workspace) = *whose else { return marks_there(file, mode) };
        let mut marks = Vec::new();
        if mode.uncommitted() {
            marks.push(Self::row_stage_button(group, workspace, file, cx));
        }
        marks.push(Self::row_viewed_box(workspace, file, cx).into_any_element());
        marks
    }

    fn toggle_selected(&mut self, pick: &Selected, cx: &mut Context<Self>) {
        let same = self.selected.as_ref() == Some(pick);
        if !same && self.refuses_reading(&pick.workspace, cx) {
            return;
        }
        self.select((!same).then(|| pick.clone()), cx);
    }

    fn row_stage_button(group: &SharedString, workspace: u64, file: &FileInfo, cx: &Context<Self>) -> AnyElement {
        if Store::get(cx).read(cx).busy(&file_busy(workspace, &file.path)) {
            let id = SharedString::from(format!("staging-{group}"));
            return div()
                .w(px(18.))
                .flex_none()
                .flex()
                .justify_center()
                .child(ui::spinner(id, 11., colours::muted()))
                .into_any_element();
        }
        let stage_file = file.clone();
        stage_button(group, file.staged)
            .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                cx.stop_propagation();
                Self::toggle_staged(workspace, &stage_file, cx);
            }))
            .into_any_element()
    }

    fn row_viewed_box(workspace: u64, file: &FileInfo, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let viewed_file = file.clone();
        viewed_box(workspace, &file.path, file.viewed).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            cx.stop_propagation();
            this.toggle_viewed(workspace, &viewed_file, cx);
        }))
    }
}
