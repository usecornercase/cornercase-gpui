use gpui::{AnyElement, Context, Div, IntoElement, SharedString, Window, div, prelude::*, px};

use super::data::{Described, Exported, ProjectEntry, Script};
use super::{Pending, Role, SettingsPanel, role_id, widgets};
use crate::theme::colours;
use crate::ui;

fn project_title(project: &ProjectEntry) -> Div {
    let name = project.folder.rsplit('/').find(|s| !s.is_empty()).unwrap_or(&project.folder).to_string();
    div()
        .flex()
        .items_baseline()
        .gap(px(8.))
        .pt(px(10.))
        .pb(px(4.))
        .child(ui::title(name))
        .child(ui::dim(project.folder.clone()))
        .when(project.project.is_none(), |d| d.child(ui::dim("· not open")))
}

fn script_lines(text: &str) -> Div {
    div()
        .w(px(300.))
        .px(px(8.))
        .py(px(5.))
        .rounded(px(5.))
        .bg(colours::panel())
        .font_family(crate::theme::FONT_FAMILY)
        .text_size(px(12.))
        .children(text.lines().map(|l| div().child(l.to_string())))
}

fn export_summary(e: Exported) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(3.))
        .pb(px(10.))
        .child(ui::dim(format!("{} settings and {} repositories went", e.settings, e.repositories)))
        .when(!e.left_out.is_empty(), |d| d.child(ui::dim(format!("left out: {}", e.left_out.join("; ")))))
        .children(
            e.warnings
                .into_iter()
                .map(|w| div().text_size(px(12.)).text_color(colours::failed()).child(format!("! {w}"))),
        )
}

fn import_section() -> Div {
    div().flex().flex_col().child(ui::section("Import")).child(
        widgets::card().child(widgets::row(
            true,
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(widgets::label(
                    "A setup a colleague shared",
                    "every change, and what would run unattended, has to be shown before it applies",
                ))
                .child(widgets::missing("Importing from here waits for a preview of it", 464)),
            ui::button_coloured("import", "Import…", colours::faint()).cursor_default(),
        )),
    )
}

impl SettingsPanel {
    pub(super) fn render_projects(
        &self,
        d: &Described,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let mut page = div().flex().flex_col().gap(px(14.)).child(widgets::heading(
            "Projects",
            "Commands a repository runs in a new worktree once it is made, and before it is removed.",
        ));
        if d.projects.is_empty() {
            page = page.child(ui::note("No projects: open a repository to give it a setup."));
        }
        for project in &d.projects {
            page = page.child(self.project_card(project, window, cx));
        }
        page
    }

    fn project_card(&self, project: &ProjectEntry, window: &Window, cx: &mut Context<Self>) -> Div {
        let card = widgets::card().child(project_title(project)).when(!project.repository, |d| {
            d.child(ui::dim("not a git repository: its commands can only be cleared").pb(px(4.)))
        });
        let scripts: Vec<_> =
            [Script::Setup, Script::Teardown].into_iter().map(|s| self.render_script(project, s, window, cx)).collect();
        card.children(scripts)
    }

    fn multiline_script(project: &ProjectEntry, script: Script, cx: &mut Context<Self>) -> Div {
        let role = Role::Script(project.folder.clone(), script);
        let pending = Pending::Clear(project.folder.clone(), script);
        div().flex().items_start().gap(px(6.)).child(script_lines(script.of(project))).child(
            ui::button(SharedString::from(format!("clear-{}", role_id(&role))), "Clear").on_click(cx.listener(
                move |this, _, _, cx| {
                    this.pending = Some(pending.clone());
                    cx.notify();
                },
            )),
        )
    }

    fn render_script(
        &self,
        project: &ProjectEntry,
        script: Script,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let role = Role::Script(project.folder.clone(), script);
        let label = widgets::label(
            widgets::capitalised(script.name()),
            match script {
                Script::Setup => "after a worktree is made, in it",
                Script::Teardown => "before a worktree is removed",
            },
        );
        let control = if script.of(project).contains('\n') {
            Self::multiline_script(project, script, cx).into_any_element()
        } else {
            self.render_text(&role, 300., window, cx).into_any_element()
        };
        let confirm = (self.pending == Some(Pending::Clear(project.folder.clone(), script))).then(|| {
            let folder = project.folder.clone();
            let go = ui::danger_button("pending-go", "Clear")
                .on_click(cx.listener(move |this, _, _, cx| this.save_script(&folder, script, "", cx)));
            Self::ask(format!("Clear this {}? Its lines cannot be typed again here.", script.name()), go, cx)
        });
        div()
            .flex()
            .flex_col()
            .child(widgets::row(false, label, control))
            .children(confirm)
            .children(self.error_of(&role))
            .into_any_element()
    }

    pub(super) fn render_share(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(widgets::heading(
                "Share",
                "Hand your setup to a colleague: agents, issues, worktrees, the interface and each repository's commands.",
            ))
            .child(div().flex().flex_col().child(ui::section("Export")).child(self.export_card(cx)))
            .child(import_section())
    }

    fn export_card(&self, cx: &mut Context<Self>) -> Div {
        widgets::card()
            .child(widgets::row(
                true,
                widgets::label(
                    "Your setup, as TOML",
                    "never tokens, accounts or personal keys (Jira email, remote access, keep awake)",
                ),
                div()
                    .flex()
                    .gap(px(6.))
                    .child(
                        ui::button("export-file", "Save to file…")
                            .on_click(cx.listener(|_, _, _, cx| Self::export_to_file(cx))),
                    )
                    .child(
                        ui::primary_button("export-copy", "Copy my setup")
                            .on_click(cx.listener(|_, _, _, cx| Self::export(None, cx))),
                    ),
            ))
            .children(self.exported.clone().map(export_summary))
    }
}
