use gpui::{AnyElement, Context, Div, FontWeight, IntoElement, SharedString, Window, div, prelude::*, px};

use super::controls::Look;
use super::data::{Described, ProjectEntry, Script};
use super::{Role, SettingsPanel, role_id, widgets};
use crate::theme::{FONT_FAMILY, colours, radius};
use crate::ui::{self, Icon};

fn project_header(project: &ProjectEntry) -> Div {
    let name = project.folder.rsplit('/').find(|s| !s.is_empty()).unwrap_or(&project.folder).to_string();
    let chips = [
        project.project.is_none().then(|| ui::quiet_chip("not open")),
        (!project.repository).then(|| ui::tinted_chip("not a git repository", colours::waiting())),
    ];
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .min_h(px(46.))
        .border_b_1()
        .border_color(colours::border())
        .child(ui::icon(Icon::FolderOpen, 15.).text_color(colours::accent()))
        .child(div().flex_none().text_size(px(13.5)).font_weight(FontWeight::SEMIBOLD).child(name))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .text_size(px(11.5))
                .text_color(colours::faint())
                .child(ui::one_line(ui::home_relative(&project.folder))),
        )
        .children(chips.into_iter().flatten())
}

fn script_lines(text: &str) -> Div {
    div()
        .w(px(Look::SCRIPT.width))
        .px(px(10.))
        .py(px(6.))
        .rounded(px(radius::CONTROL))
        .bg(colours::field())
        .border_1()
        .border_color(colours::edge())
        .font_family(FONT_FAMILY)
        .text_size(px(12.))
        .children(text.lines().map(|l| div().child(l.to_string())))
}

fn script_label(script: Script) -> Div {
    let note = match script {
        Script::Setup => "Runs in a new worktree, once it is made",
        Script::Teardown => "Runs in a worktree, before it is removed",
    };
    widgets::label(widgets::capitalised(script.name()), note)
}

fn no_projects() -> Div {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(6.))
        .py(px(36.))
        .child(ui::emblem(Icon::FolderOpen))
        .child(ui::title("No projects yet"))
        .child(ui::dim("Open a repository in cornercase to give it setup and teardown commands.").text_size(px(12.)))
}

impl SettingsPanel {
    pub(super) fn render_projects(&self, d: &Described, window: &Window, cx: &mut Context<Self>) -> Vec<Div> {
        if d.projects.is_empty() {
            return vec![no_projects()];
        }
        d.projects.iter().map(|project| self.project_card(project, window, cx)).collect()
    }

    fn project_card(&self, project: &ProjectEntry, window: &Window, cx: &mut Context<Self>) -> Div {
        let warning = (!project.repository).then(|| {
            div().pt(px(10.)).child(ui::warning_line(
                "Not a git repository, so it makes no worktrees: its commands can only be cleared.",
            ))
        });
        let scripts: Vec<AnyElement> =
            [Script::Setup, Script::Teardown].into_iter().map(|s| self.render_script(project, s, window, cx)).collect();
        widgets::card().child(project_header(project)).children(warning).children(scripts)
    }

    fn render_script(
        &self,
        project: &ProjectEntry,
        script: Script,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let role = Role::Script(project.folder.clone(), script);
        let multiline = script.of(project).contains('\n');
        let (control, slot) = if multiline {
            (script_lines(script.of(project)).into_any_element(), self.clear_action(project, script, cx))
        } else {
            (self.render_text(&role, Look::SCRIPT, window, cx).into_any_element(), div().into_any_element())
        };
        div()
            .flex()
            .flex_col()
            .child(widgets::row(script == Script::Setup, script_label(script), control, slot))
            .children(self.error_of(&role))
            .into_any_element()
    }

    fn clear_action(&self, project: &ProjectEntry, script: Script, cx: &mut Context<Self>) -> AnyElement {
        let role = Role::Script(project.folder.clone(), script);
        if self.busy.contains(&role) {
            return ui::spinner(SharedString::from(format!("saving-{}", role_id(&role))), 13., colours::muted())
                .into_any_element();
        }
        let folder = project.folder.clone();
        let id = SharedString::from(format!("clear-{}", role_id(&role)));
        ui::row_action(id, Icon::Trash, format!("Clear this {}…", script.name()), true)
            .on_click(cx.listener(move |_, _, window, cx| Self::ask_clear(&folder, script, window, cx)))
            .into_any_element()
    }
}
