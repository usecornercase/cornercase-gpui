//! The sidebar: groups, projects and their workspaces with the loudest agent state of each.

use gpui::{
    ClickEvent, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, PathPromptOptions, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder as _, px,
};

use crate::app::{Root, SIDEBAR_WIDTH, TITLEBAR_HEIGHT};
use crate::dialogs;
use crate::model::Project;
use crate::panels::PanelKind;
use crate::theme::colours;
use crate::ui::{badge, button_coloured as button};

impl Root {
    // -----------------------------------------------------------------------------------------
    // Drawing.

    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let shown = self.session.as_ref().map(|s| s.shown()).unwrap_or_default();
        let mut list = div().flex().flex_col().gap(px(1.)).px(px(8.));
        if let Some(session) = &self.session {
            let groups = &session.groups;
            let mut ordered: Vec<(Option<&crate::model::Group>, Vec<&Project>)> = Vec::new();
            for project in &session.projects {
                let group = project.group.and_then(|g| groups.iter().find(|x| x.id == g));
                match ordered.iter_mut().find(|(g, _)| g.map(|g| g.id) == group.map(|g| g.id)) {
                    Some((_, ps)) => ps.push(project),
                    None => ordered.push((group, vec![project])),
                }
            }
            for (group, projects) in ordered {
                if let Some(group) = group {
                    list = list.child(
                        div()
                            .pt(px(10.))
                            .pb(px(2.))
                            .px(px(6.))
                            .text_size(px(11.))
                            .text_color(colours::faint())
                            .child(format!("{} {}", group.icon.clone().unwrap_or_default(), group.name.to_uppercase())),
                    );
                    if group.collapsed {
                        continue;
                    }
                }
                for project in projects {
                    list = list.child(self.render_project(project, shown, cx));
                }
            }
        }
        div()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .bg(colours::sidebar())
            .border_r_1()
            .border_color(colours::border())
            .child(div().h(px(TITLEBAR_HEIGHT)).flex_none())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(14.))
                    .pb(px(6.))
                    .child(div().text_size(px(11.)).text_color(colours::faint()).child("PROJECTS"))
                    .child(
                        button("open-project", "+ Open", colours::muted())
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_project(window, cx))),
                    ),
            )
            .child(div().id("sidebar-list").flex_1().overflow_y_scroll().child(list))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(4.))
                    .px(px(10.))
                    .pt(px(8.))
                    .border_t_1()
                    .border_color(colours::border())
                    .children(PanelKind::ALL.into_iter().filter(|k| !k.docked()).map(|kind| {
                        let store = self.store.clone();
                        crate::ui::segment(SharedString::from(format!("open-{}", kind.title())), kind.title(), false)
                            .on_click(move |_: &ClickEvent, _, cx| store.update(cx, |s, cx| s.open(kind, cx)))
                    })),
            )
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .p(px(10.))
                    .child(
                        button("new-workspace", "+ Workspace", colours::muted())
                            .on_click(cx.listener(|_, _: &ClickEvent, window, cx| dialogs::new_workspace(None, window, cx))),
                    )
                    .child(
                        button("start-agent", "▶ Agent", colours::accent())
                            .on_click(cx.listener(|_, _: &ClickEvent, window, cx| dialogs::start_agent(window, cx))),
                    ),
            )
    }

    pub(crate) fn render_project(&self, project: &Project, shown: crate::model::Shown, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let id = project.id;
        let is_shown = shown.project == Some(id);
        let mut col = div().flex().flex_col().pt(px(6.)).child(
            div()
                .id(SharedString::from(format!("project-{id}")))
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(6.))
                .py(px(4.))
                .rounded(px(5.))
                .cursor_pointer()
                .hover(|s| s.bg(colours::hover()))
                .child(
                    div()
                        .flex_1()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(if is_shown { colours::text() } else { colours::muted() })
                        .child(project.name.clone()),
                )
                .when(project.collapsed, |d| d.child(badge(project.loudest())))
                .child(
                    div()
                        .id(SharedString::from(format!("project-add-{id}")))
                        .px(px(5.))
                        .rounded(px(4.))
                        .text_color(colours::faint())
                        .hover(|s| s.bg(colours::selected()).text_color(colours::text()))
                        .child("+")
                        .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            dialogs::new_workspace(Some(id), window, cx);
                        })),
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _, _| this.focus_item("project", id))),
        );
        if project.collapsed {
            return col;
        }
        for ws in &project.workspaces {
            let wid = ws.id;
            let selected = shown.workspace == Some(wid);
            let agents: Vec<_> = ws.tabs.iter().filter_map(|t| t.agent_pane()).collect();
            let label = if ws.worktree { format!("⎇ {}", ws.name) } else { ws.name.clone() };
            let detail = match agents.len() {
                0 => format!("{} tab{}", ws.tabs.len(), if ws.tabs.len() == 1 { "" } else { "s" }),
                n => format!("{n} agent{}", if n == 1 { "" } else { "s" }),
            };
            col = col.child(
                div()
                    .id(SharedString::from(format!("ws-{wid}")))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .ml(px(8.))
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .when(selected, |d| d.bg(colours::selected()))
                    .hover(|s| s.bg(colours::hover()))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .child(
                                div()
                                    .whitespace_nowrap()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .text_color(if selected { colours::text() } else { colours::muted() })
                                    .child(label),
                            )
                            .child(div().text_size(px(11.)).text_color(colours::faint()).child(detail)),
                    )
                    .child(badge(ws.loudest()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, _| this.focus_item("workspace", wid))),
            );
        }
        col
    }

    pub(crate) fn open_project(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open project".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.first()
            {
                let path = path.to_string_lossy().into_owned();
                let _ = this.update(cx, |this, _| this.act(vec!["open".into(), path, "--focus".into()]));
            }
        })
        .detach();
    }
}
