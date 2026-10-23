use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, div, prelude::FluentBuilder as _, px, relative,
};

use crate::theme::colours;
use crate::ui;

use super::pieces::{counts, faint_detail, find, split_path, status_letter};
use super::wire::{FileInfo, FileStatus, FoldClass, Staging, State};
use super::{ChangesPanel, Selected, diff, diff_view};

fn empty_diff_text(file: &FileInfo) -> &'static str {
    match file.fold {
        FoldClass::Binary => "Binary file: no lines to show.",
        FoldClass::Large => "Too large to show: only its counts are listed.",
        FoldClass::Lockfile => "A lockfile with no lines to show.",
        _ if file.status == FileStatus::Renamed => "Renamed without changes.",
        _ => "No lines to show.",
    }
}

fn uncommitted_buttons(workspace: u64, file: &FileInfo) -> [impl IntoElement + use<>; 2] {
    let staged = file.staged == Some(Staging::Yes);
    let (stage_file, discard_file) = (file.clone(), file.clone());
    [
        ui::button("open-stage", if staged { "− Unstage" } else { "+ Stage" })
            .on_click(move |_: &ClickEvent, _, cx| ChangesPanel::toggle_staged(workspace, &stage_file, cx)),
        ui::button_coloured("open-discard", "↺ Discard", colours::failed())
            .on_click(move |_: &ClickEvent, window, cx| ChangesPanel::discard(workspace, &discard_file, window, cx)),
    ]
}

impl ChangesPanel {
    fn open_file_info(&self, selected: &Selected) -> FileInfo {
        self.summary
            .as_ref()
            .and_then(|s| find(s, selected.workspace, &selected.path))
            .cloned()
            .or_else(|| self.open.data.as_ref().map(|d| d.patch.file.clone()))
            .unwrap_or_else(|| FileInfo { path: selected.path.clone(), ..FileInfo::default() })
    }

    fn first_changed_line(&self) -> u32 {
        self.open.data.as_ref().and_then(|d| d.patch.hunks.first()).map_or(1, |h| diff::changed(h).0)
    }

    fn open_actions(&self, selected: &Selected, file: &FileInfo, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let workspace = selected.workspace;
        let first_line = self.first_changed_line();
        let (viewed_path, open_path) = (file.path.clone(), file.path.clone());
        let can_open = file.status != FileStatus::Deleted;
        div()
            .flex()
            .flex_wrap()
            .gap(px(6.))
            .child(
                ui::button("open-viewed", if file.viewed { "✓ Viewed" } else { "Mark viewed" })
                    .when(file.viewed, |b| b.text_color(colours::done()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.toggle_viewed(workspace, &viewed_path, cx);
                    })),
            )
            .when(selected.mode.uncommitted(), |d| d.children(uncommitted_buttons(workspace, file)))
            .when(can_open, |d| {
                d.child(ui::button("open-editor", "Open in editor").on_click(move |_: &ClickEvent, _, cx| {
                    Self::open_in_editor(workspace, &open_path, first_line, cx);
                }))
            })
    }

    fn open_title(&self, file: &FileInfo, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (dir, name) = split_path(&file.path);
        let detail = match &file.old_path {
            Some(old) => format!("from {old}"),
            None => dir,
        };
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(status_letter(file.status).flex_none())
            .child(div().flex_none().max_w(relative(0.6)).truncate().font_weight(FontWeight::SEMIBOLD).child(name))
            .child(faint_detail(detail))
            .when(self.open.loading && self.open.data.is_some(), |d| d.child(ui::dim("updating…")))
            .child(counts(file.added, file.removed))
            .child(
                ui::icon_button("open-close", "✕")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.select(None, cx))),
            )
    }

    pub(super) fn render_open(&self, selected: &Selected, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let file = self.open_file_info(selected);
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
            .child(self.open_title(&file, cx))
            .child(self.open_actions(selected, &file, cx));
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
            return ui::note(empty_diff_text(file)).into_any_element();
        }
        diff_view::diff_list(&self.diff_list, data, cx.entity().downgrade())
    }

    pub(super) fn render_footer(&self) -> impl IntoElement + use<> {
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
